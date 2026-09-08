# Issue #738 — re-detect a persisted `auth_method` written before the differential probe

Decision record: [ADR 0066](../../adr/0066-stamp-and-redetect-stale-auth-method.md).
Background: [ADR 0056](../../adr/0056-differential-header-auth-verification.md),
[ADR 0059](../../adr/0059-auth-method-governs-private-content-completeness.md).

## Problem

`auth_method` is detected once and cached per server in `config.toml`, and nothing re-runs
detection afterwards. Issue #713 changed what detection produces for stock Bugzilla 5.0/5.2
(`header` → `query_param`), so a config written before that fix keeps a value the server
ignores. ADR 0059 measured the effect on merged `main`: `comment list` returns 3 of 5
comments and 0 of 2 private, exit 0, no diagnostic. Upgrading does not fix it and nothing
tells the user anything is wrong.

`config set-server --auth-method` writes the same field, so any invalidation rule keyed on
the *value* would also destroy a deliberate pin.

## Scope

Add `ServerConfig::auth_method_source: Option<String>` recording the provenance of the
persisted `auth_method`, plus two marker constants and a
`ServerConfig::auth_method_is_trusted()` predicate. `resolve_config_target` reports cached
auth only when the predicate holds; an absent or unrecognised marker is a cache miss, which
routes the existing `_` arm of `connect_and_configure` into re-detection and re-persistence.
`persist_detected_settings` writes the `"differential-probe"` marker and, when re-detection
changed the value, logs one `warn` naming the old method, the new method and the
`--auth-method` command that pins it back. `set_server` writes `"pinned"` when
`--auth-method` was given.

Not in scope, with owners: a `--server-auth-method` override for the inline-server surface
(future issue); the alternate-auth retry in `src/client/transport.rs` (issue #715); the
differential-probe detection logic itself (ADR 0056 / #713); `src/output/**` (issue #743);
`src/client/**`, `src/http.rs`, `src/xmlrpc/**` (issue #740). No new CLI flag, no new
command, no `docs/bzr-cli.md` command-tree change.

## Success

1. A server entry carrying `auth_method` with no `auth_method_source` re-detects on its next
   credentialed connect and persists both the fresh method and the
   `"differential-probe"` marker.
2. A server entry carrying `auth_method` with the `"differential-probe"` or `"pinned"`
   marker takes the cached path and issues no auth probe.
3. A server entry carrying an unrecognised `auth_method_source` value loads without error and
   is treated as a cache miss.
4. `config set-server --auth-method <m>` persists `auth_method_source = "pinned"`, and a
   later connect neither re-detects nor overwrites it.
5. A credentialless server entry is unaffected by the stamp on every path.
6. Re-detection that changes the persisted value emits exactly one `warn` naming both
   methods and the pin command; re-detection that confirms the value emits none.
7. `make lint`, `make test`, and `make functional-test` are green.

## Validation

- **`auth_method_is_trusted()` over absent, known, and unknown markers** —
  Mode: focused-test. `src/config/model_tests.rs`; new cases assert `false` for `None`,
  `false` for `Some("something-else")`, `true` for each known constant. Red before the
  predicate exists (no such method). Green:
  `make test-one T=auth_method_is_trusted`.
- **An unknown marker string deserializes rather than failing the load** —
  Mode: focused-test. `src/config/model_tests.rs`; round-trips a TOML server entry with
  `auth_method_source = "from-the-future"` through `toml::from_str` and asserts `Ok`. Red
  before the field exists (unknown-key handling differs). Green:
  `make test-one T=auth_method_source`.
- **`resolve_config_target` reports no cached auth for an unstamped entry, and cached auth
  for a stamped one** — Mode: focused-test.
  `src/commands/runtime/shared/connection/target_tests.rs`; two cases over a credentialed
  `ServerConfig` differing only in `auth_method_source`, asserting `cached_auth` is `None`
  then `Some`. Red before the gate exists (both return `Some`). Green:
  `make test-one T=cached_auth`.
- **`persist_detected_settings` stamps the marker and warns only on a changed value** —
  Mode: focused-test. `src/commands/runtime/shared/connection/detect_tests.rs`; cases over a
  temp config asserting the persisted `auth_method_source`, and asserting the warn fires for
  `header` → `query_param` and not for `header` → `header`, via a `tracing` capture layer.
  Red before the stamp is written. Green: `make test-one T=persist_detected`.
- **`set_server` stamps `"pinned"` for `--auth-method` and leaves it `None` otherwise** —
  Mode: focused-test. `src/commands/config/set_server_tests.rs`; extends the existing
  `auth_method` case. Red before the stamp is written. Green:
  `make test-one T=set_server`.
- **The end-to-end re-detection trigger against a real container** — Mode: focused-test.
  `tests/functional/phases/02-server-auth.sh`; cases covering success criteria 1, 2, 4 and 5
  by editing `$XDG_CONFIG_HOME/bzr/config.toml` directly and reading it back, with the
  method-value assertion version-gated to `bz50`/`bz52` as the existing ADR 0056 case is.
  Red before the trigger exists (no re-detection observed). Green: `make functional-test`.
- **`docs/bzr-cli.md` `--auth-method` prose** — Mode: task-test-not-applicable. The changed
  surface is one descriptive paragraph about when a cached value is re-probed; no executable
  consumer reads it, and the repository's `flag-drift-check.sh` verifies the command tree and
  flag list, neither of which this paragraph changes.
