# Plan: auth-method provenance (probe outcome + `config show` surface)

Goal: make a transport fallback during auth detection distinguishable from a probed
result at the persistence layer (so the stamp is written only for a genuinely
determined method), and surface the `auth_method` provenance in `bzr config show`.

Architecture: thread the probe outcome (probed vs transport fallback) out of
`detect_auth_method` into a new `auth_method_probed: bool` on `DetectedServerSettings`;
gate the `auth_method_source` stamp on that flag instead of `server_version`; map the
persisted marker to a three-way display value in `config show` and bump `SCHEMA_VERSION`.

Tech stack: Rust 1.89 (MSRV), tokio current-thread runtime, wiremock for unit HTTP
mocks, bash functional phases against a real Bugzilla container.

Spec: [2026-09-09-auth-method-provenance-design.md](../specs/2026-09-09-auth-method-provenance-design.md)
ADR: [0069](../../adr/0069-auth-method-stamp-records-probe-outcome.md)

## Global Constraints

- MSRV 1.89.0 (`rust-toolchain.toml` / `rust-version`); do not bump.
- Clippy pedantic, strict: `unwrap_used` denied, `expect_used`/`allow_attributes`
  warned. No new `#[expect(clippy::print_*)]` in `src/`.
- Runtime is `#[tokio::main(flavor = "current_thread")]`; `make check-no-spawn`
  forbids thread spawning / multi-threaded assumptions.
- User-facing CLI output goes through `Writers` (`w.out`/`w.err`), never
  `println!`/`eprintln!`, in `src/`.
- Tests live in sibling `*_tests.rs` files (`make check-test-layout`); no inline
  `mod tests` in `src/`. Sibling test files start with
  `#![expect(clippy::unwrap_used)]` when their tests trigger the lint.
- `SCHEMA_VERSION` (`src/output/mod.rs`) is the single source of truth for the
  `--json` envelope.
- `AuthMethod` is `#[derive(Copy)]`; `ServerConfig` and
  `DetectedServerSettings` are constructed in the named sites below.

## File map

| File | Action | Answerable for |
|---|---|---|
| `src/client/auth/mod.rs` | changed | `DetectedServerSettings.auth_method_probed`, `DetectedAuthMethod`, `detect_auth_method`/`network_error_outcome` threading |
| `src/client/auth/mod_tests.rs` | changed | unit tests for the probe-outcome threading |
| `src/commands/runtime/shared/connection/detect.rs` | changed | the stamp gate on `auth_method_probed`; `detection_reached_the_server` removed |
| `src/commands/runtime/shared/connection/detect_tests.rs` | changed | constructors gain the field; stamp-gate unit tests |
| `src/output/resources/config.rs` | changed | `AuthMethodSourceDisplay`, `auth_source_display`, `ServerDisplayInfo.auth_method_source`, `write_server` |
| `src/output/resources/config_tests.rs` | changed | display mapper + JSON + table tests |
| `src/output/mod.rs` | changed | `SCHEMA_VERSION` 3.0.7 → 3.0.8 |
| `docs/adr/0066-stamp-and-redetect-stale-auth-method.md` | changed | residual bullets point at ADR 0069 |
| `tests/functional/phases/01-config.sh` | changed | `config show` provenance display (3 states + credentialless) |
| `tests/functional/phases/02-server-auth.sh` | changed | section comment; stamp cases unchanged |

Expected implementation size: 200–260 changed lines (M) — derived from the file map
above: ~80 lines of production change (three source files + one constant), ~140 lines
of test change (three unit siblings + two functional phases), plus the small ADR-0066
amendment; design artifacts excluded.

## Task 1 — thread the probe outcome into `DetectedServerSettings`

Files: `src/client/auth/mod.rs`, `src/client/auth/mod_tests.rs`.

Interfaces:
- Consumes: existing `WhoamiOutcome`/`ValidLoginOutcome` enums,
  `network_error_outcome`, `detect_server_settings`, `detect_server_settings_without_auth`.
