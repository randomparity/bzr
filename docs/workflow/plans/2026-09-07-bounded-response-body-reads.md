# Plan: bounded response-body reads

**Goal.** Stop `bzr` buffering an unbounded HTTP response body: route all
thirteen `Response::text()` sites through one helper that refuses past 64 MiB,
and report the refusal as a new error with its own exit code.

**Architecture.** `src/http.rs` gains `MAX_RESPONSE_BODY_BYTES`, a
`BodyReadError` enum, and `read_body_bounded`, which streams
`reqwest::Response::chunk()` into a `Vec<u8>` and refuses as soon as the total
would exceed the limit. Seven call sites propagate the refusal as
`BzrError::ResponseTooLarge` (exit 16); six probe sites take the degraded path
they already take for an unreadable body. `schemas/error.json`, the
`SCHEMA_VERSION` pin set, and `docs/bzr-cli.md` follow the new exit code.

**Tech stack.** Rust 2021, `tokio`, `reqwest` 0.12.28 (`default-features =
false`, features `["json", "rustls-tls-native-roots"]`), `thiserror`,
`serde_json`, `wiremock` for unit-level HTTP, bash + python3 for functional
phases.

Design: `docs/workflow/specs/2026-09-07-bounded-response-body-reads-design.md`,
which carries the site inventory, the threat model, and why 64 MiB.
Decision: `docs/adr/0068-response-bodies-are-read-under-one-shared-bound.md`.

Expected implementation size: 210–290 changed lines (M) — from the file map
below: the helper (~60), thirteen call-site swaps plus six degraded arms (~60),
the error variant across four match arms (~25), sibling tests (~70),
schema/docs/pin edits (~50), functional fixture (~45).

## Global constraints

- **Branch** `feat/bound-response-body-reads-740`, base branch `main`.
- **Guardrails, run bare** (no `| tail`, no `>/dev/null`, no `|| true`):
  `make lint`, `make test`, `make skills-test`, `make functional-test`.
  Iterate with `make test-one T=<substr>` and `make test-fast`. Never run bare
  `cargo test`.
- **No inline `mod tests` in `src/`.** Unit tests live in a sibling
  `<name>_tests.rs` linked by `#[cfg(test)] #[path = "<name>_tests.rs"] mod
  tests;`; `make check-test-layout` enforces it. Siblings open with the
  file-level attribute their tests need (`#![expect(clippy::unwrap_used)]`).
- **Clippy pedantic, warnings are errors**, `unwrap_used` denied in `src/`; no
  `println!`/`eprintln!` there — diagnostics go through `tracing`.
- **`SCHEMA_VERSION` is `3.0.6` on main and becomes `3.0.7`**, across exactly
  the twelve pins listed in Task 4. The `3.0.5` strings in
  `docs/adr/0063-*.md` and `docs/adr/0065-*.md` narrate history inside Accepted
  records and must not be touched.
- **Do not edit** `docs/adr/README.md` (the orchestrator appends the index
  row), `tests/functional/run-tests.sh`, `tests/functional/lib.sh`, or the
  Makefile (another branch owns them). Phase 18b already exists and is already
  registered, so no runner edit is needed.
- **Public-write hygiene**: no absolute host paths, hostnames, usernames,
  emails, IPs, or credentials in commit messages, PR text, or docs.

## File map

| File | Change |
|---|---|
| `src/http.rs` | limit constant, `BodyReadError`, `From<BodyReadError> for BzrError`, `read_body_bounded`, `read_body_within` |
| `src/http_tests.rs` | bound, transport arm, constant |
| `src/error.rs` | `ResponseTooLarge` variant + four match arms |
| `src/error_tests.rs` | exit code, type, structured detail, transport classification |
| `src/main_tests.rs` | the new variant added to the published-schema case list |
| `src/client/response.rs` | five call sites |
| `src/client/transport.rs`, `src/client/version.rs`, `src/client/auth/whoami.rs` | one call site each, degraded |
| `src/client/auth/valid_login.rs` | three call sites (one propagating, two degraded) |
| `src/xmlrpc/protocol/client.rs` | two call sites (one propagating, one degraded) |
| `schemas/error.json` | `exit_code` maximum 16, two optional keys |
| `docs/bzr-cli.md` | exit-code row, structured-key rows, version pin |
| `src/output/mod.rs` + 10 further pin sites | `SCHEMA_VERSION` 3.0.7 |
| `tests/functional/phases/18b-http-error-preview.sh` | oversize-body fixture case |

