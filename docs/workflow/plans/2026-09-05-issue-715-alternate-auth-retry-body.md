# Issue #715 — classify the alternate-auth retry from its body

Goal: a Bugzilla refusal that is not a login failure must be reported with the
server's own error code and message, instead of being discarded in favour of the
original 401's "You must log in".

Architecture: `src/client/transport.rs` owns the 401 alternate-auth fallback and
`src/client/response.rs` owns turning an HTTP error status plus its body into a
`BzrError`. The fallback currently decides from the status alone. It will decide
from the retried body's Bugzilla error code, and will report a relayed refusal
through the same construction `response.rs` already uses, so the two cannot
drift.

Tech stack: Rust 2021, `reqwest` (async), `serde`/`serde_json`, `tracing`,
`wiremock` + `tokio` for tests, Bash for the functional tier.

Expected implementation size: 420–530 changed lines (M) — counted off the file
map: `response.rs` ~50 changed lines (a ~30-line helper pair, ~18 lines moved out
of `check_response_status`), `transport.rs` ~105, ~265 added to
`src/client/transport_tests.rs` (seven fallback cases, the R9 command-layer case
with its own fixtures, ~35 lines of helpers, and two comment corrections), ~15
added to `src/client/response_tests.rs`, ~55 added to one existing functional
phase script. `ErrorResponse`, `default_error_code` and the ADR-0015-governed
HTTP-200 error path in `check_bugzilla_200_error` are not touched.

## Global Constraints

- Unit tests live in **sibling `<name>_tests.rs` files** linked from the source
  file via `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`. An inline `mod
  tests { ... }` in `src/` is forbidden and `make check-test-layout` enforces
  it. `src/client/transport_tests.rs` already exists and is already linked; add
  to it, do not create a new file. Its file-level
  `#![expect(clippy::unwrap_used)]` stays as it is.
- All API tests are `#[tokio::test]`; HTTP is mocked with `wiremock` 0.6.5.
- Clippy pedantic is on and warnings are errors: `unwrap_used` is denied in
  `src/`, `expect_used` and `allow_attributes` warn. Prefer `#[expect(...)]` with
  a reason over `#[allow(...)]` if one is unavoidable.
- User-facing output goes through `Writers`; diagnostics through `tracing`. No
  `println!`/`eprintln!` in `src/`.
- URLs in logs go through `BugzillaClient::safe_url`; body text in logs goes
  through `crate::bugzilla_auth::redact_api_key`. Never interpolate a
  `reqwest::Error` into a log line or a message on a new code path: its `Display`
  appends ` for url ({url})` with the query string intact
  (`reqwest-0.12.28/src/error.rs:267`), which on the query-parameter auth path is
  the API key.
- Guardrails, run bare (no pipes, no `|| true`):
  - `make lint`
  - `make test-one T=<substring>` while iterating, `make test-fast` for `--lib`
    only, `make test` before committing
  - `make functional-test` for the container tier (~10 minutes; run it in the
    background, never piped through `tail`)
  - never run bare `cargo test`
- Functional phase scripts use **4-space indent** and are not CI-linted; bare
  `shfmt` will disagree with the repo's actual style. Match the surrounding file.
- Do not edit `docs/adr/README.md`, `src/client/auth/valid_login.rs`, or
  `src/client/auth/mod.rs`.
- Commits are Conventional Commits 1.0.0. At least one must carry `fix(client):`
  or `fix(auth):` so the change reaches the generated release notes. Commit
  **bodies are ignored** by the notes generator, so the user-visible exit-code
  change (spec R9: a `--permissive` batch can now exit 0 where it exited 4) must
  appear in a commit **subject**, either the main one or a second commit's. Keep
  the R9 pinning test in its own commit so the consequence stays separable in the
  diff if the operator asks for it to be revisited.

## File map

| File | Changed | Answerable for |
|---|---|---|
| `src/client/response.rs` | Yes | `error_from_status_body`; `bugzilla_error_code`; `check_response_status` delegating to the first |
| `src/client/transport.rs` | Yes | The `AlternateAuth` outcome, the authentication-code band, `send_raw`'s three-way branch |
| `src/client/response_tests.rs` | Yes | The no-`code` sentinel regression test, beside the rest of that family |
| `src/client/transport_tests.rs` | Yes | Wiremock proofs that the two 401 bodies diverge, and the R9 exit-code pin |
| `tests/functional/phases/08e-bugs-restricted-access.sh` | Yes | Contract pinned against a real Bugzilla, authenticated and credentialless |

