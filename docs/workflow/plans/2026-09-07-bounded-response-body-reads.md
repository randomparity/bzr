# Plan: bounded response-body reads

**Goal.** Stop `bzr` buffering an unbounded HTTP response body: route all
thirteen `Response::text()` sites through one helper that refuses past 64 MiB,
and report the refusal as a new error with its own exit code.

Read `docs/workflow/specs/2026-09-07-bounded-response-body-reads-design.md`
(architecture, threat model, success criteria) and
`docs/adr/0068-response-bodies-are-read-under-one-shared-bound.md` (the limit's
grounds, rejected alternatives, residuals) before Task 1. This plan does not
restate them.

Expected implementation size: 290–380 changed lines (M) — summing the tasks'
own file lists: helper ~60, call-site swaps plus `ProbeRefused` and its caller
arms ~85, error variant ~30, sibling tests ~95, schema/docs/skill-reader/pin
edits ~65, functional fixture ~45. That exceeds the frozen 250-line M
denominator by about a quarter, and the denominator does not move: it was fixed
before design, the overrun is mechanical swaps and their tests rather than new
scope, and no completion criterion was added.

## Global constraints

- **Branch** `feat/bound-response-body-reads-740`, base branch `main`.
- **Guardrails, run bare** (no `| tail`, no `>/dev/null`, no `|| true`):
  `make lint`, `make test`, `make skills-test`, `make functional-test`.
  Iterate with `make test-one T=<substr>` and `make test-fast`. Never run bare
  `cargo test`.
- **No inline `mod tests` in `src/`** — unit tests live in a sibling
  `<name>_tests.rs`, and `make check-test-layout` enforces it.
- **`clippy.toml` disallows every thread and task spawn**, and
  `tools/check-no-spawn.sh` — wired into `make lint` — fails if an entry is
  removed. Its regex scan skips `*_tests.rs`, but `make clippy` runs
  `--all-targets` and still sees them, so a spawning test needs a file-level
  `#![expect(...)]` naming **exactly** the lints it triggers — an `expect` for
  a lint the file never raises is `unfulfilled_lint_expectation`, fatal under
  `-D warnings`. `src/http_tests.rs` gains
  `#![expect(clippy::disallowed_methods, clippy::unwrap_used)]` in Task 1, the
  same line `src/client/response_tests.rs:1` carries.
- **Clippy pedantic, warnings are errors**, `unwrap_used` denied in `src/`; no
  `println!`/`eprintln!` there — diagnostics go through `tracing`.
- **At least one commit must reach the release notes.** `cliff.toml` excludes
  unscoped commits and the infra scopes `site|release|ci|adr|plan|spec|
  changelog|build|test|docs|deps`, and ignores commit bodies. Give the
  implementation commit a `feat(error)` or `fix(client)` subject — no
  backticks, brackets, or ampersands, which the release extractor rejects.
- **`SCHEMA_VERSION` is `3.0.6` on main and becomes `3.0.7`**, across exactly
  the twelve pins listed in Task 3. The `3.0.5` strings in
  `docs/adr/0063-*.md` and `docs/adr/0065-*.md` narrate history inside Accepted
  records and must not be touched, and neither may `Cargo.lock`, which carries
  an unrelated `pem 3.0.6`.
- **Do not edit** `docs/adr/README.md` (the orchestrator appends the index
  row), `tests/functional/run-tests.sh`, `tests/functional/lib.sh`, or the
  Makefile (another branch owns them). Phase 18b is already registered, so no
  runner edit is needed. **Public-write hygiene**: no absolute host paths,
  hostnames, usernames, emails, IPs, or credentials in commit messages, PR
  text, or docs.

## Task 1 — the bounded read helper and its error

Modifies `src/http.rs`, `src/http_tests.rs`, `src/error.rs`,
`src/error_tests.rs`, in one commit: lib-only clippy denies dead code, so the
producer and its only consumer land together.

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

pub(crate) async fn read_body_within(
    response: reqwest::Response,
    operation: &str,
    limit_bytes: u64,
) -> std::result::Result<String, BodyReadError>;

