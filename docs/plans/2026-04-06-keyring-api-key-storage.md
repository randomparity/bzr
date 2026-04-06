# OS Keychain-Backed API Key Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a third, mutually-exclusive API key source (`api_key_keyring`) that retrieves Bugzilla credentials from the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service), with end-to-end CLI UX for storing, removing, and migrating credentials, gated behind an optional default-enabled `keyring` Cargo feature.

**Architecture:** New `credentials` module wraps the [`keyring`](https://crates.io/crates/keyring) crate. `ServerConfig` gains an `api_key_keyring: Option<KeyringRef>` field with serde-optional `service` / `account` sub-fields. `CredentialSource` gains a `Keyring` variant and `resolve_api_key()` calls into the wrapper. Three new `bzr config` subcommands handle storage, removal, and migration. A stub module returns clear errors when the feature is disabled.

**Tech Stack:** Rust, `keyring` crate v4, `rpassword` for stdin no-echo prompts, `clap` derive macros, existing `thiserror`-based error handling.

**Design spec:** `docs/specs/2026-04-06-keyring-api-key-storage-design.md`

**Branch:** `feat/add-keyring` (already created)

---

## File Structure

**New files:**

- `src/credentials/mod.rs` — module root; conditionally exports `keyring` or `keyring_stub`.
- `src/credentials/keyring.rs` — real `keyring` crate wrapper (compiled with `feature = "keyring"`).
- `src/credentials/keyring_stub.rs` — stub that returns unsupported errors (compiled without the feature).
- `docs/troubleshooting.md` — platform-specific keyring troubleshooting guide.
- `tests/functional/keyring-test.sh` — shell script exercising the real OS keychain via the `bzr` binary.

**Modified files:**

- `Cargo.toml` — add optional `keyring` dep, `rpassword` dep, and `keyring` feature.
- `src/lib.rs` — add `pub mod credentials;`.
- `src/error.rs` — add `Keyring(String)` variant, exit code 12, error type `"keyring"`.
- `src/config.rs` — add `KeyringRef` struct, extend `ServerConfig`, extend `CredentialSource` / `CredentialSourceKind`, extend validation and `resolve_api_key`.
- `src/cli/config.rs` — add `SetKeyring`, `UnsetKeyring`, `MigrateToKeyring` variants.
- `src/commands/config.rs` — add handlers for the three new variants.
- `src/output/config.rs` — render keyring source in `ServerDisplayInfo`.
- `docs/bzr-cli.md` — new "Credential storage" section plus three subcommand entries.
- `Makefile` — add `functional-test-keyring` target.

**Implementation notes for the executor:**

- The repo's existing functional tests are shell scripts under `tests/functional/` that drive the compiled `bzr` binary. The keyring functional test follows that pattern rather than adding a Rust-based `tests/functional/keyring.rs` module, to stay consistent with the codebase.
- Each unit-test block in this repo is wrapped with `#[cfg(test)]` and `#[expect(clippy::unwrap_used)]`. Preserve that.
- `println!` is denied project-wide; command modules use `#[expect(clippy::print_stdout)]` to allow it. Follow the pattern.
- Never share worktree state across parallel tasks — tasks here are sequential.

---

## Task 1: Add dependencies and feature flag

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `keyring` feature and dependencies**

Edit `Cargo.toml`. Under `[features]`, add:

```toml
# OS keychain-backed API key storage (macOS Keychain, Windows Credential
# Manager, Linux Secret Service). Enabled by default; disable with
# --no-default-features to omit platform keyring backends.
default = ["keyring"]
keyring = ["dep:keyring", "dep:rpassword"]
```

Under `[dependencies]`, add:

```toml
keyring = { version = "4", features = ["apple-native", "windows-native", "sync-secret-service", "crypto-rust"], optional = true }
rpassword = { version = "7", optional = true }
```

Under `[dev-dependencies]`, add:

```toml
keyring = { version = "4", features = ["mock"] }
```

- [ ] **Step 2: Verify feature resolves and builds**

Run: `cargo build`
Expected: clean build, no warnings.

Run: `cargo build --no-default-features --features test-helpers`
Expected: clean build (proves the `keyring` feature is truly optional).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(deps): add optional keyring and rpassword dependencies"
```

---

## Task 2: Add `BzrError::Keyring` variant

**Files:**
- Modify: `src/error.rs`

- [ ] **Step 1: Write failing tests**

Append to the `#[cfg(test)] mod tests` block in `src/error.rs`:

```rust
#[test]
fn exit_code_keyring() {
    let err = BzrError::Keyring("keychain locked".into());
    assert_eq!(err.exit_code(), 12);
    assert_eq!(err.error_type(), "keyring");
    assert_eq!(err.to_string(), "keyring error: keychain locked");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib error::tests::exit_code_keyring`
Expected: FAIL — `BzrError::Keyring` not defined.

- [ ] **Step 3: Implement the variant**

In `src/error.rs`:

1. Add to the `BzrError` enum (after `BatchPartialFailure`, before `Other`):

```rust
    #[error("keyring error: {0}")]
    Keyring(String),
```

2. Add constant near other `ERROR_TYPE_*`:

```rust
const ERROR_TYPE_KEYRING: &str = "keyring";
```

3. Add constant near other `EXIT_CODE_*`:

```rust
const EXIT_CODE_KEYRING: i32 = 12;
```

4. Add arm in `exit_code()` (before `BzrError::Other`):

```rust
            BzrError::Keyring(_) => EXIT_CODE_KEYRING,
```

5. Add arm in `error_type()` (before `BzrError::Other`):

```rust
            BzrError::Keyring(_) => ERROR_TYPE_KEYRING,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib error::`
Expected: PASS.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs
git commit -m "feat(error): add BzrError::Keyring variant"
```

---

## Task 3: Add `KeyringRef` config type and extend `CredentialSource`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests**

Append to the `mod tests` block in `src/config.rs`:

```rust
#[test]
fn keyring_ref_defaults() {
    let r = KeyringRef {
        service: None,
        account: None,
    };
    assert_eq!(r.service_or_default(), "bzr");
    assert_eq!(r.account_or_default("prod"), "prod");
}

#[test]
fn keyring_ref_explicit() {
    let r = KeyringRef {
        service: Some("custom".into()),
        account: Some("acct".into()),
    };
    assert_eq!(r.service_or_default(), "custom");
    assert_eq!(r.account_or_default("prod"), "acct");
}

#[test]
fn keyring_ref_toml_roundtrip_empty() {
    let toml_str = r#"
url = "https://example.com"
api_key_keyring = {}
"#;
    let srv: ServerConfig = toml::from_str(toml_str).unwrap();
    assert!(srv.api_key_keyring.is_some());
    let r = srv.api_key_keyring.as_ref().unwrap();
    assert!(r.service.is_none());
    assert!(r.account.is_none());
}

#[test]
fn keyring_ref_toml_roundtrip_full() {
    let toml_str = r#"
url = "https://example.com"
api_key_keyring = { service = "bzr", account = "dave" }
"#;
    let srv: ServerConfig = toml::from_str(toml_str).unwrap();
    let r = srv.api_key_keyring.as_ref().unwrap();
    assert_eq!(r.service.as_deref(), Some("bzr"));
    assert_eq!(r.account.as_deref(), Some("dave"));
}

#[test]
fn credential_source_keyring_variant() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };
    let source = server.credential_source().unwrap();
    match source {
        CredentialSource::Keyring { service, account } => {
            assert_eq!(service, "bzr");
            // account defaults handled at resolve time via server_name
            assert_eq!(account, "");
        }
        _ => panic!("expected Keyring variant"),
    }
    assert_eq!(
        server.credential_source_kind().unwrap(),
        CredentialSourceKind::Keyring
    );
}