- Produces: `DetectedServerSettings.auth_method_probed: bool` (public), private
  `struct DetectedAuthMethod { method: AuthMethod, probed: bool }`, and
  `detect_auth_method(...) -> Result<DetectedAuthMethod>`. Task 2 relies on
  `settings.auth_method_probed`.

Verification:
- `Mode: focused-test` — contract: the detection result carries the probe outcome
  (probed for a real answer, fallback for a transport error). Test file:
  `src/client/auth/mod_tests.rs`. Red: tests reference `auth_method_probed`, which
  does not yet exist, so the crate fails to compile (the focused test target
  errors). Green: `make test-one T=auth_method_probed`.

Steps:
1. Add the field to `DetectedServerSettings` (after `server_version`):
   ```rust
   /// Whether `auth_method` was genuinely probed — the auth probe returned a real
   /// answer — rather than a transport fallback to `AuthMethod::Header`. `false` on
   /// the credentialless path, where `auth_method` is `None` and the flag carries no
   /// meaning. The `auth_method_source` stamp is written only when this is `true`
   /// (ADR 0069).
   pub auth_method_probed: bool,
   ```
2. Add the private return type beside `detect_auth_method`:
   ```rust
   /// The auth probe's answer and whether it was a genuine determination.
   struct DetectedAuthMethod {
       method: AuthMethod,
       /// `true` when a probe returned a real answer; `false` when the method is a
       /// transport fallback to [`AuthMethod::Header`].
       probed: bool,
   }
   ```
3. Change `detect_auth_method`'s signature to `-> Result<DetectedAuthMethod>` and its
   three probed returns to wrap in `DetectedAuthMethod { method, probed: true }` (the
   header-preference return uses `method: AuthMethod::Header`):
   ```rust
   WhoamiOutcome::Authenticated(method) => {
       return Ok(DetectedAuthMethod { method, probed: true });
   }
   ```
   ```rust
   // in the valid_login Authenticated arm:
   return Ok(DetectedAuthMethod { method: AuthMethod::Header, probed: true });
   // and:
   return Ok(DetectedAuthMethod { method, probed: true });
   ```
4. Change `network_error_outcome` to `-> Result<DetectedAuthMethod>`; its TLS-cert arm
   still returns `Err(BzrError::Http(e))`, and the fallback arm returns
   `Ok(DetectedAuthMethod { method: AuthMethod::Header, probed: false })`.
5. In `detect_server_settings`, consume the outcome:
   ```rust
   let detected = detect_auth_method(&http, url, api_key, email).await?;
   let (version, api_mode) = detect_version_and_mode(&http, url, api_key, detected.method).await?;
   // ... tracing::info! uses detected.method ...
   Ok(DetectedServerSettings {
       auth_method: Some(detected.method),
       api_mode,
       server_version: version,
       auth_method_probed: detected.probed,
   })
   ```
   (`detect_version_and_mode` takes the method by value; `AuthMethod` is `Copy`, so the
   later `Some(detected.method)` is a copy.)
6. In `detect_server_settings_without_auth`, set `auth_method_probed: false`.
7. Update the module doc table (lines ~30-35): add a row stating the transport
   fallback is recorded as `auth_method_probed: false`.
8. Write the unit tests in `mod_tests.rs`:
   - `probed_auth_method_sets_probed_flag`: whoami 200 with a valid body →
     `detect_server_settings` returns `auth_method_probed == true`.
   - `transport_fallback_clears_probed_flag`: whoami against an unreachable host
     (e.g. `http://127.0.0.1:1`) → `auth_method == Some(Header)` and
     `auth_method_probed == false`.
   - `valid_login_transport_error_clears_probed_flag`: whoami 404 (probe falls to
     valid_login) + valid_login against an unreachable host → `auth_method_probed == false`
     (covers the second `network_error_outcome` call site).
   Run `make test-one T=auth_method_probed`; expect the new tests to pass.
9. Run the guardrails: `cargo clippy --all-targets --features test-helpers -- -D warnings`
   (all existing `DetectedServerSettings` constructors in `detect_tests.rs` will now
   fail to compile — fix them in Task 2's step 1, or add `auth_method_probed: false`
   here to keep the tree green). Commit.