`docs/adr/0057-alternate-auth-retry-classifies-the-refusal.md` and
`docs/workflow/specs/2026-09-05-issue-715-alternate-auth-retry-body-design.md`
are already written and committed.

---

## Task 1 — classify the retry from its body

Where this fits: the whole production change. The `response.rs` half and the
`transport.rs` half are one deliverable — the extracted helper has no other
caller, so landing it alone would be dead code.

### Interfaces

Consumed from the existing codebase, each confirmed present at the stated
location and signature in this worktree:

- `crate::error::BzrError::Api { code: i64, message: String }` and
  `BzrError::HttpStatus { status: u16, body: String }` — `src/error.rs:18`, `:37`
- `crate::http::diagnostic_body_preview(body: &str) -> String` — `src/http.rs:71`
- `crate::http::utf8_prefix(body: &str, max_bytes: usize) -> &str` —
  `src/http.rs:62`
- `crate::bugzilla_auth::redact_api_key(msg: &str) -> String` —
  `src/bugzilla_auth.rs:123`
- `BugzillaClient::safe_url(url: &reqwest::Url) -> String` —
  `src/client/transport.rs:189`
- `BugzillaClient::apply_alternate_auth(&self, builder: RequestBuilder) ->
  Result<RequestBuilder>` — `src/client/transport.rs:169`
- `struct ErrorResponse { error: bool, code: i64, message: Option<String> }`,
  module-private in `src/client/response.rs:14`, with `code` defaulting to `-1`
  via `default_error_code` — **reused unchanged**
- `BODY_PREVIEW_MAX_BYTES` — module-private constant, `src/client/response.rs:627`

Published for later tasks:

- `BugzillaClient::error_from_status_body(status: reqwest::StatusCode, body:
  &str) -> BzrError` (`pub(super)`, `src/client/response.rs`)
- `BugzillaClient::bugzilla_error_code(body: &str) -> Option<i64>`
  (`pub(super)`, `src/client/response.rs`)
- `enum AlternateAuth { Replace(reqwest::Response), Refused(BzrError), Original }`
  and `fn code_proves_auth_failure(code: i64) -> bool` (both private to
  `src/client/transport.rs`)

### Verification

- Contract: *a retried 401 carrying a non-authentication Bugzilla error code
  replaces the original 401.*
  Mode: focused-test.
  Observable contract: `client.get_json_value("bug/1").await` returns
  `Err(BzrError::Api { code: 120, .. })` when the first attempt answers 401 with
  code 410 and the retry answers 401 with code 120.
  Test: `src/client/transport_tests.rs`,
  `auth_fallback_relays_a_policy_refusal_from_the_retried_body` (written in
  Task 2).
  Expected red before step 5: the assertion on `code == 120` fails, reporting
  `410`.
  Green command: `make test-one T=auth_fallback_relays_a_policy_refusal`.

- Contract: *a retried 401 carrying an authentication code leaves the original
  401 standing.*
  Mode: focused-test.
  Observable contract: `Err(BzrError::Api { code: 410, .. })` when the first
  attempt answers 401 code 410 and the retry answers 401 code **300**
  (`invalid_login_or_password`). The two bodies must differ: if the retry
  repeated code 410, `Original` and `Refused` would build the identical error
  from the identical bytes and the test would pass whatever the band check does.
  Test: `auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in`.
  Expected red: not red before the edit — it passes on `main` and exists to fail
  if the new branch over-relays. Confirm it bites by temporarily inverting the
  band check to `!code_proves_auth_failure(code)` and observing it report `300`.
  Green command: `make test-one T=auth_fallback_keeps_the_original`.
  `auth_fallback_band_edges_separate_login_failure_from_refusal` proves the same
  contract independently, at the band's edges.

- Contract: *a relayed per-resource code is suppressible by `--permissive`
  (spec R9).*
  Mode: focused-test.
  Observable contract: `crate::commands::bug::execute` with a two-id
  `--permissive` view returns `Ok`, and the JSON envelope's `failed` array holds
  exactly the bug whose fault arrived through the fallback. Driving the command
  is mandatory — asserting `is_permissive_bug_view_error()` on the error instead
  observes a predicate, not the batch outcome, and never exercises the exit-0
  path the ADR claims to pin.
  Test: `relayed_per_resource_refusal_makes_permissive_view_exit_zero_not_four`.
  Expected red: before Task 1 step 8 the fallback yields `Api { code: 410 }`,
  which `BzrError::is_permissive_bug_view_error` (`src/error.rs:247`) rejects, so
  `execute` returns `Err` and `result.is_ok()` fails.
  Green command:
  `make test-one T=relayed_per_resource_refusal_makes_permissive_view`.