// src/error.rs
BzrError::ResponseTooLarge { operation: String, limit_bytes: u64, status: Option<u16> }
```

**Interfaces consumed.** `crate::error::BzrError::Http(reqwest::Error)` and the
existing `EXIT_CODE_*` / `ERROR_TYPE_*` constant blocks in `src/error.rs`.

### Verification

All `focused-test`. Red before the code exists: `error[E0425]: cannot find
function read_body_within` for the first four, `error[E0599]: no variant named
ResponseTooLarge` for the fifth.

- The limit is enforced at the boundary and not before it — in
  `src/http_tests.rs`, `read_body_within_refuses_a_body_over_the_limit`
  (asserting `BodyReadError::TooLarge` carries the limit and the operation),
  `read_body_within_returns_a_body_at_the_limit`, and
  `read_body_within_returns_a_short_body`. Green:
  `make test-one T=read_body_within`.
- A transport failure stays distinguishable from a refusal —
  `read_body_within_reports_a_transport_failure`, driven by a truncated-response
  listener in the shape of `spawn_truncated_http_error_server`
  (`src/client/response_tests.rs:25-37`): `std::net::TcpListener` on
  `127.0.0.1:0` plus `std::thread::spawn`, declaring `Content-Length: 32` and
  writing four bytes before dropping the socket, since `wiremock` cannot express
  a truncated body. Green: same command.
- The public wrapper binds the shipped constant —
  `read_body_bounded_returns_a_normal_body` and
  `max_response_body_bytes_is_64_mib`. Green: `make test-one T=read_body` and
  `make test-one T=max_response_body`.
- No unbounded read can be reintroduced —
  `no_response_text_calls_outside_tests`, walking `env!("CARGO_MANIFEST_DIR")/
  src` recursively and failing on any `.text()` in a file not ending
  `_tests.rs`. Red today: thirteen hits; green after Task 2. This is the spec's
  only control against a fourteenth site.
- The variant's exit code is 16, its type is `response_too_large`, it publishes
  `operation`, `limit_bytes`, and `status` when set, and it is not a transport
  failure — in `src/error_tests.rs`, the four cases
  `response_too_large_reports_exit_code_and_type`, `..._publishes_operation_
  limit_and_status`, `..._omits_absent_status`, and
  `..._is_not_a_transport_failure`. Green: `make test-one T=response_too_large`.

### Steps

1. In `src/error.rs`, add the variant next to `HttpStatus`, with the
   `thiserror` message `"{operation}: server response exceeds the
   {limit_bytes}-byte response-body limit; the read was stopped at the limit
   and the response discarded"`. It must not claim the body was refused
   *without* being read — up to `limit_bytes` are read by construction.
2. Add `const EXIT_CODE_RESPONSE_TOO_LARGE: i32 = 16;` after
   `EXIT_CODE_UNSUPPORTED_CAPABILITY` and
   `const ERROR_TYPE_RESPONSE_TOO_LARGE: &str = "response_too_large";` after
   `ERROR_TYPE_UNSUPPORTED_CAPABILITY`.
3. Add one arm each to `exit_code()` and `error_type()` returning those
   constants, and one to `structured_detail()` inserting `operation` (cloned
   `String`), `limit_bytes` (`u64`), and `status` (only when `Some`, reusing
   the `status` key `schemas/error.json` already declares for `HttpStatus`).
   Leave `is_transport_failure` unchanged: the variant must not appear there.
4. In `src/http.rs`, add the constant. Its doc comment states the ceiling in
   terms a reader can check — a base64 `data` field costs 4/3, so 64 MiB admits
   a ~48 MiB attachment — and links ADR 0068 for why that number rather than
   another. Do not restate the ADR's argument here.
5. Add `BodyReadError` and `impl From<BodyReadError> for crate::error::BzrError`
   mapping `TooLarge` to `ResponseTooLarge` with `status: None`, and
   `Transport(e)` to `BzrError::Http(e)`.
6. Add the two readers, with the signatures given above.
   `read_body_bounded` is one line: `read_body_within(response, operation,
   MAX_RESPONSE_BODY_BYTES).await`. `read_body_within` takes `mut response`,
   starts an empty `Vec<u8>`, and loops
   `while let Some(chunk) = response.chunk().await.map_err(BodyReadError::Transport)?`.
   Inside, it returns `Err(BodyReadError::TooLarge { operation:
   operation.to_owned(), limit_bytes })` when
   `body.len() as u64 + chunk.len() as u64 > limit_bytes`, and otherwise
   `body.extend_from_slice(&chunk)`. After the loop it returns
   `Ok(String::from_utf8(body).unwrap_or_else(|error|
   String::from_utf8_lossy(error.as_bytes()).into_owned()))`.

   Three details are load-bearing. The `u64` addition is deliberate: summing in
   `usize` could wrap on a 32-bit target. `String::from_utf8` reuses the buffer
   on the valid-UTF-8 path and falls back to the same lossy decode
   `Response::text()` performs, so output is byte-identical at half the peak
   allocation. And the `Vec` is never pre-sized from `Content-Length` — the
   server writes that header.