## Task 1 — the bounded read helper and its error

Modifies `src/http.rs`, `src/http_tests.rs`, `src/error.rs`,
`src/error_tests.rs`. One commit: `src/http.rs` cannot compile without the
error variant, and lib-only clippy denies dead code, so the producer and its
only consumer land together.

**Interfaces produced.** Task 2 consumes exactly these:

```rust
// src/http.rs
pub(crate) const MAX_RESPONSE_BODY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum BodyReadError {
    TooLarge { operation: String, limit_bytes: u64 },
    Transport(reqwest::Error),
}

pub(crate) async fn read_body_bounded(
    response: reqwest::Response,
    operation: &str,
) -> std::result::Result<String, BodyReadError>;

// src/error.rs
BzrError::ResponseTooLarge { operation: String, limit_bytes: u64 }
```

**Interfaces consumed.** `crate::error::BzrError::Http(reqwest::Error)` and the
existing `EXIT_CODE_*` / `ERROR_TYPE_*` constant blocks in `src/error.rs`.

### Verification

All cases below are `focused-test`. Red before the code exists:
`error[E0425]: cannot find function read_body_within` for the first four,
`error[E0599]: no variant named ResponseTooLarge` for the last.

- A body over the limit is refused before it is fully buffered —
  `src/http_tests.rs::read_body_within_refuses_a_body_over_the_limit`, asserting
  `BodyReadError::TooLarge` carries the limit and the operation. Green:
  `make test-one T=read_body_within`.
- A body at or under the limit is returned unchanged —
  `read_body_within_returns_a_body_at_the_limit` and
  `read_body_within_returns_a_short_body`. Green: same command.
- A transport failure stays distinguishable from a refusal —
  `read_body_within_reports_a_transport_failure`, driven by a bare
  `tokio::net::TcpListener` on `127.0.0.1:0` that accepts one connection,
  writes `HTTP/1.1 200 OK`, `Content-Length: 100`, a five-byte body, then drops
  the socket, so `chunk()` errors mid-body. `wiremock` cannot express a
  truncated body, so this one case uses a raw listener. Green: same command.
- The public wrapper binds the shipped constant —
  `read_body_bounded_returns_a_normal_body` and
  `max_response_body_bytes_is_64_mib`. Green: `make test-one T=read_body` and
  `make test-one T=max_response_body`.
- The variant's exit code is 16, its type is `response_too_large`, it publishes
  `operation` and `limit_bytes`, and it is not a transport failure — in
  `src/error_tests.rs`: `response_too_large_reports_exit_code_and_type`,
  `response_too_large_publishes_operation_and_limit`,
  `response_too_large_is_not_a_transport_failure`. Green:
  `make test-one T=response_too_large`.

### Steps

1. In `src/error.rs`, add the variant next to `HttpStatus`, with the
   `thiserror` message
   `"{operation}: server response exceeds the {limit_bytes}-byte
   response-body limit; the response was refused without being read"`.
2. Add `const EXIT_CODE_RESPONSE_TOO_LARGE: i32 = 16;` after
   `EXIT_CODE_UNSUPPORTED_CAPABILITY` and
   `const ERROR_TYPE_RESPONSE_TOO_LARGE: &str = "response_too_large";` after
   `ERROR_TYPE_UNSUPPORTED_CAPABILITY`.
3. Add one arm each to `exit_code()` and `error_type()` returning those
   constants, and one to `structured_detail()` inserting `operation` (a cloned
   `String`) and `limit_bytes` (the `u64`). Leave `is_transport_failure`
   unchanged: the variant must not appear there, or Hybrid mode would answer an
   over-limit REST body by re-fetching it over XML-RPC.