Acceptance: `detect_server_settings` returns `auth_method_probed` correctly for probed
and fallback outcomes; credentialless path sets `false`.

## Task 2 — gate the stamp on `auth_method_probed`

Files: `src/commands/runtime/shared/connection/detect.rs`,
`src/commands/runtime/shared/connection/detect_tests.rs`.

Interfaces:
- Consumes: `DetectedServerSettings.auth_method_probed` (Task 1),
  `AUTH_METHOD_SOURCE_DETECTED`.
- Produces: the stamp written iff `auth_method_probed`; `detection_reached_the_server`
  removed. No new public API.

Verification:
- `Mode: focused-test` — contract: the `auth_method_source` stamp is written iff the
  method was probed, independent of `server_version`. Test file:
  `src/commands/runtime/shared/connection/detect_tests.rs`. Red: before the gate change,
  a fallback method with `server_version: Some` is still stamped, so a new assertion
  (fallback + version → not stamped) fails. Green: `make test-one T=persist_detected`.

Steps:
1. Update the three existing `DetectedServerSettings` constructors in `detect_tests.rs`
   (lines ~43, 68, 112) to add `auth_method_probed:` with the value that matches each
   test's intent (the existing "unreachable server stays unstamped" case uses `false`;
   the "detection stamps" cases use `true`).
2. In `persist_detected_settings`, replace the `detection_reached_the_server(settings)`
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
3. Delete `detection_reached_the_server` and its doc comment (lines ~60-79); its
   reasoning now lives in the field's doc and ADR 0069.
4. Add/update unit tests in `detect_tests.rs`:
   - `probed_method_without_version_is_stamped`: settings with
     `auth_method_probed: true`, `server_version: None` → persisted
     `auth_method_source == Some(DETECTED)`.
   - `fallback_method_with_version_is_not_stamped`: settings with
     `auth_method_probed: false`, `server_version: Some("5.1")` →
     `auth_method_source == None` (the dangerous case, now closed).
   - Keep the existing `persist_detected_does_not_stamp_an_unreachable_server` case green.
5. Run `make test-one T=persist_detected`; expect green. Run
   `cargo clippy --all-targets --features test-helpers -- -D warnings`. Commit.

Acceptance: the stamp is written iff `auth_method_probed`; `server_version` no longer
gates the auth stamp (it still gates `api_mode`/`server_version` persistence, unchanged).

## Task 3 — surface the provenance in `config show`

Files: `src/output/resources/config.rs`, `src/output/resources/config_tests.rs`,
`src/output/mod.rs`.

Interfaces:
- Consumes: `ServerConfig.auth_method`/`auth_method_source`,
  `crate::config::{AUTH_METHOD_SOURCE_DETECTED, AUTH_METHOD_SOURCE_PINNED}`.
- Produces: `ServerDisplayInfo.auth_method_source: Option<AuthMethodSourceDisplay>`
  (serialized as `pinned`/`detected`/`unstamped`, omitted when `auth_method` is
  `None`); an `Auth Source` table field.

Verification:
- `Mode: focused-test` — contract: `config show --json` carries `auth_method_source`
  for each provenance state and omits it when there is no method. Test file:
  `src/output/resources/config_tests.rs`. Red: `auth_method_source` field does not exist
  yet, so a new JSON assertion fails to compile. Green:
  `make test-one T=auth_source_display`.