- Contract: *an anonymous client never retries.*
  Mode: focused-test.
  Observable contract: with an anonymous client and a single 401 mock carrying
  `.expect(1)`, the mock server's drop-time verification passes — exactly one
  request reached it.
  Test: `anonymous_client_does_not_retry_401_with_alternate_auth`, which
  **already exists** at `src/client/transport_tests.rs:282`. Read it and confirm
  it still holds after Task 1; add nothing.
  Green command: `make test-one T=anonymous_client_does_not_retry`.

- Contract: *`error_from_status_body` preserves `check_response_status`'s
  existing behaviour for a body with no `code`.*
  Mode: focused-test.
  Observable contract: HTTP 400 with `{"error":true,"message":"boom"}` yields
  `BzrError::Api { code: -1, message: "boom" }`.
  Test: `src/client/response_tests.rs`,
  `error_body_without_a_code_reports_the_unknown_code_sentinel`. It belongs
  there, not in `transport_tests.rs`: no fallback is involved, and that file
  already owns this family (`api_error_with_string_code_parsed_correctly`,
  `api_200_error_without_code_field_uses_minus_one`).
  Expected red: reports a different code if the move drops `ErrorResponse`'s
  `-1` default.
  Green command: `make test-one T=error_body_without_a_code`.

- Contract: *the retried-body-unreadable arm leaks no transport error.*
  Mode: task-test-not-applicable.
  Changed surface: the `let Ok(body) = retried.text().await else { … }` arm in
  `retry_with_alternate_auth`. `wiremock` serves a complete response body from an
  in-process server; it exposes no way to abort a body mid-stream, so no test in
  this suite can enter the arm. It is held by inspection: the arm's only log line
  is a fixed string, and it returns `AlternateAuth::Original`, which reports the
  original response and never the error.

### Steps

1. Read `src/client/response.rs` around `ErrorResponse` (line 14) and
   `check_response_status` (line 450), and `src/client/transport.rs` in full.

2. In `src/client/response.rs`, add two `pub(super)` associated functions to the
   `impl BugzillaClient` block, immediately after `check_response_status`:

   ```rust
       /// Classify an HTTP error status and its already-read body into the
       /// error bzr reports for it. Shared with the 401 alternate-auth
       /// fallback, which must consume the retried body to classify it and so
       /// cannot hand the response back to [`Self::check_response_status`].
       pub(super) fn error_from_status_body(
           status: reqwest::StatusCode,
           body: &str,
       ) -> BzrError {
           tracing::debug!(
               %status,
               body = crate::bugzilla_auth::redact_api_key(crate::http::utf8_prefix(
                   body,
                   BODY_PREVIEW_MAX_BYTES,
               )),
               "API error response"
           );
           if let Ok(err) = serde_json::from_str::<ErrorResponse>(body) {
               if err.error {
                   return BzrError::Api {
                       code: err.code,
                       message: err.message.unwrap_or_else(|| status.to_string()),
                   };
               }
           }
           BzrError::HttpStatus {
               status: status.as_u16(),
               body: crate::http::diagnostic_body_preview(body),
           }
       }

       /// The Bugzilla error code an error body carries, or `None` when the
       /// body is not a Bugzilla error envelope (`error: true`) or carries no
       /// `code`. `ErrorResponse` defaults a missing `code` to
       /// `default_error_code()`, and Bugzilla emits no code of `-1`, so
       /// reading that sentinel back as `None` cannot swallow a real code.
       /// `None` means the response offered no signal about *why* it was
       /// refused.
       pub(super) fn bugzilla_error_code(body: &str) -> Option<i64> {
           let envelope: ErrorResponse = serde_json::from_str(body).ok()?;
           (envelope.error && envelope.code != default_error_code())
               .then_some(envelope.code)
       }
   ```

3. In `src/client/response.rs`, replace the tail of `check_response_status`'s
   error branch — the `tracing::debug!("API error response")` call, the
   `serde_json::from_str::<ErrorResponse>` block, and the trailing
   `BzrError::HttpStatus` return, currently lines 470–489 — with the single
   statement `return Err(Self::error_from_status_body(status, &body));`. Leave
   the preceding `response.text().await` match, including its `Err(e)` arm,
   exactly as it is: that arm's text lands in `HttpStatus.body`, which
   `src/error.rs:37` already redacts.

4. Run `make test-fast` bare. Expect a green `--lib` run — steps 2 and 3 are
   behaviour-preserving, and `ErrorResponse` is unchanged, so no existing test or
   call site moves.

