# Issue #380: Credentialless Read-Only Servers Design

## Context

`bzr` currently requires every named server and every ad-hoc `--server-url`
target to define exactly one API-key source. That blocks first-run exploration
of public Bugzilla servers even for read commands that Bugzilla can serve
anonymously.

Issue #380 asks for public/read-only access without credentials while keeping
write commands fail-fast and preserving existing auth/API-mode detection for
credentialed servers.

## Decision

Make credentials optional, not fake.

`ServerConfig` should allow zero or one credential source. Multiple sources
remain a configuration error. A new credential resolution path returns
`Option<String>`:

- `Some(key)` for inline, env-var, or keyring-backed credentials.
- `None` for credentialless named or inline servers.

Client construction accepts an optional API key. Request auth attachment becomes
a no-op when no credential is present. This keeps anonymous reads as real
anonymous HTTP/XML-RPC requests instead of sending an empty API key.

`--server-url <URL>` no longer requires `--server-api-key-env`. When
`--server-api-key-env` is supplied, it behaves as it does today. When omitted,
the inline server is credentialless and remains entirely ephemeral.

## Read vs Write Boundary

Read commands may connect without credentials and let the server decide whether
anonymous access is allowed:

- `bug list`, `bug view`, `bug search`, `bug history`
- `comment list`, `comment search-tags`
- `attachment list`, `attachment view`, `attachment download`
- `product list`, `product view`
- `component list`, `component view`
- `classification list`, `classification view`
- `field aliases`, `field list`
- `user search`
- `group view`, `group list-users`
- `query run`
- `server info`

Local-only commands keep their existing behavior and do not need credentials.

Mutation or identity commands require credentials before any network write or
auth-only probe:

- bug writes: `bug create`, `bug clone`, `bug update`, `bug resolve`,
  `bug close`, `bug reopen`, `bug dup`
- comment writes: `comment add`, `comment tag`
- attachment writes: `attachment upload`, `attachment update`
- admin writes: product, component, user, and group create/update plus group
  membership changes
- identity-derived reads: `bug my`, `whoami`

If a credentialless server is used with one of these commands, exit 3 with a
configuration/authentication message that names the command category and suggests
adding `api_key_env`, `api_key`, `api_key_keyring`, or using
`--server-api-key-env` for an ad-hoc run.

## Detection and Caching

Credentialed servers keep the current behavior:

- detect auth method;
- detect API mode/version;
- cache detected auth method and API mode/version for named servers;
- do not persist anything for inline servers.

Credentialless servers skip auth-method detection because there is no credential
to test. They still run API-mode/version detection where possible and cache only
the version/API-mode result for named servers. The cached `auth_method` remains
unset so a future credential added to the same server still goes through normal
auth-method detection.

If anonymous API-mode detection fails because the server requires auth even for
version probes, surface the existing request error. This is still useful:
credentialless access was attempted, and the server refused it.

## TLS and Inline Server Interaction

Credentialless access does not change TLS trust semantics. Named server TLS
settings are validated and applied exactly as today. Inline `--server-url`
continues to persist no detected settings or TOFU pins.

Issue #381 will add explicit TLS flags for ad-hoc inline servers. This issue
does not add those flags; it only ensures the credentialless inline path can use
the default TLS trust behavior already available.

## Files

- `src/config.rs`: allow zero credential sources, add optional credential
  resolution, and keep multiple-source errors unchanged.
- `src/http.rs`: make auth application accept optional credentials and skip auth
  when absent.
- `src/client/mod.rs`: carry optional credentials through `BugzillaClientConfig`
  and request construction.
- `src/client/auth/mod.rs`: keep auth-method detection credential-only.
- `src/client/version.rs`: ensure version/API-mode detection can run without an
  API key.
- `src/commands/runtime/inline_server.rs`: make the API-key env var optional on
  inline server state.
- `src/commands/runtime/shared.rs`: build credentialless clients for read
  commands, skip auth detection without credentials, and persist only API mode
  for credentialless named servers.
- `src/commands/mod.rs` and resource command modules: enforce the read/write
  credential boundary before write/auth-only commands.
- `src/cli/mod.rs`: remove the `requires = "server_api_key_env"` constraint
  from `--server-url` and update help text.
- `docs/bzr-cli.md`, `README.md`, and `CHANGELOG.md`: document read-only
  credentialless use and write-command failure behavior.

## Testing

Add failing-first tests before implementation:

- CLI parsing accepts `bzr --server-url https://bz.example.com bug view 1`
  without `--server-api-key-env`.
- CLI parsing still rejects `--server-api-key-env` without `--server-url`.
- `ServerConfig::validate` accepts zero credential sources and still rejects
  multiple sources.
- optional credential resolution returns `None` for credentialless config and
  errors clearly for missing/empty env vars when an env source is configured.
- `connect_and_configure` can build a read client for a credentialless named
  server and does not persist `auth_method`.
- inline credentialless `--server-url` connects without loading config and
  persists nothing.
- a representative read command (`bug view` or `server info`) sends no API-key
  header or query parameter when credentialless.
- `bug my` and `whoami` fail before the auth-only identity probe when no
  credential is available.
- representative write commands fail before any write request when no credential
  is available.
- `query run`, `user search`, and group read commands remain credentialless read
  paths when the server permits anonymous access.
- existing credentialed detection tests still pass and verify `auth_method`
  persists for credentialed named servers.

## Documentation

Docs should show both flows:

```bash
bzr config set-server public-bz --url https://bugzilla.example.org
bzr --server public-bz bug list --product Firefox --limit 10

bzr --server-url https://bugzilla.example.org bug view 12345
bzr --server-url https://bugzilla.example.org \
  --server-api-key-env BZR_API_KEY bug update 12345 --status RESOLVED
```

The credential behavior should be stated directly: reads may work without an API
key when the Bugzilla server allows anonymous access; writes and `whoami` require
credentials and fail fast before writing when no credential source is configured.

## Out of Scope

- Adding ad-hoc TLS flags for `--server-url`; issue #381 covers that.
- Adding anonymous fallback after credentialed auth fails. If a user configured
  a credential source, failures should remain visible.
- Retrying writes or changing write idempotency.
- Changing server-side Bugzilla permissions. Anonymous requests can still fail
  when the server requires authentication for that resource.
