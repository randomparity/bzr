# OS Keychain-Backed API Key Storage

**Issue:** [#69](https://github.com/randomparity/bzr/issues/69)
**Status:** Design approved; ready for implementation planning
**Date:** 2026-04-06

## Problem

Issue #60 introduced env-backed API key support (`api_key_env`) and hardened
`config.toml` permissions, but users are still responsible for provisioning
the secret outside `bzr`. Issue #69 tracks first-class integration with
native OS credential stores so `bzr` can retrieve Bugzilla API keys from:

- macOS Keychain
- Windows Credential Manager
- Linux Secret Service (gnome-keyring, kwallet)

## Goals

1. Add a third, mutually-exclusive credential source alongside `api_key` and
   `api_key_env`: `api_key_keyring`.
2. Provide end-to-end CLI UX for storing, removing, and migrating keyring-
   backed credentials without ever passing the secret on the command line.
3. Fail clearly and actionably when the keyring is unavailable, directing
   users toward `api_key_env` for headless / CI environments.
4. Keep the dependency optional so `--no-default-features` builds stay lean.
5. Cover the keyring path with hermetic unit/integration tests plus a
   real-backend functional test runnable via `make functional-test-all`.

## Non-goals

- No automatic fallback between sources. The three sources stay mutually
  exclusive (established by #60). Users pick one per server.
- No wrapping or replacement of platform-native tools (`security`,
  `secret-tool`, `cmdkey`) beyond the subcommands described below.
- No shared-credential discovery across tools. Default service name is
  `"bzr"`; users who want to share must opt in explicitly.

## Architecture

A new optional Cargo feature **`keyring`** (enabled by default) gates the
[`keyring`](https://crates.io/crates/keyring) crate dependency and its
platform backends (`apple-native`, `windows-native`, and a Linux Secret
Service backend). Builds with `--no-default-features` still parse keyring
config entries but return a clear "compiled without keyring support" error
at resolve time.

### Module layout

- **`src/credentials/mod.rs`** (new) — module root for credential backends.
- **`src/credentials/keyring.rs`** (new) — thin wrapper around
  `keyring::Entry` exposing `store`, `retrieve`, and `delete`, with typed
  error mapping. A sibling stub compiled when the feature is off returns
  unsupported errors.
- **`src/config.rs`** — extend `CredentialSource`, `CredentialSourceKind`,
  and `ServerConfig`; extend mutual-exclusion validation.
- **`src/error.rs`** — new `BzrError::Keyring(String)` variant with
  dedicated exit code and `"keyring"` error type.
- **`src/commands/config.rs`** — new `set-keyring`, `unset-keyring`,
  `migrate-to-keyring` subcommands.
- **`src/cli/config.rs`** — corresponding `ConfigAction` enum variants.

### Control flow

`ServerConfig::resolve_api_key` gains a third arm:

```rust
CredentialSource::Keyring { service, account } => {
    credentials::keyring::retrieve(service, account)
        .map_err(|e| BzrError::Keyring(format!(
            "server '{server_name}' keyring lookup failed: {e}"
        )))
}
```

All existing call sites that invoke `resolve_api_key` need no changes.

## Config schema

New field on `ServerConfig`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub api_key_keyring: Option<KeyringRef>,
```

New type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KeyringRef {
    /// Keyring service name. Defaults to "bzr" when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Account/username within the service. Defaults to the server name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl KeyringRef {
    pub fn service_or_default(&self) -> &str {
        self.service.as_deref().unwrap_or("bzr")
    }
    pub fn account_or_default<'a>(&'a self, server_name: &'a str) -> &'a str {
        self.account.as_deref().unwrap_or(server_name)
    }
}
```

Extended `CredentialSource`:

```rust
pub enum CredentialSource<'a> {
    Inline(&'a str),
    EnvVar(&'a str),
    Keyring { service: &'a str, account: &'a str },
}
```

Extended `CredentialSourceKind`:

```rust
pub enum CredentialSourceKind { Inline, Env, Keyring }
```

with `"keyring"` as the `as_str()` value.

### Mutual exclusion

`credential_source()` returns `Err` whenever more than one of
`api_key`, `api_key_env`, `api_key_keyring` is set. Error message names
all three fields so users can see the full set of sources.

### TOML examples

```toml
# Minimal — service="bzr", account="prod"
[servers.prod]
url = "https://bugzilla.example.com"
api_key_keyring = {}

# Explicit service + account
[servers.redhat]
url = "https://bugzilla.redhat.com"
api_key_keyring = { service = "bzr", account = "dave@redhat" }

# Headless / CI alternative (unchanged from #60)
[servers.ci]
url = "https://bugzilla.example.com"
api_key_env = "BZR_CI_API_KEY"
```

## CLI surface

Three new `bzr config` subcommands. No existing commands are modified.

### `bzr config set-keyring <server> [--service <name>] [--account <name>]`

- Requires `<server>` to exist already (created via `set-server`).
- Prompts for the API key on stdin with echo disabled (using `rpassword`).
- Stores the secret in the OS keychain at `(service, account)`, defaulting
  to `("bzr", <server>)`.
- Updates `config.toml`: clears any `api_key` / `api_key_env` and sets
  `api_key_keyring = { service?, account? }`, persisting only non-default
  values.
- On success, prints:
  `Stored API key for server '<server>' in OS keychain (service=<s>, account=<a>)`.

### `bzr config unset-keyring <server>`

- Reads `api_key_keyring` from the server entry.
- Deletes the keychain entry.
- Clears `api_key_keyring` from `config.toml`, leaving the rest of the
  server entry intact. User must re-credential via `set-server` or
  `set-keyring` afterward.
- Idempotent: a missing keychain entry is a warning, not an error.

### `bzr config migrate-to-keyring <server> [--service <name>] [--account <name>] [--yes]`

- Resolves the server's current credential via `resolve_api_key()`
  (supports both inline and env sources).
- Stores the value in the keychain.
- For **inline** sources, rewrites `config.toml` to drop `api_key` and add
  `api_key_keyring`.
- For **env** sources, does **not** rewrite config. Prints:
  `Stored API key for server '<server>' in OS keychain. The server is still
  configured to read 'api_key_env = "<VAR>"'. Edit config.toml manually to
  switch to the keychain if desired; the env var may be shared with other
  tools.`
- Without `--yes`, confirms before writing anything.

`set-server` is deliberately **not** extended to accept an API key flag,
so secrets never appear on the command line or in shell history.

## Error handling

New variant:

```rust
#[error("keyring error: {0}")]
Keyring(String),
```

Exit code: next free slot in `BzrError::exit_code()` (verified during
implementation). `error_type()` returns `"keyring"`.

The variant exists in all builds; only the producer is feature-gated. This
keeps error-handling code uniform and avoids `#[cfg]` sprinkled through
callers.

### `keyring::Error` → user-facing message

| Cause | Message |
|---|---|
| `NoEntry` | `no API key found in OS keychain for service='<s>' account='<a>'. Run \`bzr config set-keyring <server>\` to store one.` |
| `PlatformFailure` (D-Bus down, locked keyring, denied prompt) | `OS keychain unavailable: <detail>. For headless/CI environments, use \`api_key_env\` instead — see docs/bzr-cli.md.` |
| `Ambiguous` | `multiple matching keychain entries for service='<s>' account='<a>'; please remove duplicates.` |
| `BadEncoding` / `Invalid*` | `stored keychain entry for service='<s>' account='<a>' is corrupted: <detail>.` |
| any other | generic passthrough with underlying message. |

### Feature-disabled path

`#[cfg(not(feature = "keyring"))]` builds return:
`this bzr build was compiled without keyring support; rebuild with --features keyring or use api_key_env.`

## Documentation updates

- **`docs/bzr-cli.md`** — new "Credential storage" section covering all
  three sources, the three new subcommands, and a **"Headless / CI
  environments"** subsection recommending `api_key_env` with a sample
  `config.toml`, a systemd drop-in, a Dockerfile snippet, and a GitHub
  Actions example.
- **`docs/troubleshooting.md`** (new) — "Keyring / credential storage"
  section with platform-specific checks:
  - **Linux:** `systemctl --user status gnome-keyring-daemon`,
    `systemctl --user status kwalletd6`, `secret-tool store / lookup`
    probes, unlock flow, D-Bus session notes.
  - **macOS:** Keychain Access prompts, how to inspect / delete the
    `bzr` entries.
  - **Windows:** Credential Manager lookups, `cmdkey /list` verification.
- **`README.md`** — short mention of keyring support and pointer to the
  new docs sections.

## Testing strategy

### Unit tests (`src/config.rs`, `src/credentials/keyring.rs`)

- `KeyringRef` TOML round-trip for `{}`, `{ service = "..." }`,
  `{ account = "..." }`, and fully-specified form.
- `CredentialSource::Keyring` returned when only `api_key_keyring` is set.
- Mutual exclusion: each pair and the triple error with a message naming
  all three fields.
- `service_or_default()` / `account_or_default()` defaulting behavior.
- `resolve_api_key` against a **mock keyring**, using `keyring`'s built-in
  `mock` feature as a dev-dependency and
  `keyring::set_default_credential_builder(...)` in a test hook. Covers
  success, `NoEntry`, and platform-failure error mapping.
- Feature-gated-off path: `#[cfg(not(feature = "keyring"))]` test asserts
  the unsupported-build error is returned.

### Integration tests (`tests/integration.rs`)

- `set-keyring` → `resolve_api_key` → `unset-keyring` happy path, with the
  mock backend installed via a `cfg(test)` hook before any `Entry::new`.
- `migrate-to-keyring` from an inline source: asserts `config.toml` is
  rewritten to drop `api_key` and add `api_key_keyring`.
- `migrate-to-keyring` from an env source: asserts config is **not**
  rewritten and the advisory message is printed.

### Functional tests (`tests/functional/keyring.rs`, new)

- Exercises the **real OS keychain** on the host machine.
- Uses a unique service name (`bzr-functional-test-<pid>`) and always
  cleans up via a `Drop` guard.
- On macOS and Windows, runs unconditionally.
- On Linux, probes for a reachable Secret Service at startup and skips
  with a clear message if unavailable, so `make functional-test-all`
  stays green on headless dev machines.
- `make functional-test-all` target is extended to include this module.

### Out of scope

- Property / fuzz testing: the config parsing surface is small and covered
  by TOML round-trip unit tests.

## Dependency impact

- **`keyring`** (~v4.x) — optional, default-enabled. Platform backends:
  `apple-native` (macOS), `windows-native` (Windows), Linux Secret Service
  via the crate's sync backend.
- **`rpassword`** — small, unconditional. Used by `set-keyring` for
  stdin-no-echo secret entry.
- **`keyring`'s `mock` feature** — dev-dependency only.

Both crates are well-maintained and have existing use in the Rust CLI
ecosystem.

## Migration and backwards compatibility

- Existing `api_key` and `api_key_env` configs continue to work unchanged.
- Users opt in via `bzr config set-keyring <server>` or
  `bzr config migrate-to-keyring <server>`.
- No config file auto-migration; users must explicitly move existing
  credentials.

## Open questions

None. All resolved during brainstorming.