4. In `src/http.rs`, add the constant with a doc comment stating why 64 MiB: a
   base64 `data` field costs 4/3, so it admits a ~48 MiB attachment against a
   documented Bugzilla `maxattachmentsize` default of 1000 KB.
5. Add `BodyReadError` and `impl From<BodyReadError> for crate::error::BzrError`
   mapping `TooLarge` to `ResponseTooLarge` field-for-field and `Transport(e)`
   to `BzrError::Http(e)`.
6. Add the reader:

   ```rust
   pub(crate) async fn read_body_bounded(
       response: reqwest::Response,
       operation: &str,
   ) -> std::result::Result<String, BodyReadError> {
       read_body_within(response, operation, MAX_RESPONSE_BODY_BYTES).await
   }

   async fn read_body_within(
       mut response: reqwest::Response,
       operation: &str,
       limit_bytes: u64,
   ) -> std::result::Result<String, BodyReadError> {
       let mut body: Vec<u8> = Vec::new();
       while let Some(chunk) = response.chunk().await.map_err(BodyReadError::Transport)? {
           if body.len() as u64 + chunk.len() as u64 > limit_bytes {
               return Err(BodyReadError::TooLarge {
                   operation: operation.to_owned(),
                   limit_bytes,
               });
           }
           body.extend_from_slice(&chunk);
       }
       Ok(String::from_utf8_lossy(&body).into_owned())
   }
   ```

   The `u64` addition is deliberate: summing in `usize` could wrap on a 32-bit
   target. `from_utf8_lossy` is what `Response::text()` already does here,
   because `reqwest`'s `charset` feature is off in this build.
7. Write the five `src/http_tests.rs` cases and the three `src/error_tests.rs`
   cases named above. The three size cases use a limit of 16 bytes, so no case
   allocates more than a few bytes.
8. Confirm the bound bites by controlled fault: change `>` to `>=`, run
   `make test-one T=read_body_within`, observe
   `read_body_within_returns_a_body_at_the_limit` fail, then revert.
9. `make lint` and `make test-one T=read_body`, `make test-one
   T=response_too_large` — all exit 0.

**Acceptance.** All eight cases pass; `make lint` exits 0; no existing exit
code, error type, or transport classification changed.

**Adjacent, do not fix.** `CLAUDE.md` describes `BzrError` as having "19
variants" and becomes stale at 20. `CLAUDE.md` is outside this change's frozen
surface — report it, do not edit it. `src/error.rs` states no count.

## Task 2 — route all thirteen call sites

Modifies `src/client/response.rs`, `src/client/transport.rs`,
`src/client/version.rs`, `src/client/auth/whoami.rs`,
`src/client/auth/valid_login.rs`, `src/xmlrpc/protocol/client.rs`.

**Interfaces consumed.** `crate::http::read_body_bounded`,
`crate::http::BodyReadError`, `BzrError::ResponseTooLarge` (Task 1).

**Interfaces produced.** None; every change is an internal call-site swap.

### Verification

- No `Response::text()` call remains in `src/`. Mode: `focused-test`.
  Observable: `rg -n "\.text\(\)" src/` reports no match outside `*_tests.rs`.
  Red before the sweep: thirteen matches.
- Existing parse, error-classification, and probe behaviour is unchanged. Mode:
  `focused-test`. Tests: the existing `src/client/response_tests.rs`,
  `transport_tests.rs`, `version_tests.rs`, `auth/whoami_tests.rs`,
  `auth/valid_login_tests.rs`, and `src/xmlrpc/protocol/client_tests.rs`, all
  green **unmodified**. Red if the swap changes decoding or a probe's
  disposition: any body or outcome assertion in those files. Green:
  `make test-one T=response`, `T=auth`, `T=version`, `T=transport`, `T=xmlrpc`.
- An over-limit body at a degrading site takes that site's existing degraded
  path. Mode: `task-test-not-applicable`. Reason: the new arm returns the same
  value the site's existing unreadable-body arm returns —
  `AlternateAuth::Original`, `(None, ApiMode::XmlRpc)`, `AuthRejected`, `None`,
  or a substituted preview string — so no observation distinguishes the two
  arms beyond the `tracing` message, and asserting log wording tests prose. The
  refusal itself is proven in Task 1 at a limit a test can reach.