5. Add Task 2's `auth_fallback_relays_a_policy_refusal_from_the_retried_body`
   test now if you are executing strictly test-first, and run
   `make test-one T=auth_fallback_relays_a_policy_refusal` bare. Expect it to
   fail reporting `code: 410`.

6. In `src/client/transport.rs`, add the outcome type and the classification
   policy immediately above `impl BugzillaClient`:

   ```rust
   /// What the 401 alternate-auth retry established about the original refusal.
   enum AlternateAuth {
       /// The retry's response replaces the original 401.
       Replace(reqwest::Response),
       /// The retry authenticated and the server refused the request for a
       /// reason unrelated to authentication. Establishing that consumed the
       /// body, so the refusal travels as the error it will be reported as.
       Refused(BzrError),
       /// The original 401 stands: no retry was possible, or the retry proved
       /// authentication itself failed, or it said nothing that distinguishes
       /// the two.
       Original,
   }

   /// Bugzilla's own taxonomy of authentication failure, from
   /// `Bugzilla/WebService/Constants.pm`: "Authentication errors are usually
   /// 300-400", plus the historical `login_required` at 410. Every other code
   /// Bugzilla maps onto HTTP 401 — 102, 106, 109, 110, 113, 115, 120, 504,
   /// 505 — refuses a caller who did authenticate. Keying on the band rather
   /// than an enumerated list matters because `REST_STATUS_CODE_MAP` is
   /// extended at runtime through the `webservice_status_code_map` hook.
   fn code_proves_auth_failure(code: i64) -> bool {
       const LOGIN_REQUIRED: i64 = 410;
       (300..=399).contains(&code) || code == LOGIN_REQUIRED
   }
   ```

7. In `src/client/transport.rs`, change `send_raw`'s 401 branch from the
   `if let Some(retried) = …` form to a three-arm match:

   ```rust
           if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
               match self.retry_with_alternate_auth(retry_builder).await? {
                   AlternateAuth::Replace(retried) => return Ok(retried),
                   AlternateAuth::Refused(err) => return Err(err),
                   AlternateAuth::Original => {}
               }
           }
           Ok(resp)
   ```

   `Refused` returns a `BzrError::Api`, which `send`'s transient-error arm does
   not match (`is_transient` matches only `BzrError::Http`), so retry behaviour
   is unchanged.

8. In `src/client/transport.rs`, replace the whole of
   `retry_with_alternate_auth` and delete `alternate_auth_failed`:

   ```rust
       /// On 401, retry the request with the alternate auth method (header ↔
       /// query param) and classify what came back. Bugzilla maps fifteen
       /// distinct error codes onto HTTP 401, so the status alone cannot say
       /// whether the retry failed to authenticate or authenticated and was
       /// then refused; only the body can (ADR 0057).
       async fn retry_with_alternate_auth(
           &self,
           retry_builder: Option<RequestBuilder>,
       ) -> Result<AlternateAuth> {
           if self.auth.is_none() {
               return Ok(AlternateAuth::Original);
           }
           let Some(clone) = retry_builder else {
               return Ok(AlternateAuth::Original);
           };
           tracing::debug!("401 received, retrying with alternate auth method");
           let retried = self.apply_alternate_auth(clone)?.send().await?;
           let status = retried.status();
           tracing::debug!(
               url = Self::safe_url(retried.url()),
               status = %status,
               "auth fallback response"
           );
           if status != reqwest::StatusCode::UNAUTHORIZED
               && status != reqwest::StatusCode::FORBIDDEN
           {
               return Ok(AlternateAuth::Replace(retried));
           }
           // Reading the body is the only way to tell the two apart, and it
           // consumes the response — so a refusal is returned as the error it
           // will be reported as rather than as a response.
           let Ok(body) = retried.text().await else {
               // The transport error's `Display` carries the request URL, which
               // on the query-parameter path holds the API key, so nothing
               // derived from it is logged. Nothing was learned; the original
               // stands.
               tracing::debug!("auth fallback response body unreadable, returning original 401");
               return Ok(AlternateAuth::Original);
           };
           match Self::bugzilla_error_code(&body) {
               Some(code) if code_proves_auth_failure(code) => {
                   tracing::debug!(code, "auth fallback also failed to authenticate");
                   Ok(AlternateAuth::Original)
               }
               Some(code) => {
                   tracing::debug!(code, "auth fallback authenticated; server refused the request");
                   Ok(AlternateAuth::Refused(Self::error_from_status_body(
                       status, &body,
                   )))
               }
               None => {
                   tracing::debug!("auth fallback carried no Bugzilla error code");
                   Ok(AlternateAuth::Original)
               }
           }
       }
   ```