7. Add `#![expect(clippy::disallowed_methods, clippy::unwrap_used)]` as the
   first line of `src/http_tests.rs`, then write the nine cases named above.
   The three size cases use a 16-byte limit, so none allocates more than a few
   bytes.
8. Confirm the bound bites by controlled fault: change `>` to `>=`, run
   `make test-one T=read_body_within`, observe
   `read_body_within_returns_a_body_at_the_limit` fail, then revert.
9. `make lint`, `make test-one T=read_body`, `make test-one
   T=response_too_large` — all exit 0.

**Acceptance.** Every case above passes except
`no_response_text_calls_outside_tests`, which Task 2 turns green; `make lint`
exits 0; no existing exit code, error type, or transport classification
changed.

**Adjacent, do not fix.** `AGENTS.md` describes `BzrError` as having "19
variants" and becomes stale at 20 (`CLAUDE.md` is a symlink to it). It is
outside this change's frozen surface — report it, do not edit it.

## Task 2 — route all thirteen call sites

Modifies `src/client/response.rs`, `src/client/transport.rs`,
`src/client/version.rs`, `src/client/auth/whoami.rs`,
`src/client/auth/valid_login.rs`, `src/client/auth/mod.rs`,
`src/xmlrpc/protocol/client.rs`, and the two auth test siblings.

**Interfaces consumed.** `crate::http::read_body_bounded`,
`crate::http::BodyReadError`, `BzrError::ResponseTooLarge` (Task 1).

**Interfaces produced.**

```rust
// src/client/auth/whoami.rs
WhoamiOutcome::ProbeRefused(crate::error::BzrError)
// src/client/auth/valid_login.rs
ValidLoginOutcome::ProbeRefused(crate::error::BzrError)
```

### Verification

- No `Response::text()` call remains in `src/`. Mode: `focused-test`. Test:
  `src/http_tests.rs::no_response_text_calls_outside_tests` from Task 1, now
  green. Green: `make test-one T=no_response_text`.
- An over-limit auth-probe body aborts detection instead of continuing the
  probe chain. Mode: `focused-test`. Tests:
  `src/client/auth/whoami_tests.rs::whoami_probe_refuses_an_over_limit_body`
  and
  `src/client/auth/valid_login_tests.rs::valid_login_probe_refuses_an_over_limit_body`,
  each calling `probe_whoami` / `probe_valid_login` directly with a 16-byte
  limit against a wiremock server, and asserting `ProbeRefused`, not
  `AuthRejected`. Red before the outcome exists: `error[E0599]: no variant
  named ProbeRefused`. Green: `make test-one T=refuses_an_over_limit_body`.
  This is why step 3 gives those two functions a `limit_bytes` parameter: they
  are the only sites whose degraded value changes, so they are the only ones
  that need to be reachable at a testable limit.
- Existing parse, error-classification, and probe behaviour is unchanged. Mode:
  `focused-test`. Tests: the existing `response_tests.rs`, `transport_tests.rs`,
  `version_tests.rs`, `auth/whoami_tests.rs`, `auth/valid_login_tests.rs`, and
  `xmlrpc/protocol/client_tests.rs`, green apart from the two cases added above.
  Red if the swap changes decoding or a probe's disposition: any body or outcome
  assertion in those files. Green: `make test-one T=response`, `T=auth`,
  `T=version`, `T=transport`, `T=xmlrpc`.
- The three remaining degraded arms take the site's existing degraded path.
  Mode: `task-test-not-applicable`. Reason: each returns exactly the value that
  site's existing unreadable-body arm returns — `AlternateAuth::Original`,
  `None`, or a substituted preview `String` — so no observation distinguishes
  the two arms beyond the `tracing` message, and asserting log wording tests
  prose. The two sites whose degraded value would have differed are
  reclassified as propagating below rather than waived.

### Steps

Ten sites propagate. Where the site simply used `?`, replace the
`.text().await?` expression with `crate::http::read_body_bounded(<response>,
<operation>).await?`; `?` converts through the `From` impl from Task 1. Where
the site reads `resp.url()` for a `safe_url`, keep that line before the body
read, because the read consumes the response.