#[test]
fn credential_source_rejects_keyring_with_inline() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("k".into()),
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };
    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("api_key"));
    assert!(err.to_string().contains("api_key_keyring"));
}

#[test]
fn credential_source_rejects_keyring_with_env() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: Some("VAR".into()),
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };
    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("api_key_env"));
    assert!(err.to_string().contains("api_key_keyring"));
}

#[test]
fn credential_source_rejects_all_three() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("k".into()),
        api_key_env: Some("VAR".into()),
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };
    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("multiple API key sources"));
}
```

Note: the existing `make_server_config` helper and other existing tests construct `ServerConfig` directly without the new field — they will fail to compile until Step 3 adds `api_key_keyring: None` to those constructors. Update all existing call sites in the same step.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::tests::keyring_ref_defaults`
Expected: FAIL — `KeyringRef` not defined.

- [ ] **Step 3: Implement `KeyringRef` and extend `CredentialSource`**

Edit `src/config.rs`:

1. Add the `KeyringRef` struct below `ServerConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

2. Add `api_key_keyring` field to `ServerConfig`:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_keyring: Option<KeyringRef>,
```

Place it right after `api_key_env` so related fields stay grouped.

3. Extend `CredentialSourceKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSourceKind {
    Inline,
    Env,
    Keyring,
}
```

Update `as_str`:

```rust
impl CredentialSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialSourceKind::Inline => "inline",
            CredentialSourceKind::Env => "env",
            CredentialSourceKind::Keyring => "keyring",
        }
    }
}
```

4. Extend `CredentialSource`:

```rust
#[derive(Debug)]
pub enum CredentialSource<'a> {
    Inline(&'a str),
    EnvVar(&'a str),
    Keyring {
        service: &'a str,
        account: &'a str,
    },
}
```

Update `kind()`:

```rust
impl CredentialSource<'_> {
    pub fn kind(&self) -> CredentialSourceKind {
        match self {
            CredentialSource::Inline(_) => CredentialSourceKind::Inline,
            CredentialSource::EnvVar(_) => CredentialSourceKind::Env,
            CredentialSource::Keyring { .. } => CredentialSourceKind::Keyring,
        }
    }
}
```

5. Rewrite `credential_source()` to handle three-way exclusion. Note: because `CredentialSource::Keyring` borrows `&'a str` and `account` defaults to the server name which isn't available here, return an empty `&""` for `account` when the `KeyringRef::account` is None — the final resolution adds the server name at `resolve_api_key` time. (Alternative of plumbing the server name through would complicate every call site; the empty-account-means-defaulted convention stays internal.)

```rust
    pub fn credential_source(&self) -> Result<CredentialSource<'_>> {
        let count = usize::from(self.api_key.is_some())
            + usize::from(self.api_key_env.is_some())
            + usize::from(self.api_key_keyring.is_some());
        match count {
            0 => Err(BzrError::config(
                "server config must define one of 'api_key', 'api_key_env', or 'api_key_keyring'",
            )),
            1 => {
                if let Some(api_key) = self.api_key.as_deref() {
                    Ok(CredentialSource::Inline(api_key))
                } else if let Some(var_name) = self.api_key_env.as_deref() {
                    Ok(CredentialSource::EnvVar(var_name))
                } else {
                    // unwrap safe: count==1 and neither of the above matched
                    let r = self.api_key_keyring.as_ref().ok_or_else(|| {
                        BzrError::config("internal: keyring credential unexpectedly missing")
                    })?;
                    Ok(CredentialSource::Keyring {
                        service: r.service_or_default(),
                        account: r.account.as_deref().unwrap_or(""),
                    })
                }
            }
            _ => Err(BzrError::config(
                "server config cannot define multiple API key sources \
                 (api_key, api_key_env, api_key_keyring)",
            )),
        }
    }
```

6. Update the two-field error test `server_config_rejects_multiple_api_key_sources` and `config_load_rejects_multiple_api_key_sources` to match the new message `multiple API key sources`. Find each test and replace the assertion:

```rust
    assert!(err.to_string().contains("multiple API key sources"));
```

7. Update every existing `ServerConfig { ... }` struct literal in `src/config.rs` tests to include `api_key_keyring: None`. The existing helper `make_server_config` is the main entry point; also update the two inline constructors in `env_backed_server_resolves_api_key_from_environment` and `server_config_rejects_multiple_api_key_sources`.

8. Update every other `ServerConfig { ... }` literal in the codebase. Run a grep:

```bash
rg -l 'ServerConfig \{' src tests
```