9. Run `make test-one T=auth_fallback` bare. Expect every `auth_fallback` test to
   pass, including the one red at step 5.

10. Run `make lint` bare. Expect exit 0. If clippy reports `large_enum_variant`
    on `AlternateAuth`, box the `BzrError` variant rather than adding an
    `expect` attribute.

11. Run `make test` bare, in the background. Expect exit 0.

### Acceptance criteria

- `alternate_auth_failed` no longer exists.
- `retry_with_alternate_auth` returns `AlternateAuth` and reads the retried body
  only for a 401 or 403.
- `check_response_status` and the retry build their `BzrError` through the same
  `error_from_status_body`.
- `ErrorResponse`, `default_error_code`, and `check_bugzilla_200_error` are
  unchanged — that is the part carrying real meaning for the ADR-0015 path.
- Nothing derived from a `reqwest::Error` reaches a log or a message on the new
  body-read path.
- `make lint` and `make test` are green.

---

## Task 2 — wiremock proofs that the two bodies diverge

Where this fits: the only tier that can drive the defect, because it needs the
first attempt to fail authentication while the retry succeeds — a condition a
stock Bugzilla container does not present.

### Interfaces

Consumed, each confirmed present:

- `crate::client::test_helpers::test_client(base_url: &str) -> BugzillaClient` —
  header auth, `retry_max: 0` — `src/client/test_helpers.rs:13`
- `BugzillaClient::get_json_value(path: &str) -> Result<serde_json::Value>`
- `wiremock::{Mock, MockServer, ResponseTemplate}` and
  `wiremock::matchers::{method, path}` — already imported at the top of
  `src/client/transport_tests.rs`
- `BzrError`, in scope through the file's `use super::*`

Published for later tasks: nothing.

### Verification

Every entry below is `Mode: focused-test`, in
`src/client/transport_tests.rs`, green under
`make test-one T=auth_fallback`, except the last row, whose green command is
`make test-one T=relayed_per_resource_refusal_makes_permissive_view`.

| Test | Contract | Expected red |
|---|---|---|
| `auth_fallback_relays_a_policy_refusal_from_the_retried_body` | retried 401 code 120 wins over original 401 code 410 | before Task 1 step 8: reports `410` |
| `auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in` | retried 401 code **300** does not win over original 401 code 410 | under an inverted band check: reports `300` |
| `auth_fallback_keeps_the_original_401_when_the_retry_carries_no_envelope` | retried 401 with an HTML body does not win | if `bugzilla_error_code` returned a code for unparseable input |
| `auth_fallback_keeps_the_original_401_when_the_retried_envelope_has_no_code` | retried 401 `{"error":true,"message":…}` does not win | if the `-1` sentinel were not read back as "no signal" |
| `auth_fallback_relays_a_403_policy_refusal` | retried **403** code 120 wins | if 403 were not classified alongside 401 |
| `auth_fallback_band_edges_separate_login_failure_from_refusal` | 299 → relayed, 300 → kept, 399 → kept, 400 → relayed | if the band were written `300..400` or `300..=400` |
| `relayed_per_resource_refusal_makes_permissive_view_exit_zero_not_four` | a `--permissive` batch whose only fault is a relayed 102 returns `Ok` with that bug in `failed` (spec R9) | before Task 1 step 8: `execute` returns `Err` |

One further case goes in `src/client/response_tests.rs`, not here, because no
fallback is involved: `error_body_without_a_code_reports_the_unknown_code_sentinel`
— a plain HTTP 400 `{"error":true,"message":"boom"}` still reports `code: -1`,
red if Task 1 step 2 dropped `ErrorResponse`'s default. Mount a single 400 mock
and follow the shape of `api_error_with_string_code_parsed_correctly` already in
that file.

### Steps

1. Read `src/client/transport_tests.rs:118–194`. **Do not copy the "LIFO"
   explanation in its comments at lines 135 and 174: it is wrong.** In
   `wiremock` 0.6.5 (`src/mock_set.rs:63-72`) `handle_request` stable-sorts the
   mocks by priority and takes the **first** match, and `register`
   (`src/mock_set.rs:92-96`) pushes in insertion order, so with equal priority
   the **first-registered** mock wins. Those existing tests work because their
   first-registered mock carries extra matchers the first request fails, not
   because of ordering. Correct both comments to say
   `// registered first; discriminated by the auth matchers, not by order` while
   you are in the file.