### Steps

Seven sites propagate. Replace the `.text().await?` expression with
`crate::http::read_body_bounded(<response>, <operation>).await?`; `?` converts
through the `From` impl from Task 1. Where the site reads `resp.url()` for a
`safe_url`, keep that line before the body read, because the read consumes the
response.

| Site | Function | Operation string |
|---|---|---|
| `src/client/response.rs:231` | `parse_strict_bug_adjacency_response` | `"strict Bug.get"` |
| `src/client/response.rs:270` | `check_mutation_response` | `"mutation response"` |
| `src/client/response.rs:283` | `parse_json` | `"response body"` |
| `src/client/response.rs:305` | `parse_json_value` | `"response body"` |
| `src/client/auth/valid_login.rs:115` | the credential confirmation returning `crate::error::Result<()>` | `"rest/valid_login"` |
| `src/xmlrpc/protocol/client.rs:90` | the XML-RPC call | `&format!("XML-RPC {method}")`, using the `method: &str` parameter already in scope |

1. `src/client/response.rs:456` `check_response_status` already matches on the
   read result. Split its `Err` arm in two: `BodyReadError::TooLarge` returns
   `BzrError::ResponseTooLarge` with the arm's `operation` and `limit_bytes`,
   and `BodyReadError::Transport(e)` keeps the existing behaviour verbatim —
   the `<failed to read response body: {e}>` preview inside `HttpStatus`. The
   refusal wins over the status here: the status alone is already reported by
   the ordinary path, and the fact worth telling the operator is that a 4xx or
   5xx arrived with a body `bzr` refused to buffer. Operation `"error
   response"`.

Six sites degrade. Each already has an `Err`/`else` arm for an unreadable body;
replace it with a two-variant `match` whose `Transport` arm is the existing
code verbatim and whose `TooLarge` arm logs `limit_bytes` and returns the same
value:

| Site | `TooLarge` and `Transport` both return | Operation string |
|---|---|---|
| `src/client/transport.rs:196` `retry_with_alternate_auth` | `Ok(AlternateAuth::Original)` | `"auth fallback"` |
| `src/client/version.rs:93` | `Ok((None, ApiMode::XmlRpc))` | `"version probe"` |
| `src/client/auth/whoami.rs:84` `probe_whoami` | `WhoamiOutcome::AuthRejected` / `WhoamiOutcome::NetworkError(e)` | `"whoami probe"` |
| `src/client/auth/valid_login.rs:164` `probe_valid_login` | `ValidLoginOutcome::AuthRejected` / `ValidLoginOutcome::NetworkError(e)` | `"valid_login probe"` |
| `src/client/auth/valid_login.rs:418` `read_probe_leg` | `None` | `"header auth probe"` |
| `src/xmlrpc/protocol/client.rs:79` | a preview `String` | `"XML-RPC error response"` |

2. At `transport.rs:196`, keep the existing comment explaining why the transport
   error is never formatted: its `Display` carries the URL, which on the
   query-parameter path holds the API key. `TooLarge` logs at `warn`, the
   existing `Transport` line stays at `debug`.
3. At `whoami.rs:84` and `valid_login.rs:164`, the existing arm needs the
   `reqwest::Error` for `NetworkError(e)`, which the `Transport` arm supplies
   unchanged. `TooLarge` returns `AuthRejected` instead: the probe learned
   nothing, and `AuthRejected` is that probe's "no", which its caller already
   handles.
4. At `xmlrpc/protocol/client.rs:79`, `TooLarge` produces
   `format!("<response body exceeds the {limit_bytes}-byte limit>")` and
   `Transport(e)` keeps `format!("<failed to read response body: {e}>")`.
5. Run `rg -n "\.text\(\)" src/` — expect no match outside `*_tests.rs`.
6. Run the five `make test-one` commands above and `make lint` — all exit 0,
   with no edits to any existing `*_tests.rs`.

**Acceptance.** All thirteen sites read through the helper; every existing
suite passes unmodified; `make lint` exits 0.