| Site | Function | Operation string |
|---|---|---|
| `src/client/response.rs:231` | `parse_strict_bug_adjacency_response` | `"strict Bug.get"` |
| `src/client/response.rs:270` | `check_mutation_response` | `"mutation response"` |
| `src/client/response.rs:283` | `parse_json` | `"response body"` |
| `src/client/response.rs:305` | `parse_json_value` | `"response body"` |
| `src/client/auth/valid_login.rs:115` | the credential confirmation returning `crate::error::Result<()>` | `"rest/valid_login"` |
| `src/xmlrpc/protocol/client.rs:90` | the XML-RPC call | `&format!("XML-RPC {method}")`, using the `method: &str` parameter already in scope |

1. `src/client/response.rs:456` `check_response_status` already matches on the
   read result. Split its `Err` arm in two. `BodyReadError::Transport(e)` keeps
   the existing behaviour verbatim — the `<failed to read response body: {e}>`
   preview inside `HttpStatus`. `BodyReadError::TooLarge` returns
   `BzrError::ResponseTooLarge` with the arm's `operation` and `limit_bytes`
   **and `status: Some(status.as_u16())`**, so the 4xx or 5xx the operator
   needs is not lost; that is why the variant carries a status at all.
   Operation `"error response"`.
2. `src/client/version.rs:93`: today the arm returns `Ok((None,
   ApiMode::XmlRpc))`. Keep that for `Transport`, and return
   `Err(BzrError::from(error))` for `TooLarge`. The function already propagates
   a TLS-certificate failure this way (`version.rs:74-79`); without this, a
   server selects the wire protocol by choosing a body size.
3. `src/client/auth/whoami.rs:84` `probe_whoami` and
   `src/client/auth/valid_login.rs:164` `probe_valid_login`: give each a
   `limit_bytes: u64` parameter and call `read_body_within` rather than
   `read_body_bounded`; their callers pass
   `crate::http::MAX_RESPONSE_BODY_BYTES`, and their tests pass 16. The
   existing arm needs the `reqwest::Error` for `NetworkError(e)`, which the
   `Transport` arm supplies unchanged. For `TooLarge`, add and return
   `ProbeRefused(BzrError::from(error))` on each outcome enum. Then:
   `detect_whoami_auth` (`whoami.rs:41-72`) must return `ProbeRefused`
   immediately from the header leg rather than falling through to the
   query-param leg, which would put the API key in a URL, and
   `detect_valid_login_auth` must do the same for its legs.
   `src/client/auth/mod.rs` gains one arm per enum beside the existing
   `NetworkError(e) => return network_error_outcome(e)` arms:
   `ProbeRefused(error) => return Err(error)`. Operation strings
   `"whoami probe"` and `"valid_login probe"`.

Three sites degrade. Each already has an `Err`/`else` arm for an unreadable
body; replace it with a two-variant `match` whose `Transport` arm is the
existing code verbatim and whose `TooLarge` arm logs `limit_bytes` and returns
the same value:

| Site | Both arms return | Operation string |
|---|---|---|
| `src/client/transport.rs:196` `retry_with_alternate_auth` | `Ok(AlternateAuth::Original)` | `"auth fallback"` |
| `src/client/auth/valid_login.rs:418` `read_probe_leg` | `None` | `"header auth probe"` |
| `src/xmlrpc/protocol/client.rs:79` | a preview `String` | `"XML-RPC error response"` |

4. At `transport.rs:196`, keep the existing comment explaining why the
   transport error is never formatted: its `Display` carries the URL, which on
   the query-parameter path holds the API key. `TooLarge` logs at `warn`, the
   existing `Transport` line stays at `debug`.
5. At `valid_login.rs:418`, `Transport` keeps its `redacted_probe_error` debug
   line and `TooLarge` logs `limit_bytes` at debug; both return `None`. The leg
   was already inconclusive, and no credential is re-sent by returning here.
6. At `xmlrpc/protocol/client.rs:79`, `TooLarge` produces
   `format!("<response body exceeds the {limit_bytes}-byte limit>")` and
   `Transport(e)` keeps `format!("<failed to read response body: {e}>")`.
7. Run the five `make test-one` commands above, plus `make test-one
   T=no_response_text` and `make test-one T=refuses_an_over_limit_body`, and
   `make lint` — all exit 0, with no edits to existing cases in any
   `*_tests.rs`.