2. Append this helper, whose ordering is the mechanism the new tests depend on:

   ```rust
   /// Mount the two halves of a 401 alternate-auth fallback on `route`. The
   /// mocks share identical matchers, so ordering alone must separate them:
   /// wiremock stable-sorts by priority and serves the first match, and equal
   /// priorities keep insertion order — so `first` is registered first and
   /// capped at one serve, and the retry falls through to `retried`.
   async fn mount_auth_fallback_on(
       mock: &MockServer,
       route: &str,
       first: ResponseTemplate,
       retried: ResponseTemplate,
   ) {
       Mock::given(method("GET"))
           .and(path(route.to_string()))
           .respond_with(first)
           .up_to_n_times(1)
           .expect(1)
           .mount(mock)
           .await;
       Mock::given(method("GET"))
           .and(path(route.to_string()))
           .respond_with(retried)
           .expect(1)
           .mount(mock)
           .await;
   }

   /// The common case: the fallback on `/rest/bug/1`.
   async fn mount_auth_fallback(
       mock: &MockServer,
       first: ResponseTemplate,
       retried: ResponseTemplate,
   ) {
       mount_auth_fallback_on(mock, "/rest/bug/1", first, retried).await;
   }

   fn bugzilla_error(code: i64, message: &str) -> serde_json::Value {
       serde_json::json!({"error": true, "code": code, "message": message})
   }

   fn login_required() -> ResponseTemplate {
       ResponseTemplate::new(401).set_body_json(bugzilla_error(410, "You must log in"))
   }
   ```

3. Append the relay test, which is the shape every other fallback test in the
   table follows — mount the pair, call `get_json_value("bug/1")`, match the
   error:

   ```rust
   #[tokio::test]
   async fn auth_fallback_relays_a_policy_refusal_from_the_retried_body() {
       let mock = MockServer::start().await;
       mount_auth_fallback(
           &mock,
           login_required(),
           ResponseTemplate::new(401).set_body_json(bugzilla_error(
               120,
               "you are not allowed to restrict bugs to this group in the 'FuncTestProd' product",
           )),
       )
       .await;

       let client = test_client(&mock.uri());
       let err = client.get_json_value("bug/1").await.unwrap_err();
       match err {
           BzrError::Api { code, message } => {
               assert_eq!(code, 120, "the retried body's code must win");
               assert!(
                   message.contains("not allowed to restrict bugs"),
                   "the retried body's message must win: {message}"
               );
           }
           other => panic!("expected a relayed Api error, got {other:?}"),
       }
   }
   ```

4. Append the remaining tests from the Verification table in that same shape.
   Their retried templates are, in order:
   `ResponseTemplate::new(401).set_body_json(bugzilla_error(300, "The username
   or password you entered is not valid"))` — asserting `code == 410` and that
   the message is the original's, which is what makes the pair discriminate;
   `ResponseTemplate::new(401).set_body_string("<html>Proxy Authentication
   Required</html>")`; `ResponseTemplate::new(401).set_body_json(json!({"error":
   true, "message": "refused"}))`; and
   `ResponseTemplate::new(403).set_body_json(bugzilla_error(120, "refused by
   policy"))`. The band-edge test loops
   `for (retried_code, expected) in [(299, 299), (300, 410), (399, 410), (400,
   400)]`, building a fresh `MockServer` per iteration and asserting
   `code == expected` with `"retried code {retried_code}"` as the message.

4a. Append the R9 exit-code pin,
   `relayed_per_resource_refusal_makes_permissive_view_exit_zero_not_four`. It
   drives the command layer, because that is where the exit code is observable.
   No export needs adding: `crate::test_helpers::setup_test_env` is `pub`
   (`src/test_helpers.rs:67`) and seeds `api_key = "test-key"` with
   `auth_method = "header"` — the credentialed header-auth client the fallback
   needs — and `crate::commands::bug::execute` is `pub(crate)`
   (`src/commands/bug/mod.rs:65`).

   Mirror `view_multi_permissive_api_102_suppressed`
   (`src/commands/bug/view_tests.rs:662-691`) for structure, and copy the three
   small fixtures it uses — `make_view_action` (`view_tests.rs:10`),
   `ok_bug_body` (`:22`), `api_error_body` (`:51`) — into
   `transport_tests.rs` rather than exporting them; test-file duplication is
   already excluded from the repo's duplication metric by
   `sonar-project.properties`. The case lives here, not in `view_tests.rs`,
   because the behaviour under test is the transport's relay and `view_tests.rs`
   is outside this change's permitted surface.

   Body: `let (_lock, mock, _tmp) = setup_test_env().await;`, mount an OK bug on
   `/rest/bug/1`, `mount_auth_fallback(&mock, login_required(),
   ResponseTemplate::new(401).set_body_json(api_error_body(102, "Access
   Denied")))` on `/rest/bug/2`, then `crate::commands::bug::execute` with
   `make_view_action(&["1", "2"], true)`, a `CapturedIo`, and a
   `CommandContext::new(None, OutputFormat::Json, None)`. Assert
   `result.is_ok()` and that `failed` holds exactly one entry with `id == "2"`,
   reading it through `crate::test_helpers::json_envelope_data`. Add a comment
   naming ADR 0057's Consequences as the decision this pins.

   `setup_test_env` takes the global `ENV_LOCK`, so this case serialises against
   the command tests. That is the existing norm for command-driving tests, not a
   new cost. Use `mount_auth_fallback_on(&mock, "/rest/bug/2", …)` for the
   fallback half, since this case needs it on a route other than `/rest/bug/1`.