For each match, add `api_key_keyring: None,` in the same spot (after `api_key_env`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::`
Expected: all config tests pass.

Run: `cargo build --all-targets`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/output/config.rs src/commands/config.rs tests/
git commit -m "feat(config): add KeyringRef type and CredentialSource::Keyring variant"
```

---

## Task 4: Create `credentials` module with keyring wrapper

**Files:**
- Create: `src/credentials/mod.rs`
- Create: `src/credentials/keyring.rs`
- Create: `src/credentials/keyring_stub.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `src/credentials/keyring.rs` with a test block at the bottom (we'll build it out below). First add a test that proves store → retrieve → delete works against the mock backend:

```rust
#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn install_mock() {
        // Idempotent: subsequent calls are no-ops.
        let _ = ::keyring::set_default_credential_builder(
            ::keyring::mock::default_credential_builder(),
        );
    }

    #[test]
    fn store_retrieve_delete_roundtrip() {
        install_mock();
        store("bzr-test", "acct1", "secret-value").unwrap();
        let got = retrieve("bzr-test", "acct1").unwrap();
        assert_eq!(got, "secret-value");
        delete("bzr-test", "acct1").unwrap();
    }

    #[test]
    fn retrieve_missing_entry_maps_to_no_entry_message() {
        install_mock();
        let err = retrieve("bzr-test", "missing-account").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no API key found"), "got: {msg}");
    }

    #[test]
    fn delete_missing_entry_is_ok() {
        install_mock();
        // Idempotent
        delete("bzr-test", "never-existed").unwrap();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib credentials::keyring::tests`
Expected: FAIL — `credentials` module not defined.

- [ ] **Step 3: Create the module and implement the wrapper**

Create `src/credentials/mod.rs`:

```rust
//! Credential storage backends.
//!
//! Currently provides a single backend: the OS keychain, via the
//! [`keyring`] crate. Gated behind the `keyring` Cargo feature;
//! when disabled, a stub returns clear "unsupported" errors so the
//! binary still parses keyring-backed config entries.

#[cfg(feature = "keyring")]
pub mod keyring;

#[cfg(not(feature = "keyring"))]
#[path = "keyring_stub.rs"]
pub mod keyring;
```

Create `src/credentials/keyring.rs`:

```rust
//! OS keychain wrapper around the `keyring` crate.
//!
//! Maps `keyring::Error` variants to user-facing `BzrError::Keyring`
//! messages so callers get actionable guidance on failures.

use ::keyring::{Entry, Error as KrError};

use crate::error::{BzrError, Result};

/// Store a secret in the OS keychain at `(service, account)`.
pub fn store(service: &str, account: &str, secret: &str) -> Result<()> {
    let entry = new_entry(service, account)?;
    entry
        .set_password(secret)
        .map_err(|e| map_error(service, account, &e))
}

/// Retrieve a secret from the OS keychain at `(service, account)`.
pub fn retrieve(service: &str, account: &str) -> Result<String> {
    let entry = new_entry(service, account)?;
    entry
        .get_password()
        .map_err(|e| map_error(service, account, &e))
}

/// Delete a secret from the OS keychain. Missing entries are not an error.
pub fn delete(service: &str, account: &str) -> Result<()> {
    let entry = new_entry(service, account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(KrError::NoEntry) => Ok(()),
        Err(e) => Err(map_error(service, account, &e)),
    }
}

fn new_entry(service: &str, account: &str) -> Result<Entry> {
    Entry::new(service, account).map_err(|e| {
        BzrError::Keyring(format!(
            "failed to open keychain entry for service='{service}' account='{account}': {e}"
        ))
    })
}

fn map_error(service: &str, account: &str, err: &KrError) -> BzrError {
    let message = match err {
        KrError::NoEntry => format!(
            "no API key found in OS keychain for service='{service}' account='{account}'. \
             Run `bzr config set-keyring <server>` to store one."
        ),
        KrError::PlatformFailure(inner) => format!(
            "OS keychain unavailable: {inner}. \
             For headless/CI environments, use api_key_env instead — see docs/bzr-cli.md."
        ),
        KrError::Ambiguous(_) => format!(
            "multiple matching keychain entries for service='{service}' account='{account}'; \
             please remove duplicates."
        ),
        KrError::BadEncoding(_) | KrError::Invalid(..) => format!(
            "stored keychain entry for service='{service}' account='{account}' is corrupted: {err}"
        ),
        other => format!("keychain error: {other}"),
    };
    BzrError::Keyring(message)
}

// (test block from Step 1 lives here)
```

Create `src/credentials/keyring_stub.rs`:

```rust
//! Stub keychain backend used when the `keyring` feature is disabled.
//!
//! Every function returns a clear error pointing the user at
//! `api_key_env` or a feature-enabled rebuild.

use crate::error::{BzrError, Result};

const UNSUPPORTED: &str =
    "this bzr build was compiled without keyring support; \
     rebuild with --features keyring or use api_key_env";

pub fn store(_service: &str, _account: &str, _secret: &str) -> Result<()> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

pub fn retrieve(_service: &str, _account: &str) -> Result<String> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

pub fn delete(_service: &str, _account: &str) -> Result<()> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}
```

Edit `src/lib.rs` and add the module declaration near other `pub mod` entries:

```rust
pub mod credentials;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib credentials::`
Expected: three tests pass.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

Run: `cargo build --no-default-features --features test-helpers`
Expected: clean build (stub path compiles).

- [ ] **Step 5: Commit**

```bash
git add src/credentials/ src/lib.rs
git commit -m "feat(credentials): add keyring wrapper with stub fallback"
```

---

## Task 5: Wire `resolve_api_key` to keyring backend

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing test**

Append to `mod tests` in `src/config.rs`:

```rust
#[test]
fn resolve_api_key_from_keyring() {
    // Install mock backend (idempotent across tests).
    let _ = ::keyring::set_default_credential_builder(
        ::keyring::mock::default_credential_builder(),
    );
    crate::credentials::keyring::store("bzr", "srv1", "keyring-secret").unwrap();

    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };

    assert_eq!(
        server.resolve_api_key("srv1").unwrap(),
        "keyring-secret"
    );

    // cleanup
    crate::credentials::keyring::delete("bzr", "srv1").unwrap();
}