**Acceptance.** All thirteen sites read through the helper; the structural test
is green; every pre-existing case passes unmodified; `make lint` exits 0.

## Task 3 — publish the new exit code

Modifies `schemas/error.json`, `src/main_tests.rs`, `docs/bzr-cli.md`, the two
bundled readers, and the twelve `SCHEMA_VERSION` pins.

**Interfaces consumed.** `BzrError::ResponseTooLarge`,
`EXIT_CODE_RESPONSE_TOO_LARGE`, `ERROR_TYPE_RESPONSE_TOO_LARGE` (Task 1).

**Interfaces produced.** `SCHEMA_VERSION == "3.0.7"`.

### Verification

- The published error schema admits exit code 16 and the two new keys. Mode:
  `focused-test`. Test:
  `src/main_tests.rs::format_dispatch_error_json_family_matches_published_schema`,
  with a `BzrError::ResponseTooLarge` case added to its error list; it reads
  `schemas/error.json` through `bzr schema error` and asserts both the
  `minimum..=maximum` bound and that every emitted key is declared. Red with
  the maximum left at 15: `formatted exit_code 16 outside schema bounds
  1..=15`. Red with the keys undeclared: `emitted error.operation is not
  declared in schemas/error.json error.properties`. Green:
  `make test-one T=format_dispatch_error_json_family`.
  `src/commands/schema_tests.rs::assert_conforms` checks only top-level keys,
  so it does not reach the nested `error` object and is not the gate here.
- The bundled dependency-analysis reader accepts an exit-16 envelope. Mode:
  `focused-test`. Test: a new case in
  `content/skills/bzr-dependency-analysis/tests/test_collect.py` passing an
  exit-16 envelope carrying `operation` and `limit_bytes` to
  `validate_error_envelope` and asserting it returns rather than raising
  `FatalCollection`. Red before the edit: `FatalCollection:
  collection-malformed-output`. Green: `python3 -m pytest
  content/skills/bzr-dependency-analysis/tests/test_collect.py`. No repository
  gate runs this file; say so when reporting the run.
- The version pin set is consistent across the crate, the docs, and the
  bundled skills. Mode: `focused-test`. Green: `make test` and
  `make skills-test`, both exit 0.

### Steps

1. `schemas/error.json`: change `"maximum": 15` to `"maximum": 16` under
   `properties.error.properties.exit_code`.
2. In the same file, add two optional properties beside the existing
   variant-specific keys, matching their description style: `operation`
   (string) and `limit_bytes` (integer), each described as belonging to
   `ResponseTooLarge`. Extend the `status` key's description to name
   `ResponseTooLarge` beside `HttpStatus`, and the schema's top-level
   `description` — which lists example variant keys — to name the two new ones.
3. `src/main_tests.rs`: add `BzrError::ResponseTooLarge { operation: "response
   body".into(), limit_bytes: 67_108_864, status: Some(200) }` to the error
   list in `format_dispatch_error_json_family_matches_published_schema`.
4. `content/skills/bzr-dependency-analysis/scripts/collect.py`: raise the
   `validate_error_envelope` ceiling from `1 <= value["exit_code"] <= 14` to
   `<= 16`, and add `operation` to `ERROR_STRING_KEYS` and `limit_bytes` to
   `ERROR_INTEGER_KEYS`. **Also add `capability` and `capability_status`**,
   which shipped with exit 15 and were never added — same verified root cause
   (this validator is a hand-maintained mirror of the error contract and lags
   it), and a validator accepting 16 while rejecting 15 would be indefensible.
   Report the pre-existing gap.
5. `content/skills/bzr-reference/SKILL.md`: add a `response_too_large | 16 |
   operation, limit_bytes, and status when the refused response carried an
   error status` row to the error-type table (`:145-162`), whose next line
   asserts "That is the whole set". The drift checks do not scan `SKILL.md` and
   `make skills-test` does not read the table, so verify every cell by hand
   against `src/error.rs` and `schemas/error.json`; nothing else will. Leave
   `AGENTS.md`'s variant count alone (`CLAUDE.md` is a symlink to it) — the
   orchestrator owns it.
6. `docs/bzr-cli.md`: add the exit-code table row after the row for 15 —
   `| 16 | Response too large (the server's response body exceeds bzr's 64 MiB
   response-body limit; the read stops at the limit and the response is
   discarded unparsed) |` — and add `operation` and `limit_bytes` rows to the
   structured-error key table that documents `field`, `value`, `status`, and
   `api_code`, attributed to `response_too_large`.