5. Run `make test-one T=auth_fallback` and `make test-one T=error_body_without`
   bare. Expect both green.

6. Prove the relay test bites. Temporarily replace the
   `match Self::bugzilla_error_code(&body)` block in `retry_with_alternate_auth`
   with `return Ok(AlternateAuth::Original);` — the status-only predicate's
   behaviour — run `make test-one T=auth_fallback_relays_a_policy_refusal` bare,
   and confirm it fails reporting `code: 410`. Undo the edit by hand (do not
   `git checkout` the file: it carries uncommitted work) and re-run to confirm
   green.

7. Run `make lint` and `make test` bare. Expect exit 0.

### Acceptance criteria

- Eight new `#[tokio::test]` cases in `src/client/transport_tests.rs`; no inline
  `mod tests` anywhere.
- The two misleading "LIFO" comments at `src/client/transport_tests.rs:135`
  and `:174` corrected.
- The relay test observed red under the restored status-only predicate and green
  after reverting the fault.
- `make check-test-layout` (inside `make lint`) passes.

---

## Task 3 — pin the contract against a real Bugzilla

Where this fits: the mandatory functional tier. It cannot reproduce the masking
— a stock container authenticates the first attempt — and neither direction can
be reddened by mutating `code_proves_auth_failure`. What it pins is the outcome a
user must see, in both the authenticated and the credentialless direction, so a
regression that reintroduces a login-failure message reddens the suite.

### Interfaces

Consumed, each confirmed present in `tests/functional/lib.sh` or in
`tests/functional/phases/08e-bugs-restricted-access.sh`:

- `test_begin <id> <description>`, `test_pass`, `test_fail <msg>`,
  `test_skip <msg>`
- `run_bzr <args…>`, `run_bzr_raw <args…>`, `assert_exit_code <n>`,
  `assert_success`
- `assert_stderr_json <jq-path> <expected>`, `assert_stderr_not_contains <text>`
- `unique_name <prefix>`
- `$RESTRICTED_BUG` (`08e:114`) and the `public` server alias (`08e:164`)

Published for later tasks: nothing.

### Verification

- Contract: *an authenticated, policy-refused write reports the server's own
  error and never a login failure.*
  Mode: focused-test — `08e` test id
  `authenticated-policy-refusal-is-not-reported-as-a-login-failure`.
  Expected red: only with the status-only predicate restored **and** a server
  that 401s the first attempt. Against a stock container it is green either way,
  which is the same limit `08e`'s #504 tests carry and is written into the phase
  comment rather than claimed away.
  Green command: `make functional-test`.
- Contract: *a credentialless write reports the server's own answer for an
  unauthenticated request.*
  Mode: focused-test — `08e` test id
  `credentialless-policy-refused-write-reports-the-servers-own-answer`.
  Expected red: **no mutation of the new code reddens this test**, and the plan
  says so rather than claiming a bite it does not have. Removing the
  `self.auth.is_none()` guard would not change the observable outcome either:
  `apply_alternate_auth` (`src/client/transport.rs:169-187`) strips both
  credentials and then falls through to `_ => Ok(builder)` at `:185` when
  `self.auth` is `None`, so the "retry" is the byte-identical anonymous request
  and the server answers it identically. The guard's own bite is held by the
  existing `anonymous_client_does_not_retry_401_with_alternate_auth`
  (`src/client/transport_tests.rs:282`), whose `.expect(1)` fails if a second
  request is sent. What this functional test pins is the user-visible contract:
  a credentialless write reports the server's own answer with
  `error.type == "api"` and an observed `api_code`.
  Green command: `make functional-test`.