Steps:
1. Add the display enum and mapper in `config.rs`:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
   #[serde(rename_all = "snake_case")]
   enum AuthMethodSourceDisplay {
       Pinned,
       Detected,
       Unstamped,
   }

   impl AuthMethodSourceDisplay {
       fn as_str(self) -> &'static str {
           match self {
               Self::Pinned => "pinned",
               Self::Detected => "detected",
               Self::Unstamped => "unstamped",
           }
       }
   }

   /// Map a persisted `(auth_method, auth_method_source)` pair to its provenance for
   /// display. `None` when there is no method to attribute.
   fn auth_source_display(
       auth_method: Option<AuthMethod>,
       source: Option<&str>,
   ) -> Option<AuthMethodSourceDisplay> {
       let _ = auth_method?;
       match source {
           Some(AUTH_METHOD_SOURCE_PINNED) => Some(AuthMethodSourceDisplay::Pinned),
           Some(AUTH_METHOD_SOURCE_DETECTED) => Some(AuthMethodSourceDisplay::Detected),
           // Absent (pre-stamp) or an unrecognised marker: both re-detect, so both
           // read as unstamped.
           _ => Some(AuthMethodSourceDisplay::Unstamped),
       }
   }
   ```
   Add the import: `use crate::config::{AUTH_METHOD_SOURCE_DETECTED, AUTH_METHOD_SOURCE_PINNED};`
   (merge into the existing `use crate::config::{...}` line).
2. Add the field to `ServerDisplayInfo` (after `auth_method`):
   ```rust
   #[serde(skip_serializing_if = "Option::is_none")]
   auth_method_source: Option<AuthMethodSourceDisplay>,
   ```
3. In `ServerDisplayInfo::from_config`, set it (before the other fields that borrow
   `srv`, so order is not an issue — `srv.auth_method` is `Copy`):
   ```rust
   auth_method_source: auth_source_display(srv.auth_method, srv.auth_method_source.as_deref()),
   ```
4. In `write_server`, after the `Auth` field, print the source when present:
   ```rust
   write_field(out, "Auth", &auth_display(s.auth_method.as_ref()));
   if let Some(source) = s.auth_method_source {
       write_field(out, "Auth Source", source.as_str());
   }
   ```
5. Bump `SCHEMA_VERSION` from `3.0.7` to `3.0.8` in exactly these twelve files — one
   atomic change; a partial sweep fails the functional and skill tests. This mirrors the
   3.0.6 → 3.0.7 sweep in the 2026-09-07 bounded-response-body-reads plan:
   - `src/output/mod.rs` (the const)
   - `README.md`
   - `docs/bzr-cli.md`
   - `content/skills/bzr-reference/reference/commands.md`, `json-recipes.md`
   - `content/skills/bzr-dependency-analysis/scripts/collect.py`, `tests/test_collect.py`
     (all four occurrences, including the candidate list), `tests/fixtures/recording_runner.py`
   - `tests/functional/phases/08e-bugs-restricted-access.sh`, `18a-json-envelope.sh`,
     `18c-skills-install.sh`, `18d-dependency-analysis.sh`
   Elsewhere in `src/` and in `tests/integration.rs` the version is referenced via the
   `SCHEMA_VERSION` constant, not re-literalized, so those need no change. Verify the
   sweep left no straggler in the pin set: `rg -n '3.0.7' src/output/mod.rs README.md
   docs/bzr-cli.md content/skills tests/functional` → expect no match. A repository-wide
   grep does not work: this branch's own ADR/spec/plan narrate the 3.0.7 → 3.0.8
   transition, and `Cargo.lock` carries unrelated version strings.
6. Write unit tests in `config_tests.rs`:
   - `auth_source_display_maps_each_state`: pinned marker → `Pinned`; detected marker →
     `Detected`; `None` marker → `Unstamped`; unknown marker (`"from-the-future"`) →
     `Unstamped`; `auth_method: None` → `None`.
   - `config_show_json_carries_auth_method_source`: a `ServerConfig` with
     `auth_method: Some(Header)` + each of the three sources serializes to the matching
     `json["auth_method_source"]`; a config with `auth_method: None` omits the key
     (`json.get("auth_method_source").is_none()`).
   - `write_config_renders_auth_source`: table output contains `Auth Source: detected`
     for a detected server and no `Auth Source` line for a method-less server.
7. Run `make test-one T=auth_source_display`; expect green. Run
   `cargo clippy --all-targets --features test-helpers -- -D warnings`. Commit.

Acceptance: `config show` distinguishes pinned/detected/unstamped in table and JSON;
the JSON field is additive and `SCHEMA_VERSION` is 3.0.8.

## Task 4 — functional coverage

Files: `tests/functional/phases/01-config.sh`, `tests/functional/phases/02-server-auth.sh`.

Interfaces:
- Consumes: the `config show` table/JSON output (Task 3) and the stamp behaviour
  (Tasks 1-2). Uses the lib.sh helpers `run_bzr`, `assert_success`, `assert_json`,
  `assert_stdout_contains`, and the `--config` alternate-file pattern already used in
  `01-config.sh`.

Verification:
- `Mode: focused-test` — contract: against a real container, `config show` shows the
  three provenance states and a credentialless server shows none, and the live
  detection path still stamps a probed method. Test files: the two phase scripts. Red:
  before Task 3, `config show --json` has no `auth_method_source` key, so the new
  `assert_json` calls fail. Green: `make functional-test` (or
  `make functional-test-bz53`) runs the phase green.

Steps:
1. In `01-config.sh`, after the existing `config-show` case, add a self-contained
   block that writes three `--config` files (no network) and asserts the display:
   - a server with `auth_method = "header"` +
     `auth_method_source = "differential-probe"` →
     `assert_json '.servers.<n>.auth_method_source' 'detected'` and table
     `assert_stdout_contains 'Auth Source: detected'`.
   - a server with `auth_method = "header"` +
     `auth_method_source = "pinned"` → `auth_method_source == 'pinned'`.
   - a server with `auth_method = "header"` and no source →
     `auth_method_source == 'unstamped'`.
   - a credentialless server (url only, no api_key/auth_method) →
     `assert_json '.servers.<n>.auth_method' 'null'` and no `auth_method_source` key.
   Use the same `mktemp -d` + `--config "$DIR/file.toml"` + cleanup pattern as the
   existing `config-reads-an-alternate-config-file` case. Give each case a distinct
   semantic test id (e.g. `config-show-auth-source-detected`, `-pinned`, `-unstamped`,
   `-credentialless-none`).
2. In `02-server-auth.sh`, update the ADR-0066 section comment (lines ~238-240) that
   currently says `config show` "does not carry the stamp" — it now does. Keep the four
   existing stamp cases (`auth-method-stamp-written-by-detection`,
   `auth-method-unstamped-entry-redetects`, `auth-method-pin-survives-connect`,
   `auth-method-stamp-ignored-without-credentials`) unchanged; they still pass because
   the live happy path (probed) still stamps.
3. Run `make functional-test` (default version) and confirm the new cases pass. If
   Docker/podman is unavailable, record that in the PR body per AGENTS.md.

Acceptance: all three provenance states + the credentialless path are covered
functionally; the ADR-0066 functional cases still pass.

## Task 5 — amend ADR 0066's residual

Files: `docs/adr/0066-stamp-and-redetect-stale-auth-method.md`.

Interfaces: none (documentation only).

Verification:
- `Mode: task-test-not-applicable` — changed surface: two prose bullets in a merged ADR
  record. No executable consumer validates ADR prose, and there is no drift test over
  ADR-0066; the only check is that the residual bullets now name ADR 0069, which is a
  human-readable pointer, not machine-checkable structure.

Steps:
1. In `## Consequences`, amend the residual bullet that begins "Residual: a server that
   answers the auth probes but not `rest/version` …" to state it is resolved: the stamp
   now gates on `auth_method_probed` (ADR 0069), so a transport fallback is never stamped
   as probe-derived.
2. Amend the bullet "`bzr config show` is unchanged …" to state `config show` now
   surfaces the provenance (`Auth Source` field, `auth_method_source` JSON field) per
   ADR 0069.
3. Leave the rest of ADR 0066 untouched.
4. Run `make lint` (format + clippy + layout checks) to confirm the doc edit is clean.
   Commit.

Acceptance: both deferred residuals in ADR 0066 point at ADR 0069 as the resolution.

## Rollback

Each task is an independent commit. Reverting Task 5 (docs) is trivial. Reverting Task 3
removes the display field and restores `SCHEMA_VERSION` 3.0.7. Reverting Task 2 restores
the `server_version` gate. Reverting Task 1 removes the field and reverts
`detect_auth_method` to `Result<AuthMethod>`. No migration or persisted-data change:
`auth_method_source` already exists and its meaning is unchanged; only when it is written
and how it is displayed change.