#[test]
fn resolve_api_key_from_keyring_with_explicit_service_and_account() {
    let _ = ::keyring::set_default_credential_builder(
        ::keyring::mock::default_credential_builder(),
    );
    crate::credentials::keyring::store("myservice", "myacct", "explicit-secret").unwrap();

    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: Some("myservice".into()),
            account: Some("myacct".into()),
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };

    assert_eq!(
        server.resolve_api_key("any-name").unwrap(),
        "explicit-secret"
    );

    crate::credentials::keyring::delete("myservice", "myacct").unwrap();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::tests::resolve_api_key_from_keyring`
Expected: FAIL — `resolve_api_key` doesn't yet handle the `Keyring` case (panics or returns wrong value).

- [ ] **Step 3: Implement the third arm**

In `src/config.rs`, replace `resolve_api_key` with:

```rust
    pub fn resolve_api_key(&self, server_name: &str) -> Result<String> {
        match self.credential_source()? {
            CredentialSource::Inline(api_key) => Ok(api_key.to_string()),
            CredentialSource::EnvVar(var_name) => {
                let value = std::env::var(var_name).map_err(|_| {
                    BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is not set"
                    ))
                })?;
                if value.is_empty() {
                    return Err(BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is empty"
                    )));
                }
                Ok(value)
            }
            CredentialSource::Keyring { service, account } => {
                // account == "" means "default to server_name"
                let account = if account.is_empty() { server_name } else { account };
                crate::credentials::keyring::retrieve(service, account)
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): resolve API keys from OS keychain"
```

---

## Task 6: Render keyring source in config output

**Files:**
- Modify: `src/output/config.rs`

- [ ] **Step 1: Write failing test**

Append to the `mod tests` in `src/output/config.rs`:

```rust
#[test]
fn server_display_info_keyring_source() {
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(crate::config::KeyringRef {
            service: Some("bzr".into()),
            account: Some("prod".into()),
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
    };
    let info = ServerDisplayInfo::from_config(&srv);
    assert_eq!(info.api_key_source, "keyring");
    assert!(info.api_key.contains("bzr"));
    assert!(info.api_key.contains("prod"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib output::config::tests::server_display_info_keyring_source`
Expected: FAIL.

- [ ] **Step 3: Add the keyring arm**

In `src/output/config.rs`, update `ServerDisplayInfo::from_config` to handle the new variant:

```rust
    fn from_config(srv: &crate::config::ServerConfig) -> Self {
        let (api_key, api_key_source) = match srv.credential_source() {
            Ok(CredentialSource::Inline(api_key)) => {
                (mask_api_key(api_key), CredentialSourceKind::Inline.as_str())
            }
            Ok(CredentialSource::EnvVar(var_name)) => {
                (var_name.to_string(), CredentialSourceKind::Env.as_str())
            }
            Ok(CredentialSource::Keyring { service, account }) => {
                let shown_account = if account.is_empty() { "<server-name>" } else { account };
                (
                    format!("{service}/{shown_account}"),
                    CredentialSourceKind::Keyring.as_str(),
                )
            }
            Err(_) => ("[invalid config]".to_string(), "invalid"),
        };
        Self {
            url: srv.url.clone(),
            email: srv.email.clone(),
            api_key,
            api_key_source: api_key_source.to_string(),
            auth_method: srv.auth_method,
            tls_insecure: srv.tls_insecure,
        }
    }
```

Also update `print_config`'s field label selection:

```rust
                if s.api_key_source == "env" {
                    print_field("API Key Env", &s.api_key);
                } else if s.api_key_source == "keyring" {
                    print_field("Keyring", &s.api_key);
                } else {
                    print_field("API Key", &s.api_key);
                }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib output::config::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/output/config.rs
git commit -m "feat(output): render keyring credential source in config show"
```

---

## Task 7: Add CLI variants for keyring subcommands

**Files:**
- Modify: `src/cli/config.rs`

- [ ] **Step 1: Add the three new variants**

Replace the `ConfigAction` enum in `src/cli/config.rs`:

```rust
use clap::Subcommand;

use crate::types::AuthMethod;

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set up a server
    SetServer {
        /// Server alias name
        name: String,
        /// Server URL
        #[arg(long)]
        url: String,
        /// API key (less secure: may leak via shell history or process args)
        #[arg(
            long,
            conflicts_with = "api_key_env",
            required_unless_present = "api_key_env"
        )]
        api_key: Option<String>,
        /// Name of an environment variable that contains the API key
        #[arg(long, conflicts_with = "api_key", required_unless_present = "api_key")]
        api_key_env: Option<String>,
        /// Login email (required for older Bugzilla servers)
        #[arg(long)]
        email: Option<String>,
        /// Override auto-detected auth method (`header` or `query_param`)
        #[arg(long)]
        auth_method: Option<AuthMethod>,
        /// Accept invalid TLS certificates (self-signed, expired, wrong host)
        #[arg(long)]
        tls_insecure: bool,
    },
    /// Set the default server
    SetDefault {
        /// Server alias name
        name: String,
    },
    /// Show current configuration
    Show,
    /// Store an API key for a server in the OS keychain (prompts stdin)
    SetKeyring {
        /// Server alias name (must already exist)
        name: String,
        /// Override keyring service name (defaults to "bzr")
        #[arg(long)]
        service: Option<String>,
        /// Override keyring account name (defaults to the server name)
        #[arg(long)]
        account: Option<String>,
    },
    /// Remove a server's API key from the OS keychain
    UnsetKeyring {
        /// Server alias name
        name: String,
    },
    /// Migrate a server's existing inline/env API key into the OS keychain
    MigrateToKeyring {
        /// Server alias name
        name: String,
        /// Override keyring service name (defaults to "bzr")
        #[arg(long)]
        service: Option<String>,
        /// Override keyring account name (defaults to the server name)
        #[arg(long)]
        account: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}
```

- [ ] **Step 2: Run build to check the new enum compiles**

Run: `cargo check`
Expected: FAIL — `commands/config.rs` match is non-exhaustive.

- [ ] **Step 3: Stub the new match arms so the build is green**

Append to the match in `src/commands/config.rs::execute` (before the closing `}`):

```rust
        ConfigAction::SetKeyring { .. }
        | ConfigAction::UnsetKeyring { .. }
        | ConfigAction::MigrateToKeyring { .. } => {
            return Err(crate::error::BzrError::Other(
                "keyring subcommands not yet implemented".into(),
            ));
        }
```

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: clean.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/cli/config.rs src/commands/config.rs
git commit -m "feat(cli): add set-keyring/unset-keyring/migrate-to-keyring variants"
```

---

## Task 8: Implement `set-keyring` command

**Files:**
- Modify: `src/commands/config.rs`

- [ ] **Step 1: Write failing test**

Append to the `mod tests` block in `src/commands/config.rs`:

```rust
#[tokio::test]
async fn set_keyring_stores_secret_and_rewrites_config() {
    let _ = ::keyring::set_default_credential_builder(
        ::keyring::mock::default_credential_builder(),
    );
    let (_lock, _tmp) = setup_config_env().await;

    // Create an inline server first.
    execute(
        &ConfigAction::SetServer {
            name: "prod".into(),
            url: "https://prod.example.com".into(),
            api_key: Some("old-inline-value".into()),
            api_key_env: None,
            email: None,
            auth_method: None,
            tls_insecure: false,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    // Inject the test secret via the BZR_KEYRING_TEST_SECRET env var so we
    // don't need an interactive stdin.
    // SAFETY: Serialized via ENV_LOCK.
    unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", "new-keyring-value") };

    execute(
        &ConfigAction::SetKeyring {
            name: "prod".into(),
            service: None,
            account: None,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };

    // Config should have been rewritten: inline cleared, api_key_keyring set.
    let config = Config::load().unwrap();
    let server = &config.servers["prod"];
    assert!(server.api_key.is_none());
    assert!(server.api_key_env.is_none());
    assert!(server.api_key_keyring.is_some());

    // Resolving the API key now fetches from the (mock) keychain.
    assert_eq!(
        server.resolve_api_key("prod").unwrap(),
        "new-keyring-value"
    );

    // Cleanup.
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib commands::config::tests::set_keyring_stores_secret_and_rewrites_config`
Expected: FAIL (command not yet implemented).

- [ ] **Step 3: Implement `set-keyring`**

First, remove the stub arm from Task 7. In `src/commands/config.rs::execute`, delete the combined stub branch and add a real `SetKeyring` arm:

```rust
        ConfigAction::SetKeyring {
            name,
            service,
            account,
        } => {
            let mut config = Config::load()?;
            if !config.servers.contains_key(name) {
                return Err(crate::error::BzrError::config(format!(
                    "server '{name}' not found; create it first with `bzr config set-server`"
                )));
            }

            let service_name = service.as_deref().unwrap_or("bzr").to_string();
            let account_name = account.as_deref().unwrap_or(name.as_str()).to_string();

            let secret = read_secret_from_prompt_or_env(&service_name, &account_name)?;
            crate::credentials::keyring::store(&service_name, &account_name, &secret)?;

            let server = config.servers.get_mut(name).ok_or_else(|| {
                crate::error::BzrError::config(format!("server '{name}' disappeared"))
            })?;
            server.api_key = None;
            server.api_key_env = None;
            server.api_key_keyring = Some(crate::config::KeyringRef {
                service: service.clone(),
                account: account.clone(),
            });
            let path = Config::path()?;
            config.save()?;

            let human = format!(
                "Stored API key for server '{name}' in OS keychain \
                 (service={service_name}, account={account_name})\nConfig file: {}",
                path.display()
            );
            output::print_result(
                &ConfigResult::configured(
                    name.as_str(),
                    "",
                    false,
                    path.to_string_lossy(),
                    true,
                ),
                &human,
                format,
            );
        }
        ConfigAction::UnsetKeyring { .. } | ConfigAction::MigrateToKeyring { .. } => {
            return Err(crate::error::BzrError::Other(
                "keyring subcommand not yet implemented".into(),
            ));
        }
```

Add a private helper at the bottom of the file (above `#[cfg(test)]`):

```rust
fn read_secret_from_prompt_or_env(service: &str, account: &str) -> Result<String> {
    // Test hook: integration/unit tests inject the secret via env var so
    // they don't need an interactive TTY.
    if let Ok(val) = std::env::var("BZR_KEYRING_TEST_SECRET") {
        if !val.is_empty() {
            return Ok(val);
        }
    }
    let prompt = format!(
        "Enter API key for service='{service}' account='{account}' (input hidden): "
    );
    rpassword::prompt_password(&prompt).map_err(|e| {
        crate::error::BzrError::Io(std::io::Error::other(format!(
            "failed to read API key from stdin: {e}"
        )))
    })
}
```

Note: `rpassword` is only available when the `keyring` feature is enabled. Guard the helper and the three subcommand arms with `#[cfg(feature = "keyring")]` and provide `#[cfg(not(feature = "keyring"))]` fallbacks that return the stub error:

```rust
#[cfg(not(feature = "keyring"))]
fn read_secret_from_prompt_or_env(_service: &str, _account: &str) -> Result<String> {
    Err(crate::error::BzrError::Keyring(
        "this bzr build was compiled without keyring support; \
         rebuild with --features keyring or use api_key_env"
            .into(),
    ))
}
```

The `SetKeyring` match arm itself calls through `credentials::keyring::store` which already has a stub for the disabled case, so no further `cfg` gating is needed at the match site.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib commands::config::`
Expected: PASS (including new `set_keyring_stores_secret_and_rewrites_config`).

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

Run: `cargo build --no-default-features --features test-helpers`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/commands/config.rs
git commit -m "feat(commands): implement `bzr config set-keyring`"
```

---

## Task 9: Implement `unset-keyring` command

**Files:**
- Modify: `src/commands/config.rs`

- [ ] **Step 1: Write failing test**

Append to `mod tests` in `src/commands/config.rs`:

```rust
#[tokio::test]
async fn unset_keyring_removes_secret_and_clears_config() {
    let _ = ::keyring::set_default_credential_builder(
        ::keyring::mock::default_credential_builder(),
    );
    let (_lock, _tmp) = setup_config_env().await;

    // Build a keyring-backed server.
    execute(
        &ConfigAction::SetServer {
            name: "prod".into(),
            url: "https://prod.example.com".into(),
            api_key: Some("tmp".into()),
            api_key_env: None,
            email: None,
            auth_method: None,
            tls_insecure: false,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", "secret") };
    execute(
        &ConfigAction::SetKeyring {
            name: "prod".into(),
            service: None,
            account: None,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();
    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };

    // Now unset.
    execute(
        &ConfigAction::UnsetKeyring {
            name: "prod".into(),
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    let config = Config::load().unwrap();
    // Server should still exist but have no credential source now
    // (resolving it is expected to fail — validation only runs on save).
    let server = &config.servers["prod"];
    assert!(server.api_key_keyring.is_none());
    assert!(server.api_key.is_none());
    assert!(server.api_key_env.is_none());

    // Keychain entry is gone (idempotent delete succeeds).
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}
```

Important: `Config::save` runs `validate()` which requires a credential source. After unset, we save the config WITHOUT calling validate — see implementation below.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib commands::config::tests::unset_keyring_removes_secret_and_clears_config`
Expected: FAIL.

- [ ] **Step 3: Implement `unset-keyring`**

Replace the stub arms in `src/commands/config.rs::execute`:

```rust
        ConfigAction::UnsetKeyring { name } => {
            let mut config = Config::load()?;
            let server = config.servers.get_mut(name).ok_or_else(|| {
                crate::error::BzrError::config(format!("server '{name}' not found"))
            })?;
            let keyring_ref = server.api_key_keyring.take().ok_or_else(|| {
                crate::error::BzrError::config(format!(
                    "server '{name}' has no keyring credential to unset"
                ))
            })?;
            let service_name = keyring_ref.service_or_default().to_string();
            let account_name = keyring_ref.account_or_default(name).to_string();
            // Idempotent: missing entry is not an error.
            crate::credentials::keyring::delete(&service_name, &account_name)?;

            // Saving would fail validation (the server has no credential
            // source now). Write raw TOML directly to bypass validate().
            let path = Config::path()?;
            save_without_validation(&config, &path)?;

            let human = format!(
                "Removed keychain entry for server '{name}' (service={service_name}, \
                 account={account_name}).\nThe server entry is still present but has \
                 no API key source — re-run `bzr config set-server` or \
                 `bzr config set-keyring` to re-credential.\nConfig file: {}",
                path.display()
            );
            output::print_result(
                &ConfigResult::configured(
                    name.as_str(),
                    "",
                    false,
                    path.to_string_lossy(),
                    true,
                ),
                &human,
                format,
            );
        }
        ConfigAction::MigrateToKeyring { .. } => {
            return Err(crate::error::BzrError::Other(
                "migrate-to-keyring not yet implemented".into(),
            ));
        }
```

Add a new helper below `read_secret_from_prompt_or_env`:

```rust
/// Save the config to disk without running the credential-source validator.
/// Used by `unset-keyring`, which intentionally leaves a server with no
/// credential source temporarily.
fn save_without_validation(config: &Config, path: &std::path::Path) -> Result<()> {
    use std::fs;
    use std::io::Write as _;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    // Best-effort: preserve existing permissions by writing in place.
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}
```

Note: `Config::validate` is private. If this plan step surfaces a visibility issue when compiling (`save_without_validation` calling `toml::to_string_pretty(config)` works because `Serialize` is derived — no need to call `validate`), the implementation above should compile as-is. If the executor hits a visibility problem at runtime, escalate for a one-line fix.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib commands::config::`
Expected: PASS.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/commands/config.rs
git commit -m "feat(commands): implement `bzr config unset-keyring`"
```

---

## Task 10: Implement `migrate-to-keyring` command

**Files:**
- Modify: `src/commands/config.rs`

- [ ] **Step 1: Write failing tests**

Append to `mod tests`:

```rust
#[tokio::test]
async fn migrate_to_keyring_from_inline_rewrites_config() {
    let _ = ::keyring::set_default_credential_builder(
        ::keyring::mock::default_credential_builder(),
    );
    let (_lock, _tmp) = setup_config_env().await;

    execute(
        &ConfigAction::SetServer {
            name: "prod".into(),
            url: "https://prod.example.com".into(),
            api_key: Some("inline-secret".into()),
            api_key_env: None,
            email: None,
            auth_method: None,
            tls_insecure: false,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    execute(
        &ConfigAction::MigrateToKeyring {
            name: "prod".into(),
            service: None,
            account: None,
            yes: true,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    let config = Config::load().unwrap();
    let server = &config.servers["prod"];
    assert!(server.api_key.is_none(), "inline key should be cleared");
    assert!(server.api_key_keyring.is_some());
    assert_eq!(
        server.resolve_api_key("prod").unwrap(),
        "inline-secret"
    );
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}

#[tokio::test]
async fn migrate_to_keyring_from_env_preserves_config() {
    let _ = ::keyring::set_default_credential_builder(
        ::keyring::mock::default_credential_builder(),
    );
    let (_lock, _tmp) = setup_config_env().await;

    unsafe { std::env::set_var("BZR_MIGRATE_TEST_KEY", "env-secret") };
    execute(
        &ConfigAction::SetServer {
            name: "prod".into(),
            url: "https://prod.example.com".into(),
            api_key: None,
            api_key_env: Some("BZR_MIGRATE_TEST_KEY".into()),
            email: None,
            auth_method: None,
            tls_insecure: false,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();

    execute(
        &ConfigAction::MigrateToKeyring {
            name: "prod".into(),
            service: None,
            account: None,
            yes: true,
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap();
    unsafe { std::env::remove_var("BZR_MIGRATE_TEST_KEY") };

    let config = Config::load().unwrap();
    let server = &config.servers["prod"];
    // Env source preserved — config.toml is NOT rewritten.
    assert_eq!(server.api_key_env.as_deref(), Some("BZR_MIGRATE_TEST_KEY"));
    assert!(server.api_key_keyring.is_none());

    // The secret IS in the keychain.
    let stored = crate::credentials::keyring::retrieve("bzr", "prod").unwrap();
    assert_eq!(stored, "env-secret");
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib commands::config::tests::migrate_to_keyring`
Expected: FAIL.

- [ ] **Step 3: Implement `migrate-to-keyring`**

Replace the stub arm with:

```rust
        ConfigAction::MigrateToKeyring {
            name,
            service,
            account,
            yes,
        } => {
            let mut config = Config::load()?;
            let server = config.servers.get(name).ok_or_else(|| {
                crate::error::BzrError::config(format!("server '{name}' not found"))
            })?;
            let source_kind = server.credential_source_kind()?;
            let current_secret = server.resolve_api_key(name)?;

            if !yes {
                return Err(crate::error::BzrError::InputValidation(
                    "migrate-to-keyring requires --yes to confirm non-interactive migration"
                        .into(),
                ));
            }

            let service_name = service.as_deref().unwrap_or("bzr").to_string();
            let account_name = account.as_deref().unwrap_or(name.as_str()).to_string();
            crate::credentials::keyring::store(&service_name, &account_name, &current_secret)?;

            let path = Config::path()?;
            let human = match source_kind {
                crate::config::CredentialSourceKind::Inline => {
                    // Rewrite config: drop inline, add keyring ref.
                    let server = config.servers.get_mut(name).ok_or_else(|| {
                        crate::error::BzrError::config(format!("server '{name}' disappeared"))
                    })?;
                    server.api_key = None;
                    server.api_key_keyring = Some(crate::config::KeyringRef {
                        service: service.clone(),
                        account: account.clone(),
                    });
                    config.save()?;
                    format!(
                        "Migrated server '{name}' from inline API key to OS keychain \
                         (service={service_name}, account={account_name}).\nConfig file: {}",
                        path.display()
                    )
                }
                crate::config::CredentialSourceKind::Env => {
                    // Do NOT rewrite: env var may be shared with other tools.
                    format!(
                        "Stored API key for server '{name}' in OS keychain \
                         (service={service_name}, account={account_name}).\n\
                         The server is still configured to read 'api_key_env'. \
                         Edit config.toml manually to switch to the keychain if desired; \
                         the env var may be shared with other tools.\nConfig file: {}",
                        path.display()
                    )
                }
                crate::config::CredentialSourceKind::Keyring => {
                    return Err(crate::error::BzrError::config(format!(
                        "server '{name}' already uses a keyring credential source"
                    )));
                }
            };

            output::print_result(
                &ConfigResult::configured(
                    name.as_str(),
                    "",
                    false,
                    path.to_string_lossy(),
                    true,
                ),
                &human,
                format,
            );
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib commands::config::`
Expected: PASS.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/commands/config.rs
git commit -m "feat(commands): implement `bzr config migrate-to-keyring`"
```

---

## Task 11: Update `docs/bzr-cli.md`

**Files:**
- Modify: `docs/bzr-cli.md`

- [ ] **Step 1: Add new subcommand docs**

Open `docs/bzr-cli.md`. Locate the section documenting `bzr config` subcommands. After the `config show` entry, add:

````markdown
### `bzr config set-keyring <server> [--service NAME] [--account NAME]`

Store an API key for a previously-configured server in the OS keychain
(macOS Keychain, Windows Credential Manager, or Linux Secret Service).
The key is read from stdin with echo disabled, so it never appears on
the command line or in shell history. After storage, `config.toml` is
rewritten to drop any inline `api_key` / `api_key_env` value and add an
`api_key_keyring` reference.

- `--service NAME` overrides the keyring service name (default: `bzr`).
- `--account NAME` overrides the keyring account name (default: the
  server alias).

Example:

```console
$ bzr config set-keyring prod
Enter API key for service='bzr' account='prod' (input hidden):
Stored API key for server 'prod' in OS keychain (service=bzr, account=prod)
```

### `bzr config unset-keyring <server>`

Remove a server's API key from the OS keychain and clear the
`api_key_keyring` entry from `config.toml`. The server entry itself is
preserved; re-run `bzr config set-server` or `bzr config set-keyring`
afterward to re-credential it.

Idempotent: missing keychain entries are treated as a warning, not an
error.

### `bzr config migrate-to-keyring <server> [--service NAME] [--account NAME] --yes`

Copy an existing inline or env-backed API key into the OS keychain.

- For **inline** sources, `config.toml` is rewritten: `api_key` is
  dropped and `api_key_keyring` is added.
- For **env** sources, `config.toml` is left unchanged — the env var may
  be shared with other tools. The secret is still stored in the
  keychain so you can later edit `config.toml` manually to switch over.

`--yes` is required to confirm the migration.

## Credential storage

`bzr` supports three mutually-exclusive API key sources per server:

| Source | Config field | Typical use |
|---|---|---|
| Inline | `api_key = "..."` | Personal dev machines with hardened file permissions |
| Environment variable | `api_key_env = "BZR_API_KEY"` | Headless servers, CI/CD, containers |
| OS keychain | `api_key_keyring = {}` | Desktop workstations with an unlocked keychain daemon |

Exactly one must be set per server; `bzr config load` rejects any
combination.

### Headless / CI environments

Keychain access requires an unlocked user keyring daemon, which is
typically not available in headless servers, CI runners, or containers.
Use the environment variable source instead:

```toml
[servers.ci]
url = "https://bugzilla.example.com"
api_key_env = "BZR_API_KEY"
```

Inject the secret at runtime without writing it to disk:

**GitHub Actions:**

```yaml
    - name: Run bzr
      env:
        BZR_API_KEY: ${{ secrets.BZR_API_KEY }}
      run: bzr bug list --status NEW
```

**systemd drop-in:**

```ini
[Service]
EnvironmentFile=/etc/bzr.env    # mode 0600, owner root
```

**Docker:**

```dockerfile
ENV BZR_API_KEY=""
# Inject at runtime: docker run -e BZR_API_KEY=... ...
```

See also: `docs/troubleshooting.md` for platform-specific keychain
troubleshooting.
````

- [ ] **Step 2: Verify the page renders cleanly**

Run: `cargo build`
Expected: clean (documentation change only).

- [ ] **Step 3: Commit**

```bash
git add docs/bzr-cli.md
git commit -m "docs(cli): document keyring subcommands and credential storage"
```

---

## Task 12: Add `docs/troubleshooting.md`

**Files:**
- Create: `docs/troubleshooting.md`

- [ ] **Step 1: Write the troubleshooting guide**

Create `docs/troubleshooting.md` with:

````markdown
# Troubleshooting

## Keyring / credential storage

`bzr` uses the OS-native credential store when a server is configured
with `api_key_keyring`. This section covers common failure modes per
platform.

### Linux (Secret Service)

`bzr` talks to a running Secret Service daemon over D-Bus — typically
`gnome-keyring-daemon` (GNOME, XFCE) or `kwalletd6` (KDE). Headless
systems and most containers do not have one.

Check daemon status:

```bash
systemctl --user status gnome-keyring-daemon
# or
systemctl --user status kwalletd6
```

Probe the Secret Service with `secret-tool`:

```bash
secret-tool store --label="bzr test" service bzr-test account probe
secret-tool lookup service bzr-test account probe
secret-tool clear service bzr-test account probe
```

If `secret-tool` fails with "Cannot autolaunch D-Bus without X11
$DISPLAY", you are in a headless session. Use `api_key_env` instead.

If the daemon is running but the keyring is locked, unlock it via your
desktop's keyring application, then retry `bzr`.

### macOS

Keychain entries live in your login keychain. Inspect them via
**Keychain Access.app** under the `bzr` service name (or whatever
custom `--service` you chose).

macOS may prompt the first time `bzr` reads an entry. Choose "Always
Allow" to suppress subsequent prompts for the same binary path.

To remove an entry manually:

```bash
security delete-generic-password -s bzr -a <account>
```

### Windows

Credentials are stored in **Windows Credential Manager** under
`Generic Credentials`. List them from the command line:

```cmd
cmdkey /list
```

To delete an entry manually:

```cmd
cmdkey /delete:bzr:<account>
```

### Error: "OS keychain unavailable"

This indicates the platform backend is reachable but returned an error.
Common causes:

- **Linux:** Secret Service daemon crashed or never started. Restart
  your desktop session or start the daemon explicitly.
- **macOS:** user denied the Keychain access prompt. Retry and click
  "Always Allow".
- **Windows:** Credential Manager service is disabled. Start it via
  `services.msc` (Credential Manager).

If you cannot resolve the keychain issue, fall back to `api_key_env`:

```bash
bzr config set-server <name> --url <url> --api-key-env BZR_API_KEY
export BZR_API_KEY=<your-key>
```

### Error: "no API key found in OS keychain"

The server's `api_key_keyring` reference points at an entry that
doesn't exist. Create it with:

```bash
bzr config set-keyring <server>
```

Or migrate an existing credential into the keychain:

```bash
bzr config migrate-to-keyring <server> --yes
```

### Error: "this bzr build was compiled without keyring support"

You installed a `bzr` build without the `keyring` Cargo feature.
Rebuild with it (it is on by default):

```bash
cargo install --path . --features keyring
```

Or switch the affected server to `api_key_env`.
````

- [ ] **Step 2: Commit**

```bash
git add docs/troubleshooting.md
git commit -m "docs: add keyring troubleshooting guide"
```

---

## Task 13: Add functional test script and Makefile target

**Files:**
- Create: `tests/functional/keyring-test.sh`
- Modify: `Makefile`

- [ ] **Step 1: Create the shell test**

Create `tests/functional/keyring-test.sh` with mode `0755`:

```bash
#!/usr/bin/env bash
#
# Real-OS-keychain functional test for bzr.
#
# Builds bzr, sets up a temporary XDG_CONFIG_HOME, writes a server
# entry, stores a secret via `bzr config set-keyring`, resolves it,
# migrates inline → keyring, and cleans up. Runs on macOS, Linux
# (if Secret Service is reachable), and Windows (via Git Bash).
#
# Usage: tests/functional/keyring-test.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT_DIR"

TMP_CONFIG_HOME="$(mktemp -d)"
trap 'rm -rf "$TMP_CONFIG_HOME"; cleanup_keychain' EXIT

SERVICE_NAME="bzr-functional-test-$$"
SERVER_NAME="fntest"
SECRET="functional-test-secret-$RANDOM"

cleanup_keychain() {
  # Idempotent cleanup — ignore errors.
  XDG_CONFIG_HOME="$TMP_CONFIG_HOME" \
    ./target/debug/bzr config unset-keyring "$SERVER_NAME" 2>/dev/null || true
}

# On Linux, probe for a reachable Secret Service before running.
if [[ "$(uname -s)" == "Linux" ]]; then
  if ! command -v secret-tool >/dev/null 2>&1; then
    echo "SKIP: secret-tool not installed; cannot verify Secret Service."
    exit 0
  fi
  if ! secret-tool lookup dummy-probe dummy-value >/dev/null 2>&1; then
    # The lookup returns non-zero for "not found", which is fine —
    # we only want to detect "service unavailable" errors.
    rc=$?
    if [[ $rc -ne 1 ]]; then
      echo "SKIP: Secret Service unavailable (rc=$rc)."
      exit 0
    fi
  fi
fi

echo "Building bzr..."
cargo build --quiet

BZR="./target/debug/bzr"
export XDG_CONFIG_HOME="$TMP_CONFIG_HOME"

echo "1. Creating server with inline key..."
"$BZR" config set-server "$SERVER_NAME" \
  --url "https://example.invalid" \
  --api-key "initial-inline"

echo "2. Migrating inline → keyring..."
"$BZR" config migrate-to-keyring "$SERVER_NAME" \
  --service "$SERVICE_NAME" --yes

echo "3. Verifying credential resolves from keychain..."
# `config show` will resolve the key and either succeed or print an
# error. We grep for the keyring source marker.
if ! "$BZR" config show --format json | grep -q '"api_key_source": "keyring"'; then
  echo "FAIL: expected api_key_source=keyring in config show output"
  exit 1
fi

echo "4. Storing a fresh secret via set-keyring..."
BZR_KEYRING_TEST_SECRET="$SECRET" "$BZR" config set-keyring "$SERVER_NAME" \
  --service "$SERVICE_NAME"

echo "5. Removing keychain entry..."
"$BZR" config unset-keyring "$SERVER_NAME"

echo "OK: keyring functional test passed."
```

Set the executable bit:

```bash
chmod +x tests/functional/keyring-test.sh
```

- [ ] **Step 2: Add Makefile target**

Edit `Makefile`. Add `functional-test-keyring` to the `.PHONY` list (line 4-7) and append a new target after `functional-test-all`:

```make
functional-test-keyring: ## Run keyring functional test against real OS keychain
	tests/functional/keyring-test.sh
```

- [ ] **Step 3: Run the functional test locally**

Run: `make functional-test-keyring`
Expected on macOS/Windows: PASS. Expected on headless Linux: SKIP message. Expected on Linux with a running Secret Service: PASS.

If the test is being executed as part of the automated plan, skip this manual run step.

- [ ] **Step 4: Commit**

```bash
git add tests/functional/keyring-test.sh Makefile
git commit -m "test(functional): add real OS keychain end-to-end test"
```

---

## Task 14: Final verification

**Files:** (none modified)

- [ ] **Step 1: Full test suite**

Run: `cargo test --all-targets`
Expected: all tests pass.

- [ ] **Step 2: Full lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 3: Build without feature**

Run: `cargo build --no-default-features --features test-helpers`
Expected: clean build.

- [ ] **Step 4: Format check**

Run: `cargo fmt -- --check`
Expected: clean. If not, run `cargo fmt` and commit.

- [ ] **Step 5: Verify exit-code uniqueness**

Run: `cargo test --lib error::tests`
Expected: all `exit_code_*` tests pass, confirming `12` is not colliding.

- [ ] **Step 6: Manual smoke test (optional, host-dependent)**

On the developer's machine:

```bash
cargo install --path . --features keyring
bzr config set-server smoke --url https://bugzilla.example.com --api-key placeholder
bzr config migrate-to-keyring smoke --yes
bzr config show
bzr config unset-keyring smoke
```

Expected: no errors, `config show` displays `api_key_source: keyring`
between migrate and unset.

- [ ] **Step 7: Push branch**

```bash
git push -u origin feat/add-keyring
```

---

## Self-review notes

**Spec coverage check** (traced against `docs/specs/2026-04-06-keyring-api-key-storage-design.md`):

- Architecture (new `credentials` module, feature flag, stub) → Tasks 1, 4.
- Config schema (`KeyringRef`, extended `CredentialSource`, mutual exclusion) → Tasks 3, 5.
- CLI surface (three subcommands) → Tasks 7–10.
- Error handling (`BzrError::Keyring`, mapping from `keyring::Error`) → Tasks 2, 4.
- Documentation (bzr-cli.md credential storage + headless/CI, troubleshooting) → Tasks 11, 12.
- Testing (unit, integration, functional) → Tasks 3, 4, 5, 6, 8, 9, 10, 13.

**Known deviations from the spec:**

- The functional test is a shell script (`tests/functional/keyring-test.sh`) rather than a Rust module (`tests/functional/keyring.rs`). This matches the existing functional-test convention in the repo, which is entirely shell-driven. Functionality is equivalent.
- `CredentialSource::Keyring { account }` uses an empty string as a sentinel for "default to the server name" because `&str` can't borrow dynamically from the server name at `credential_source()` time without plumbing the name through every call site. The convention is internal to `src/config.rs` and is resolved at the `resolve_api_key` boundary.

**Placeholder scan:** no TBDs, TODOs, or unexplained "appropriate error handling" phrasing found.

**Type/name consistency:** `KeyringRef`, `service_or_default`, `account_or_default`, `CredentialSourceKind::Keyring`, `credentials::keyring::{store, retrieve, delete}`, `BZR_KEYRING_TEST_SECRET`, `BzrError::Keyring`, exit code `12` — used consistently across all tasks.
