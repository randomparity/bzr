# Auth-method provenance: separate a probed result from a transport fallback, and surface the stamp

- Issue: #760
- ADR: [0069](../../adr/0069-auth-method-stamp-records-probe-outcome.md)
- Base branch: `main`
- Routed review depth: iterating

## Goal

Close the two gaps ADR 0066 left in the `auth_method_source` stamp:

1. **Trustworthy** — a transport fallback during auth detection must be
   distinguishable from a probed result at the persistence layer, so the stamp
   is written only for a method the probe genuinely determined.
2. **Observable** — `bzr config show` must distinguish a server's `auth_method`
   provenance: `pinned`, `detected`, or `unstamped`.

## Architecture

Detection already distinguishes the two outcomes internally — the probe returns
`Authenticated(method)` for a real answer and hits `network_error_outcome` (a
transport fallback to `AuthMethod::Header`) for a non-TLS transport error — but
collapses them into a bare `Result<AuthMethod>`. The change threads that
distinction out: a small private `DetectedAuthMethod { method, probed }` carries
it from `detect_auth_method` to `DetectedServerSettings`, which gains a public
`auth_method_probed: bool`. The persistence glue gates the stamp on that flag
instead of on `server_version.is_some()`, and the `config show` writer maps the
persisted marker to a three-way display value.

## Components

### 1. `DetectedServerSettings` gains `auth_method_probed: bool`
(`src/client/auth/mod.rs`)

```rust
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DetectedServerSettings {
    pub auth_method: Option<AuthMethod>,
    pub api_mode: ApiMode,
    /// `Some` when the version endpoint responded successfully; `None` on
    /// transient failures. Callers should only persist `api_mode` and
    /// `server_version` when this is `Some`.
    pub server_version: Option<String>,
    /// Whether `auth_method` was genuinely probed — the auth probe returned a
    /// real answer — rather than a transport fallback to `AuthMethod::Header`.
    /// `false` on the credentialless path, where `auth_method` is `None` and the
    /// flag carries no meaning. The `auth_method_source` stamp is written only
    /// when this is `true` (ADR 0069).
    pub auth_method_probed: bool,
}
```

- `detect_server_settings` (credentialed) sets `auth_method_probed` from the
  probe outcome.
- `detect_server_settings_without_auth` (credentialless) sets it `false`.
- Every test constructor of the struct gains the field (3 in
  `src/commands/runtime/shared/connection/detect_tests.rs`).

### 2. `detect_auth_method` threads the outcome
(`src/client/auth/mod.rs`)

Introduce a private return type and change the two seams:

```rust
struct DetectedAuthMethod {
    method: AuthMethod,
    probed: bool,
}
```

- `detect_auth_method` returns `Result<DetectedAuthMethod>`.
  - Each `Authenticated(method)` arm (whoami, valid_login, and the
    `verify_header_auth_via_rest` header-preference path) returns
    `DetectedAuthMethod { method, probed: true }`.
  - Both `NetworkError(e)` arms return `network_error_outcome(e)`.
- `network_error_outcome` returns `Result<DetectedAuthMethod>` and its fallback
  arm yields `DetectedAuthMethod { method: AuthMethod::Header, probed: false }`
  (the TLS-cert error arm still returns `Err(BzrError::Http(e))` unchanged).
- `detect_server_settings` consumes it:
  `let detected = detect_auth_method(...).await?;` then builds the settings with
  `auth_method: Some(detected.method)` and `auth_method_probed: detected.probed`.

The module doc table (lines 30-35) already names the two outcomes; add one row
clarifying that the fallback is now recorded as `auth_method_probed: false`.

### 3. The stamp gates on `auth_method_probed`
(`src/commands/runtime/shared/connection/detect.rs`)

In `persist_detected_settings`, inside the `if let Some(auth_method) =
settings.auth_method` block, replace the `detection_reached_the_server(settings)`
gate with `settings.auth_method_probed`:

```rust
if let Some(auth_method) = settings.auth_method {
    warn_on_auth_method_change(server_name, srv.auth_method, auth_method);
    srv.auth_method = Some(auth_method);
    if settings.auth_method_probed {
        // Stamp the provenance only when the probe genuinely determined the
        // method; a transport fallback stays unstamped and is retried on the
        // next connect (ADR 0069).
        srv.auth_method_source = Some(AUTH_METHOD_SOURCE_DETECTED.to_owned());
    }
}
```

Delete `detection_reached_the_server` and its doc comment (the residual it
describes is resolved); its reasoning moves to the field's doc and ADR 0069.
The `if settings.server_version.is_some() { ... }` block that persists
`api_mode`/`server_version` is unchanged — `server_version` stays the gate for
that separate concern.

### 4. `config show` surfaces the provenance
(`src/output/resources/config.rs`)

Add a display enum and a mapper:

A `AuthMethodSourceDisplay` enum — `Pinned`/`Detected`/`Unstamped`,
`#[serde(rename_all = "snake_case")]` — with an `as_str` method, and an
`auth_source_display(auth_method: Option<AuthMethod>, source: Option<&str>) ->`
`Option<AuthMethodSourceDisplay>` mapper: `None` when there is no method to
attribute; `Pinned` for `AUTH_METHOD_SOURCE_PINNED`; `Detected` for
`AUTH_METHOD_SOURCE_DETECTED`; `Unstamped` for an absent or unrecognised marker.
Complete code in plan Task 3.

- `ServerDisplayInfo` gains
  `#[serde(skip_serializing_if = "Option::is_none")] auth_method_source:
  Option<AuthMethodSourceDisplay>`.
- `ServerDisplayInfo::from_config` sets it from
  `auth_source_display(srv.auth_method, srv.auth_method_source.as_deref())`.
- `write_server` prints `Auth Source: <as_str>` only when the field is present,
  immediately after the `Auth` field.
- Import `AUTH_METHOD_SOURCE_DETECTED`, `AUTH_METHOD_SOURCE_PINNED` from
  `crate::config`.

### 5. `SCHEMA_VERSION` bump
(`src/output/mod.rs`)

`3.0.7` → `3.0.8` (additive `--json` change per ADR 0007).

### 6. ADR 0066 residual amendment
(`docs/adr/0066-stamp-and-redetect-stale-auth-method.md`)

Amend the two deferred-residual bullets in `## Consequences` to point at the
resolution:
- The "Residual: a server that answers the auth probes but not `rest/version`
  …" bullet → note it is resolved by ADR 0069 (the stamp now gates on
  `auth_method_probed`).
- The "`bzr config show` is unchanged …" bullet → note `config show` now
  surfaces the provenance (ADR 0069).

Leave the rest of ADR 0066 untouched.

## Data flow

```
detect_auth_method
  ├─ Authenticated(method)          → DetectedAuthMethod { method, probed: true }
  ├─ NetworkError (non-TLS)         → network_error_outcome → { Header, probed: false }
  └─ NetworkError (TLS cert)        → Err(BzrError::Http)   [unchanged]
        │
        ▼
DetectedServerSettings { auth_method: Some(_), auth_method_probed, api_mode, server_version }
        │
        ▼  persist_detected_settings
  auth_method persisted always; auth_method_source = DETECTED iff auth_method_probed
  api_mode / server_version persisted iff server_version.is_some()   [unchanged]
        │
        ▼  config show
  ServerDisplayInfo.auth_method_source = pinned | detected | unstamped (iff auth_method present)
```

## Settled ambiguity

Issue acceptance criterion 2 — "a server that answers auth probes but not
`rest/version` is not stamped as probe-derived" — is read against the *dangerous*
case the issue and ADR 0066 both name: a method that is a **transport fallback**
(auth probe did not genuinely determine it) must never be stamped as
probe-derived, regardless of whether the version probe succeeded. A method the
auth probe **genuinely determined** is stamped, even if the version probe failed
— that is the point of keying on the probe outcome instead of the version proxy.
Note that this is narrower than criterion 2's literal wording: a server that
genuinely answers the auth probes but not `rest/version` *is* stamped under this
reading (it answers auth probes, so `auth_method_probed` is true); the criterion
is applied to the fallback case it exists to catch.
The spec means: `auth_method_source = "differential-probe"` is written iff
`auth_method_probed` is `true`.