## Task 3 — publish the new exit code

Modifies `schemas/error.json`, `src/main_tests.rs`, `docs/bzr-cli.md`, and the
twelve `SCHEMA_VERSION` pins.

**Interfaces consumed.** `BzrError::ResponseTooLarge`,
`EXIT_CODE_RESPONSE_TOO_LARGE`, `ERROR_TYPE_RESPONSE_TOO_LARGE` (Task 1).

**Interfaces produced.** `SCHEMA_VERSION == "3.0.7"`.

### Verification

- Contract: the published error schema admits exit code 16 and the two new
  keys. Mode: `focused-test`. Test:
  `src/main_tests.rs::format_dispatch_error_json_family_matches_published_schema`,
  with a `BzrError::ResponseTooLarge` case added to its error list. That test
  reads `schemas/error.json` through `bzr schema error`, asserts the emitted
  `exit_code` lies within the schema's declared `minimum..=maximum`, and
  asserts every emitted key is a declared property. Red with the maximum left
  at 15: `formatted exit_code 16 outside schema bounds 1..=15`. Red with the
  keys undeclared: `emitted error.operation is not declared in
  schemas/error.json error.properties`. Green: `make test-one
  T=format_dispatch_error_json_family`.
  `src/commands/schema_tests.rs::assert_conforms` checks only top-level keys,
  so it does not cover the nested `error` object and is not the gate here.
- Contract: the version pin set is consistent across the crate, the docs, and
  the bundled skills. Mode: `focused-test`. Green: `make test` and
  `make skills-test`, both exit 0.

### Steps

1. `schemas/error.json`: change `"maximum": 15` to `"maximum": 16` under
   `properties.error.properties.exit_code`.