### Steps

1. Read `tests/functional/phases/08e-bugs-restricted-access.sh` in full,
   including its header comment and its 4-space indentation. Note the `unset`
   block at lines 375–377: the new block goes **before** it, after the last test
   at line 373, so it does not depend on variables the unset has cleared.

2. Extend the header comment block with a #715 scope note, after the existing
   #504 scope note and before the `echo "── Phase 8e"` line: state that the 401
   alternate-auth retry used to judge its outcome by HTTP status alone, that
   Bugzilla shares that status across fifteen error codes, that a stock Bugzilla
   cannot reproduce the masking because it needs the first attempt to fail
   authentication while the retry succeeds (the #713 condition), that the
   divergent path is driven by wiremock in `src/client/transport_tests.rs`, and
   that these two tests pin the contract in both directions.

3. Before line 375, add the fixture: `UNAVAILABLE_GROUP=$(unique_name
   unavail-grp)`, then a `test_begin "fixture-group-not-enabled-on-the-product"`
   block that runs `run_bzr group create --name "$UNAVAILABLE_GROUP"
   --description "not enabled on any product"` and `assert_success`. Comment
   that the absence of a `group_control_map` row is deliberate: the group exists
   and `FuncTestProd` does not permit restricting bugs to it, which is the exact
   shape Bugzilla refuses with `group_restriction_not_allowed`
   (`Bugzilla/Bug.pm` throws it when `group_is_settable` is false). Add
   `UNAVAILABLE_GROUP` to the phase's `unset` at 08e:375-377, so the new fixture
   variable is cleared with the other eight rather than leaking past the phase.

4. Add the authenticated direction:

   ```bash
   test_begin "authenticated-policy-refusal-is-not-reported-as-a-login-failure" "authenticated policy refusal is not reported as a login failure"
   if [[ -n "$RESTRICTED_BUG" ]]; then
       run_bzr_raw --json bug update "$RESTRICTED_BUG" --groups-add "$UNAVAILABLE_GROUP"
       # The contract #715 broke: the server's own refusal must survive the
       # alternate-auth fallback. Assert the negative too — "must log in" is the
       # wrong answer this issue was filed about.
       if assert_stderr_json '.error.type' "api" &&
           assert_stderr_not_contains "must log in"; then
           test_pass
       fi
   else test_skip "no restricted bug"; fi
   ```

5. Add the credentialless direction, the same shape, running
   `run_bzr_raw --json --server public bug update "$RESTRICTED_BUG"
   --groups-add "$UNAVAILABLE_GROUP"` under test id
   `credentialless-policy-refused-write-reports-the-servers-own-answer`, and
   asserting `.error.type` is `api`. Comment that with no credentials there is no
   retry at all — `transport.rs`'s `self.auth.is_none()` guard returns before any
   body is read — so this direction pins that the guard is intact.

6. Run `make functional-test` bare, in the background, and read it on
   completion. Both new ids must pass. **Record what the container actually
   returned for each direction**: the exit code and the `.error.api_code`. Do not
   guess either before observing it — the phase's existing anonymous-read test
   answers `102`, not `410`, so the credentialless write's answer is a
   measurement, not a prediction.

7. Fold the observed values back in: add `assert_exit_code <observed>` and
   `assert_stderr_json '.error.api_code' "<observed>"` to both tests, with a
   comment naming the Bugzilla error the code corresponds to. If the authenticated
   write unexpectedly *succeeds* — a container that accepts an unpermitted group
   — do not weaken the assertion: `test_fail` with that observation and report it,
   because the fixture then does not produce a policy refusal and the test proves
   nothing.

8. Run `make lint` bare — it includes `check-shell` and
   `check-functional-test-ids`, both of which read this file. Expect exit 0. If
   `check-functional-test-ids` objects, match the id convention it enforces
   rather than editing the check.

9. Re-run `make functional-test` bare, in the background. Expect exit 0.

### Acceptance criteria

- Two new test ids in `08e`, both green against a real container, both placed
  before the phase's `unset` block, and `UNAVAILABLE_GROUP` added to that block.
- Both directions assert an observed exit code and `api_code`, not a guessed one.
- The scope limit — a stock server cannot reproduce the masking, and neither test
  is reddened by mutating the band — is stated in the phase comment, not implied
  by silence.
- `make lint` and `make functional-test` are green.

---

## Deferrals carried into implementation

None. Every finding from the design review was `accepted-fixed` or
`rejected-with-evidence`; see the run's report.