## Threat model

The change moves what bzr trusts from its own cached detection; it does not move
what an untrusted actor can reach.

- **Boundary inventory.** No new trust boundary. The auth probes (whoami,
  valid_login) still send the API key under TLS exactly as before; the
  discriminator only records which outcome the probe produced. The `config show`
  writer still masks the API key (`mask_api_key`); the new `auth_method_source`
  value is `pinned`/`detected`/`unstamped`, a non-secret classification.
- **Actor model.** The local operator owns the config file; the Bugzilla server
  is untrusted (may be slow, unreachable, or a MITM, but TLS is enforced). The
  risk being managed is bzr over-trusting its own cached fallback, not an
  external attacker.
- **Control per boundary.** The discriminator is set only by the probe outcome —
  an existing distinction (`Authenticated` vs `NetworkError`) already made by the
  outcome enums. The stamp is written only when `auth_method_probed` is true —
  the new control. A wrong discriminator in either direction has no secret
  impact: marking a fallback as probed at worst sends the key via a method the
  server ignores (an auth failure the user sees, not a leak); marking a probed
  method as fallback only costs one extra re-detection.
- **Out of scope.** The fallback-to-`header` behaviour itself (ADR 0066), TLS /
  TOFU / pin-rotation handling, and the credentialless path are unchanged.

## Testing

- **Unit (wiremock), `src/client/auth/mod_tests.rs` / `mod_tests.rs`:**
  - probed method → `auth_method_probed: true`.
  - whoami transport error → `auth_method_probed: false`, method `Header`.
  - valid_login transport error (whoami 404 first) → `auth_method_probed: false`.
- **Unit, `src/commands/runtime/shared/connection/detect_tests.rs`:**
  - probed + version `Some` → stamped `differential-probe`.
  - probed + version `None` → stamped `differential-probe` (no longer skipped).
  - fallback + version `Some` → **not** stamped (the dangerous case, closed).
  - fallback + version `None` → not stamped (unchanged).
- **Unit, `src/output/resources/config_tests.rs`:**
  - `auth_source_display` maps pinned/detected/unstamped (None and unknown
    marker both → `unstamped`) and `None` method → `None`.
  - `config show --json` carries `auth_method_source` for each of the three
    states and omits it when `auth_method` is absent.
  - table render shows `Auth Source` for the three states.
- **Functional, `tests/functional/phases/01-config.sh`:** `config show --json`
  (and table) against crafted `--config` files showing `detected`, `pinned`,
  `unstamped`, and a credentialless server (no method → no `auth_method_source`).
- **Functional, `tests/functional/phases/02-server-auth.sh`:** keep the existing
  ADR 0066 stamp cases (detection stamps, unstamped re-detects, pin survives,
  credentialless unaffected); update the section comment that no longer says
  `config show` does not carry the stamp.

## Global Constraints

- MSRV 1.89.0 (`rust-toolchain.toml` / `rust-version`).
- Clippy pedantic with strict rules; `unwrap_used` denied, `expect_used` and
  `allow_attributes` warned. No new `#[expect(clippy::print_*)]` in `src/`.
- Runtime is `#[tokio::main(flavor = "current_thread")]`; `make check-no-spawn`
  forbids multi-threaded runtime assumptions or thread spawning.
- User-facing CLI output goes through `Writers` (`w.out`/`w.err`), never
  `println!`/`eprintln!`, in `src/`.
- Tests live in sibling `*_tests.rs` files (`make check-test-layout`); no inline
  `mod tests` in `src/`.
- `SCHEMA_VERSION` is the single source of truth for the `--json` envelope.

## Acceptance criteria → where satisfied

1. Fallback distinguishable from probed at the persistence layer, without the
   `server_version` proxy → Components 1-3 (`auth_method_probed`, gated stamp).
2. A fallback method is not stamped as probe-derived → Component 3 (gate on
   `auth_method_probed`) + the settled-ambiguity reading.
3. `config show` distinguishes pinned / detected / unstamped → Component 4.
4. Functional coverage for all three states, including the credentialless path
   → Testing (functional phases 01 and 02).
5. ADR 0066's residual note amended to point at the resolution → Component 6.