7. `docs/bzr-cli.md`, the `bzr attachment download` section: one sentence
   recording that an attachment whose base64 payload exceeds the 64 MiB limit
   cannot be downloaded and exits 16 — the one user-visible regression here.
8. Bump `SCHEMA_VERSION` from `3.0.6` to `3.0.7` in exactly these twelve files
   — one atomic change; a partial sweep fails the suite. `src/output/mod.rs`
   (the const, line 10); `README.md`; `docs/bzr-cli.md`; under
   `content/skills/bzr-reference/reference/`, `commands.md` and
   `json-recipes.md`; under `content/skills/bzr-dependency-analysis/`,
   `scripts/collect.py`, `tests/test_collect.py`, and
   `tests/fixtures/recording_runner.py`; and under
   `tests/functional/phases/`, `08e-bugs-restricted-access.sh`,
   `18a-json-envelope.sh`, `18c-skills-install.sh`, and
   `18d-dependency-analysis.sh`.
9. Verify the sweep left no straggler *within the pin set*:
   `rg -n "3\.0\.6" src README.md docs/bzr-cli.md content/skills tests/functional`
   — expect no match. A repository-wide grep does not work: `Cargo.lock`
   carries an unrelated `pem 3.0.6`, and this branch's own ADR, spec, and plan
   narrate the 3.0.6 → 3.0.7 transition. Confirm no Accepted ADR was touched:
   `git diff --name-only main...HEAD -- docs/adr/` must name only `0068-*`.
10. `make test`, `make skills-test`, `make lint` — all exit 0.

**Acceptance.** No `3.0.6` remains in the pin set; `collect.py` accepts an
exit-16 envelope; the three guardrails exit 0; no Accepted ADR was edited.

## Task 4 — prove the bound against a real socket

Modifies `tests/functional/phases/18b-http-error-preview.sh` only.

**Interfaces consumed.** Exit code 16, error type `response_too_large`, and the
`limit_bytes` structured key (Tasks 1 and 3).

**Interfaces produced.** None.

### Verification

Both `focused-test`; both green under `make functional-test`.

- A server streaming past the limit is refused with exit 16 — the new
  `oversized-response-body-is-refused` case. Red before Tasks 1–2: `bzr`
  buffers the fixture's whole 96 MiB budget and the exit-16 assertion fails
  against a different code.
- The published schema admits the new code end to end — the new
  `schema-error-admits-exit-16` case, asserting `bzr schema error` reports
  `.properties.error.properties.exit_code.maximum == 16`.

### Steps

1. Update the phase header comment — "Creates: one loopback HTTP fixture
   process" becomes two — and add `#740` beside `#512` in the banner.
2. After the existing case and its teardown, add a second python3 fixture,
   copying the existing case's `mktemp` port file, readiness loop, `127.0.0.1:0`
   bind, and `kill`/`rm -f` teardown verbatim rather than sharing them, so the
   two cases stay independent. Set `protocol_version = "HTTP/1.1"` and silence
   `log_message`. The handler answers any path starting `/rest/version` with a
   200 and the small body `{"version":"5.0.4"}` plus a matching
   `Content-Length`, so version detection succeeds and the test exercises the
   real request; every other path gets a 200, `Content-Type:
   application/json`, `Content-Length: 100663296`, and a 65536-byte filler
   buffer written until **exactly that 96 MiB budget** is sent, then close,
   catching `BrokenPipeError` and `ConnectionResetError` throughout. The budget
   is not optional: `tests/functional/lib.sh` wraps `run_bzr` in no `timeout`,
   so an unbounded writer against a regressed client would buffer until the
   host ran out of memory instead of failing the assertion. 96 MiB is 1.5× the
   limit — enough that a correct client refuses well before the end, small
   enough that a regressed one fails in seconds.
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
   no `run-tests.sh` edit, keeping this task off the harness branch.
6. `make check-shell` — exit 0. The phase scripts are shellcheck'd and
   `bash -n`'d by that target; match the file's existing tab indentation.
7. `make functional-test` — exit 0, with both new cases reported passing.

**Acceptance.** `make lint` (which runs `check-shell` and
`check-functional-test-ids`) and `make functional-test` both exit 0; the new
cases appear in phase 18b's output.

