# ADR 0066: A provenance stamp decides when a persisted `auth_method` is re-detected

## Status

Accepted

## Context

`auth_method` is detected once per server and cached in `config.toml`. `connect_and_configure`
(`src/commands/runtime/shared/connection/mod.rs`) returns straight from the
`(Some(method), Some(mode))` arm, and `resolve_config_target`
(`.../connection/target.rs`) reads the method from config, so nothing re-runs detection
once a value is persisted.

Issue #713 (ADR 0056) replaced the any-2xx header probe with a differential probe, which
changed the answer for stock Bugzilla 5.0/5.2 from `header` to `query_param`. A config
written before that fix keeps `header` indefinitely. ADR 0059 measured the result against
merged `main`: `bzr comment list` returned 3 of 5 comments and 0 of 2 private ones, exit 0,
no diagnostic. ADR 0056 recorded the residual and declined the fix because it needs a
persisted-config change. This record takes that decision.

The field is overloaded. `bzr config set-server --auth-method <method>` writes the same
`auth_method` key (`src/commands/config/set_server.rs`), and ADR 0059 documents a pinned
`header` as a legitimate configuration. Nothing today distinguishes a value detection chose
from one a user pinned, so no rule keyed on the value alone can invalidate one without
destroying the other.

## Decision

Add one optional `ServerConfig` field, `auth_method_source`, recording where the persisted
`auth_method` came from. It holds `"differential-probe"` when detection wrote it and
`"pinned"` when `--auth-method` did. `auth_method` is trusted only while
`auth_method_source` matches a currently-known marker; anything else — an absent stamp, or a
marker this build does not know — is a cache miss, and `resolve_config_target` reports no
cached auth. The existing `_` arm of `connect_and_configure` then re-detects and persists,
stamping the current marker, so the miss costs one detection round per server, once.

The stamp is `Option<String>` compared against constants, matching
`server_extensions_known` (`src/commands/runtime/shared/capability.rs`), which already
invalidates a detection cache by comparing persisted strings against what this build knows.

When re-detection changes the persisted value, `persist_detected_settings` logs one `warn`
naming the old method, the new one, and how to pin the old one back. That is the signal the
issue reports as missing. It describes the pin rather than printing a runnable command:
`set_server::handle` builds a fresh `ServerConfig` and inserts it
(`src/commands/config/set_server.rs`), so a command short enough to log would drop the
server's credential, email and TLS settings if pasted.

The stamp is written only when detection reached the server. `detect_auth_method` does not
fail on an unreachable one: `network_error_outcome` (`src/client/auth/mod.rs`) turns any
non-TLS transport error into `Ok(AuthMethod::Header)`, so a timeout during the probe is
indistinguishable from a real answer at the persistence layer. Stamping that fallback would
mark a guess as probe-derived and trust it permanently — the stale-`header` state this record
exists to end, re-created by the correction. `server_version.is_some()` is the local signal
for reachability, and the same field already gates `api_mode` persistence for the same reason.

Re-detection triggered *only* by a missing stamp persists best-effort. Such a server connected
without touching the config file before this record, so a config directory that cannot be
written must not become a hard failure on the first post-upgrade connect: the detected values
are usable in memory and the cost of not recording them is one more detection next run. A
genuinely uncached server still fails, because there the write is how the setting survives.

## Consequences

- The affected population is corrected on its next credentialed connect, without the user
  knowing to re-run anything. The cost is one detection round per server, once; every later
  connect takes the cached path unchanged.
- A `--auth-method` pin written *before* this record is indistinguishable from a stale
  detected value and is re-detected once. The `warn` names the change and how to restore
  the pin. Pins written from here on carry `"pinned"` and are never re-detected.
- Credentialless servers are unaffected: `resolve_config_target` reads `auth_method` only
  when an API key resolves, so a config with no credential never reaches the stamp check.
- On that one re-detection, a TLS or auth failure surfaces as an error instead of a silently
  stale success. The cached path already contacts the server (`probe_cached_connection`),
  so this adds no new network dependency, only a louder failure on the transition. A
  transport failure does not surface, by design: it leaves the entry unstamped, so the
  correction is retried rather than resolved wrongly.
- Residual (resolved by ADR 0069): a server whose auth probes fell back to the transport
  default while `rest/version` still succeeded was stamped with the fallback method and
  trusted forever. The stamp now gates on `DetectedServerSettings.auth_method_probed`, so
  a transport fallback is never stamped as probe-derived and is retried on the next connect.
- (Amended by ADR 0069.) `bzr config show` now surfaces the provenance: its
  `ServerDisplayInfo` (`src/output/resources/config.rs`) carries an `auth_method_source`
  JSON field and an `Auth Source` table field, mapped to `pinned`/`detected`/`unstamped`.
- A future detection change reuses the mechanism by adding a marker constant; the previous
  marker stops matching and that population re-detects once.

## Considered & rejected

- **Treat `auth_method` as always-derived — detect every invocation, persist nothing.**
  verified: every network command routes through `connect_and_configure`, whose cached arm
  issues no auth probe at all; `detect_auth_method` (`src/client/auth/mod.rs`, merged `main`
  @ `3f7b2b5e`) costs a `whoami` probe or a two-leg `valid_login` plus the differential
  comparison. judgment: paying that on every invocation forever to correct a one-time
  upgrade transition is the wrong trade.
- **Provide an explicit refresh path only** (a `config refresh-detection` command or flag).
  judgment: the issue states most users will never run it, so the affected population — whose
  only symptom is missing data — stays broken by default, which is the defect being fixed.
  It also adds CLI surface that nothing triggers.
- **A typed `enum AuthMethodSource` instead of `Option<String>`.** verified: `AuthMethod`
  and `ApiMode` are plain `#[serde(rename_all = "snake_case")]` unit enums
  (`src/types/transport.rs`) with no unknown-value fallback, and `Config::load_at` →
  `read_unvalidated_at` propagates any deserialize error as `BzrError::TomlParse`
  (`src/config/store.rs:79-90`), so a marker written by a future bzr would make the whole
  config file unreadable to an older one — every command, not just this one. `#[serde(other)]`
  is unavailable for a plain string-deserialized enum. judgment: tolerating an unknown value
  is this field's core requirement, since it exists to arbitrate across bzr versions.
- **Stamp the bzr version that wrote the value and compare versions.** judgment: it ties
  cache validity to release numbering rather than to the probe that produced the value, so
  every release would have to remember whether detection changed; a generation marker names
  the thing that actually changed.
- **Re-detect on every cache hit whose value is `header`, with no schema change.** verified:
  `--auth-method` is an existing supported override (`src/cli/config.rs`,
  `docs/bzr-cli.md:2065`) and ADR 0059 records a pinned `header` as legitimate. judgment: a
  value-triggered rule cannot tell a pin from a stale detection, so it would re-probe and
  overwrite the pin on every connect.
- **Do nothing — keep ADR 0056's "re-run the whole `config set-server` line" remedy.**
  verified: ADR 0059 measured that population against merged `main` and found 3 of 5
  comments, 0 of 2 private, exit 0, no diagnostic. judgment: a remedy the user must first
  know to look for does not reach a population whose only symptom is absent data.