2. In the same file, add two optional properties beside the existing
   variant-specific keys, matching their description style: `operation`
   (string, "ResponseTooLarge: the operation whose response was refused.") and
   `limit_bytes` (integer, "ResponseTooLarge: the response-body byte limit that
   was exceeded.").
3. Extend the schema's top-level `description`, which lists example
   variant-specific keys, to name `operation`/`limit_bytes` for a refused
   response.
4. `src/main_tests.rs`: add `BzrError::ResponseTooLarge { operation:
   "response body".into(), limit_bytes: 67_108_864 }` to the error list in
   `format_dispatch_error_json_family_matches_published_schema`.
5. `docs/bzr-cli.md`: add the exit-code table row after the row for 15 —
   `| 16 | Response too large (the server's response body exceeds bzr's 64 MiB
   response-body limit; the response is refused without being buffered) |` —
   and add `operation` and `limit_bytes` rows to the structured-error key table
   that documents `field`, `value`, `status`, and `api_code`, attributed to
   `response_too_large` and matching the neighbouring rows' wording.
6. Bump `SCHEMA_VERSION` from `3.0.6` to `3.0.7` in exactly these twelve files.
   This is one atomic change; a partial sweep fails the suite:
   `src/output/mod.rs` (the const, line 10), `README.md`, `docs/bzr-cli.md`,
   `content/skills/bzr-reference/reference/commands.md`,
   `content/skills/bzr-reference/reference/json-recipes.md`,
   `content/skills/bzr-dependency-analysis/scripts/collect.py`,
   `content/skills/bzr-dependency-analysis/tests/test_collect.py`,
   `content/skills/bzr-dependency-analysis/tests/fixtures/recording_runner.py`,
   `tests/functional/phases/08e-bugs-restricted-access.sh`,
   `tests/functional/phases/18a-json-envelope.sh`,
   `tests/functional/phases/18c-skills-install.sh`,
   `tests/functional/phases/18d-dependency-analysis.sh`.
7. Verify the sweep left no straggler: `rg -n "3\.0\.6" --glob '!target' .` —
   expect no match. Confirm `docs/adr/0063-*.md` and `docs/adr/0065-*.md` are
   unchanged: `git diff --name-only docs/adr/` names only `0068-*`.
8. `make test`, `make skills-test`, `make lint` — all exit 0.

**Acceptance.** No `3.0.6` remains; the three guardrails exit 0; no Accepted
ADR was edited.

## Task 4 — prove the bound against a real socket

Modifies `tests/functional/phases/18b-http-error-preview.sh` only.

**Interfaces consumed.** Exit code 16, error type `response_too_large`, and the
`limit_bytes` structured key (Tasks 1 and 3).

**Interfaces produced.** None.

### Verification

- Contract: a server streaming past the limit is refused with exit 16. Mode:
  `focused-test`. Test: the new `oversized-response-body-is-refused` case in
  phase 18b. Red before Tasks 1–2: `bzr` buffers the whole stream and the case
  reports a different exit code or does not terminate. Green:
  `make functional-test`.
- Contract: the published schema admits the new code end to end. Mode:
  `focused-test`. Test: the new `schema-error-admits-exit-16` case asserting
  `bzr schema error` reports
  `.properties.error.properties.exit_code.maximum == 16`. Green: same command.

### Steps

1. Update the phase header comment — it says "Creates: one loopback HTTP
   fixture process"; make it two — and add `#740` beside `#512` in the banner.
2. After the existing case and its teardown, add a second python3 fixture,
   copying the existing case's `mktemp` port file, readiness loop, and
   `kill`/`rm -f` teardown shape verbatim rather than sharing them, so the two
   cases stay independent. The handler must:
   - answer any path starting `/rest/version` with a 200 and the small body
     `{"version":"5.0.4"}` plus a matching `Content-Length`, so version
     detection succeeds and the test exercises the real request;
   - answer every other path with a 200, `Content-Type: application/json`, a
     `Content-Length` of `1 << 40`, and then write a 65536-byte filler buffer
     in a loop until the client disconnects, catching `BrokenPipeError` and
     `ConnectionResetError`;
   - set `protocol_version = "HTTP/1.1"` and silence `log_message`.
   Bind to `127.0.0.1:0` and write the chosen port to the temp file, as the
   existing fixture does.
3. Add the refusal case. Every `assert_*` helper calls `test_fail` itself and
   returns non-zero, and `test_fail` is not idempotent, so keep all four
   assertions in one `if` chain with no `else` branch that calls `test_fail`
   again:

   ```bash
   test_begin "oversized-response-body-is-refused" "oversized response body is refused (#740)"
   run_bzr --server-url "http://127.0.0.1:${_oversize_port}" --api rest server info
   if assert_exit_code 16 &&
       assert_stderr_json '.error.type' "response_too_large" &&
       assert_stderr_json '.error.exit_code' "16" &&
       assert_stderr_json '.error.limit_bytes' "67108864"; then
       test_pass
   fi
   ```

4. Add the schema case, which needs no fixture. `bzr schema <name>` writes the
   schema verbatim with no envelope (`src/commands/schema.rs::write_one`), so
   the jq path starts at the schema root, not at `.data`:

   ```bash
   test_begin "schema-error-admits-exit-16" "published error schema admits exit 16 (#740)"
   run_bzr_raw schema error
   if assert_success &&
       assert_raw_json '.properties.error.properties.exit_code.maximum' "16"; then
       test_pass
   fi
   ```

5. Both test IDs must match `^[a-z0-9]+(-[a-z0-9]+)*$` for
   `make check-functional-test-ids`; the two above do. No new phase file means
   no `run-tests.sh` edit, which is what keeps this task off the branch that
   owns the harness.
6. `make check-shell` — exit 0. The phase scripts are shellcheck'd and
   `bash -n`'d by that target; match the file's existing tab indentation.
7. `make functional-test` — exit 0, with both new cases reported passing.

**Acceptance.** `make lint` (which runs `check-shell` and
`check-functional-test-ids`) and `make functional-test` both exit 0; the new
cases appear in phase 18b's output.

## Rollback

Every task is additive and confined to the branch. Reverting the merge restores
the unbounded reads; no data is migrated, no config key is written, no
persisted state changes. The only externally visible artifacts are
`SCHEMA_VERSION` 3.0.7 and exit code 16 — a consumer pinned to 3.0.6 sees a
version it does not know rather than a changed payload, which is what ADR
0007's additive rule is for.
