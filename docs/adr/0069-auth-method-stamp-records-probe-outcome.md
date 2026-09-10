# 0069 — The auth-method stamp records whether the method was probed, and `config show` surfaces it

- Status: Accepted
- Date: 2026-09-09
- Issue: #760
- Related: [0007](0007-json-output-schema-version-envelope.md),
  [0056](0056-differential-header-auth-verification.md),
  [0066](0066-stamp-and-redetect-stale-auth-method.md)

## Context

ADR 0066 added the `auth_method_source` stamp so a persisted `auth_method` is
re-detected exactly once instead of staying stale forever. It landed with two
gaps it named and deferred: the stamp records *that* a method was detected, but
nothing records *how well* (a transport fallback is indistinguishable from a
probe result), and nothing lets anyone see the stamp.

The first gap matters because the stamp's trust rule keys on the value.
`network_error_outcome` (`src/client/auth/mod.rs`) turns any non-TLS transport
error into `Ok(AuthMethod::Header)`, so a timeout during the probe returns the
same `Ok` as a server that genuinely answered with header auth. To decide
whether to stamp, `persist_detected_settings`
(`src/commands/runtime/shared/connection/detect.rs`) uses
`server_version.is_some()` as a proxy for "reached the server." That proxy is
wrong in both directions: a probed method whose version probe failed is left
unstamped (re-detected needlessly), and — the dangerous case — a fallback
`header` whose version probe succeeded is stamped `"differential-probe"` and
trusted forever, re-creating the exact stale-`header` state ADR 0066 exists to
end, on a narrower population.

The second gap matters because the three states behave differently on the next
connect — pinned is never re-detected, detected takes the cached path,
unstamped triggers a detection round — but `bzr config show` shows only the
method, so neither a user nor a bug report can tell which state a server is in.

## Decision

1. **Record the probe outcome on the detection result.** Add
   `auth_method_probed: bool` to `DetectedServerSettings`
   (`src/client/auth/mod.rs`). It is `true` only when the auth probe returned a
   real answer (`Authenticated`); `false` when the method is a transport
   fallback (`network_error_outcome`) or on the credentialless path
   (`auth_method: None`). `detect_auth_method` threads the outcome through a
   small private `DetectedAuthMethod { method, probed }` return so each
   `Authenticated` arm and both `network_error_outcome` arms set it.

2. **Gate the stamp on the probe outcome, not `server_version`.**
   `persist_detected_settings` writes `auth_method_source = "differential-probe"`
   only when `auth_method_probed` is `true`, and clears any pre-existing stamp
   when it persists an unprobed value — a transport fallback can never sit under
   a trusted marker, so the next connect retries detection. `server_version`
   stays the gate for `api_mode`/`server_version` persistence — a separate
   question this decision does not touch.

3. **Surface the provenance in `config show`.** `ServerDisplayInfo`
   (`src/output/resources/config.rs`) gains
   `auth_method_source: Option<AuthMethodSourceDisplay>` mapped from the
   persisted marker: `pinned`, `detected`, or `unstamped` (an absent or
   unrecognised marker both read as `unstamped`). It is present only when
   `auth_method` is present, shown as an `Auth Source` table field and an
   additive `auth_method_source` JSON field.

## Consequences

- The stamp now keys on the probe's actual outcome. A probed method is stamped
  whether or not the version probe succeeded (no more needless re-detection); a
  transport fallback is never stamped as probe-derived (the dangerous case is
  closed) and is retried on the next connect, exactly as ADR 0066 intends for an
  unreachable server.
- `DetectedServerSettings` is `pub` and `#[non_exhaustive]`, so the added field
  is not a breaking change for external constructors (there are none — it is
  crate-internal); every in-crate and test constructor gains the field.
- `bzr config show --json` gains an additive field. Per ADR 0007 that is a
  patch-level schema change, so `SCHEMA_VERSION` bumps 3.0.7 → 3.0.8.
  `--output ndjson` is unaffected (the field rides the same serialized value).
- No new published schema: `config show` has no schema file today (it is not in
  the `bzr schema` registry), and the issue asks only that the provenance be
  surfaced "if the schema allows it additively." There is no schema to update,
  so none is added.
- The `config show` provenance values (`pinned`/`detected`/`unstamped`) are
  display-only. The trust decision (`auth_method_is_trusted`) matches only the raw
  generation marker, so a config file edited to a display label (e.g.
  `auth_method_source = "detected"`) deserializes but is a cache miss — the next
  connect re-detects and re-stamps. The trust path is fail-closed, so the cost is one
  extra detection round-trip, never a trust violation; the label is documented as
  display-only in `docs/bzr-cli.md`.
- ADR 0066's two deferred residuals (the probed/fallback distinction and the
  `config show` surface) are resolved by this record; its residual bullets are
  amended to point here.

## Considered & rejected

- **Keep `server_version.is_some()` as the reachability proxy.** verified: it
  mis-stamps in both directions — a probed method with a failed version probe
  stays unstamped, and a fallback method with a succeeded version probe is
  stamped and trusted forever (the residual ADR 0066 records). The proxy
  conflates "the version endpoint answered" with "the auth method was
  genuinely determined," which are independent probe outcomes on the same host.
- **A typed `AuthMethodProvenance { Probed, Fallback }` enum on
  `DetectedServerSettings`.** judgment: the probe has exactly two outcomes for
  the method (real answer vs transport fallback) and the issue calls for a
  "flag"; a `bool` (`auth_method_probed`) names both states and avoids a
  two-variant enum that would only ever be `true`/`false` at the persistence
  seam.
- **Expose the raw `auth_method_source` string in `config show --json`.**
  judgment: the markers are internal generation strings (`differential-probe`);
  a stable three-way display enum (`pinned`/`detected`/`unstamped`) is the
  contract the issue names and insulates consumers from a future marker rename,
  at the cost of one mapping function.
- **Publish a new `config-display` JSON schema.** judgment: the issue asks only
  that the provenance be surfaced additively "if the schema allows it"; there is
  no existing schema to extend, and adding a published schema is scope the issue
  did not request.
