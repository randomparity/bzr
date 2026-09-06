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

Expected implementation size: 330–450 changed lines (M) — derived from the file
map below: two small production edits (~60 and ~40 changed lines), one test
block of roughly 200 lines in an existing sibling file, and roughly 70 lines
added to one existing functional phase script.

## Global Constraints

- Unit tests live in **sibling `<name>_tests.rs` files** linked from the source
  file via `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`. An inline `mod
  tests { ... }` in `src/` is forbidden and `make check-test-layout` enforces
  it. `src/client/transport_tests.rs` and `src/client/response_tests.rs` already
  exist and are already linked; add to them, do not create new files.
- Sibling test files begin with the file-level inner attribute they already
  carry. `src/client/transport_tests.rs` begins with
  `#![expect(clippy::unwrap_used)]`. Do not change it.
- All API tests are `#[tokio::test]`; HTTP is mocked with `wiremock`.
- Clippy pedantic is on and warnings are errors: `unwrap_used` is denied in
  `src/` (permitted in the sibling test files by their `expect` attribute),
  `expect_used` and `allow_attributes` warn. Use `#[expect(...)]` with a reason
  rather than `#[allow(...)]` if one is unavoidable.
- User-facing output goes through `Writers`; diagnostics go through `tracing`.
  No `println!`/`eprintln!` in `src/`.
- URLs in logs go through `BugzillaClient::safe_url`; body text in logs goes
  through `crate::bugzilla_auth::redact_api_key`.
- Guardrails, run bare (no pipes, no `|| true`):
  - `make lint`
  - `make test-one T=<substring>` while iterating, `make test-fast` for
    `--lib` only, `make test` before committing
  - `make functional-test` for the container tier (~10 minutes; run it in the
    background, never piped through `tail`)
  - never run bare `cargo test`
- Functional phase scripts use **4-space indent** and are not CI-linted; bare
  `shfmt` will disagree with the repo's actual style. Match the surrounding
  file.
- Do not edit `docs/adr/README.md`, `src/client/auth/valid_login.rs`, or
  `src/client/auth/mod.rs`.
- Commits are Conventional Commits 1.0.0. At least one commit must carry
  `fix(client): …` so the change reaches the generated release notes.

## File map

| File | Created / Changed | Answerable for |
|---|---|---|
| `src/client/response.rs` | Changed | `ErrorResponse` shape; `error_from_status_body`; `bugzilla_error_code` |
| `src/client/transport.rs` | Changed | The `AlternateAuth` outcome, the authentication-code band, `send_raw`'s three-way branch |
| `src/client/transport_tests.rs` | Changed | Wiremock proofs that the two 401 bodies diverge |
| `tests/functional/phases/08e-bugs-restricted-access.sh` | Changed | Contract pinned against a real Bugzilla, authenticated and credentialless |
| `docs/adr/0057-alternate-auth-retry-classifies-the-refusal.md` | Created | The decision |
| `docs/workflow/specs/2026-09-05-issue-715-alternate-auth-retry-body-design.md` | Created | The design |

---

## Task 1 — classify the retry from its body

Where this fits: the whole production change. The `response.rs` half and the
`transport.rs` half are one deliverable — the extracted helper has no other
caller, so landing it alone would be dead code.

### Interfaces

Consumed from the existing codebase (each confirmed present at the stated
signature in this repository at `origin/main`):

- `crate::error::BzrError::Api { code: i64, message: String }` and
  `BzrError::HttpStatus { status: u16, body: String }` — `src/error.rs:18`
- `crate::http::diagnostic_body_preview(body: &str) -> String` — `src/http.rs:71`
- `crate::http::utf8_prefix(body: &str, max_bytes: usize) -> &str` —
  `src/http.rs:62`
- `crate::bugzilla_auth::redact_api_key(msg: &str) -> String` —
  `src/bugzilla_auth.rs:123`
- `BugzillaClient::safe_url(url: &reqwest::Url) -> String` —
  `src/client/transport.rs:180`
- `BugzillaClient::apply_alternate_auth(&self, builder: RequestBuilder) ->
  Result<RequestBuilder>` — `src/client/transport.rs:169`
- `BODY_PREVIEW_MAX_BYTES` — a private constant already in scope in
  `src/client/response.rs`

Published for later tasks:

- `BugzillaClient::error_from_status_body(status: reqwest::StatusCode, body:
  &str) -> BzrError` (`pub(super)`, in `src/client/response.rs`)
- `BugzillaClient::bugzilla_error_code(body: &str) -> Option<i64>`
  (`pub(super)`, in `src/client/response.rs`)
- `enum AlternateAuth { Replace(reqwest::Response), Refused(BzrError), Original }`
  (private to `src/client/transport.rs`)

### Verification

- Contract: *a retried 401 carrying a non-authentication Bugzilla error code
  replaces the original 401*.
  Mode: focused-test.
  Observable contract: `client.get_json_value("bug/1").await` returns
  `Err(BzrError::Api { code: 120, .. })` when the first attempt answers 401 with
  code 410 and the retry answers 401 with code 120.
  Test: `src/client/transport_tests.rs`,
  `auth_fallback_relays_a_policy_refusal_from_the_retried_body`.
  Expected red before the production edit: the assertion on `code == 120` fails,
  reporting `code: 410`.
  Green command: `make test-one T=auth_fallback_relays_a_policy_refusal`.

- Contract: *a retried 401 carrying an authentication code leaves the original
  401 standing*.
  Mode: focused-test.
  Observable contract: `Err(BzrError::Api { code: 410, .. })` when both attempts
  answer 401 with code 410.
  Test: `src/client/transport_tests.rs`,
  `auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in`.
  Expected red: none before the edit — this test passes on `main` and exists to
  fail if the new branch over-relays. Confirm it by temporarily inverting the
  band check (`!code_proves_auth_failure(code)`) and observing it report `120`
  instead of `410`.
  Green command:
  `make test-one T=auth_fallback_keeps_the_original_401_when_the_retry_also`.

- Contract: *`ErrorResponse` with no `code` field still reports `-1`*.
  Mode: focused-test.
  Observable contract: an HTTP 400 body `{"error":true,"message":"boom"}`
  produces `BzrError::Api { code: -1, message: "boom" }`.
  Test: `src/client/transport_tests.rs`,
  `error_body_without_a_code_reports_the_unknown_code_sentinel`.
  Expected red: fails to compile or reports a different code if
  `error_from_status_body` drops the `-1` default while moving it.
  Green command: `make test-one T=error_body_without_a_code`.

### Steps

1. Read `src/client/response.rs` around the `ErrorResponse` struct (top of file)
   and `check_response_status` (near line 450) so the edits below land against
   the current text.

2. In `src/client/response.rs`, change the `ErrorResponse` struct's `code` field
   so an absent `code` is representable. Replace:

   ```rust
       #[serde(default = "default_error_code", deserialize_with = "deserialize_code")]
       code: i64,
   ```

   with:

   ```rust
       #[serde(default, deserialize_with = "deserialize_optional_code")]
       code: Option<i64>,
   ```

3. In `src/client/response.rs`, replace the `default_error_code` function:

   ```rust
   fn default_error_code() -> i64 {
       -1
   }
   ```

   with the sentinel constant and the optional-code adapter:

   ```rust
   /// The code reported for a Bugzilla error envelope that carries no `code`
   /// field. Bugzilla itself never emits a negative code other than -303 and
   /// -32000, so this cannot collide with a real one.
   const UNKNOWN_API_ERROR_CODE: i64 = -1;

   /// `deserialize_code` in an `Option`, so a missing `code` field is `None`
   /// rather than indistinguishable from a server that sent the sentinel.
   fn deserialize_optional_code<'de, D: serde::Deserializer<'de>>(
       deserializer: D,
   ) -> std::result::Result<Option<i64>, D::Error> {
       deserialize_code(deserializer).map(Some)
   }
   ```

4. In `src/client/response.rs`, replace the body of `check_response_status`'s
   error branch with a call to the new shared helper. Replace this block:

   ```rust
               tracing::debug!(
                   %status,
                   body = crate::bugzilla_auth::redact_api_key(crate::http::utf8_prefix(
                       &body,
                       BODY_PREVIEW_MAX_BYTES,
                   )),
                   "API error response"
               );
               if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                   if err.error {
                       return Err(BzrError::Api {
                           code: err.code,
                           message: err.message.unwrap_or_else(|| status.to_string()),
                       });
                   }
               }
               return Err(BzrError::HttpStatus {
                   status: status.as_u16(),
                   body: crate::http::diagnostic_body_preview(&body),
               });
   ```

   with:

   ```rust
               return Err(Self::error_from_status_body(status, &body));
   ```

5. In `src/client/response.rs`, immediately after `check_response_status`, add
   the two shared helpers:

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
                       code: err.code.unwrap_or(UNKNOWN_API_ERROR_CODE),
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
       /// `code`. `None` means the response offered no signal about *why* it
       /// was refused.
       pub(super) fn bugzilla_error_code(body: &str) -> Option<i64> {
           let envelope: ErrorResponse = serde_json::from_str(body).ok()?;
           envelope.error.then_some(envelope.code).flatten()
       }
   ```

6. Run `make test-fast` bare. Expect a green `--lib` run: this step is
   behaviour-preserving, so every existing test still passes.

7. Add the failing test from the Verification inventory's first contract to
   `src/client/transport_tests.rs` (Task 1's test code is written out in Task 2
   below; add that test first if you are executing strictly test-first). Run
   `make test-one T=auth_fallback_relays_a_policy_refusal` bare and confirm it
   reports `code: 410` where `120` was expected.

8. In `src/client/transport.rs`, add the outcome type and the classification
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

9. In `src/client/transport.rs`, replace `send_raw`'s 401 branch. Replace:

   ```rust
           if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
               if let Some(retried) = self.retry_with_alternate_auth(retry_builder).await? {
                   return Ok(retried);
               }
           }
           Ok(resp)
   ```

   with:

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

10. In `src/client/transport.rs`, replace `retry_with_alternate_auth` and delete
    `alternate_auth_failed`. Replace this whole block:

    ```rust
        /// On 401, retry the request with the alternate auth method (header ↔ query param).
        /// Returns `Ok(Some(response))` if the retry should replace the original 401,
        /// `Ok(None)` if the retry also proved auth failed or wasn't possible, or
        /// `Err` on transport-level failures.
        async fn retry_with_alternate_auth(
            &self,
            retry_builder: Option<RequestBuilder>,
        ) -> Result<Option<reqwest::Response>> {
            if self.auth.is_none() {
                return Ok(None);
            }
            let Some(clone) = retry_builder else {
                return Ok(None);
            };
            tracing::debug!("401 received, retrying with alternate auth method");
            let retried = self.apply_alternate_auth(clone)?.send().await?;
            tracing::debug!(
                url = Self::safe_url(retried.url()),
                status = %retried.status(),
                "auth fallback response"
            );
            if Self::alternate_auth_failed(retried.status()) {
                tracing::debug!("auth fallback also failed, returning original 401");
                return Ok(None);
            }
            Ok(Some(retried))
        }

        fn alternate_auth_failed(status: reqwest::StatusCode) -> bool {
            status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN
        }
    ```

    with:

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
                // The transport error's `Display` carries the request URL,
                // which on the query-parameter path holds the API key, so it
                // is not logged. Nothing was learned; the original stands.
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

11. Run `make test-one T=auth_fallback` bare. Expect every `auth_fallback` test
    to pass, including the one that was red at step 7.

12. Run `make lint` bare. Expect exit 0. If clippy reports
    `large_enum_variant` on `AlternateAuth`, box the `BzrError` variant rather
    than adding an `expect` attribute.

13. Run `make test` bare, in the background. Expect exit 0.

### Acceptance criteria

- `alternate_auth_failed` no longer exists.
- `retry_with_alternate_auth` returns `AlternateAuth` and reads the retried body
  only for a 401 or 403.
- `check_response_status` and the retry build their `BzrError` through the same
  `error_from_status_body`.
- Nothing derived from a `reqwest::Error` reaches a log or an error message on
  the new body-read path.
- `make lint` and `make test` are green.

---

## Task 2 — wiremock proofs that the two bodies diverge

Where this fits: the only tier that can drive the defect, because it needs the
first attempt to fail authentication while the retry succeeds — a condition a
stock Bugzilla container does not present.

### Interfaces

Consumed from Task 1 and the existing test module (each confirmed present):

- `crate::client::test_helpers::test_client(base_url: &str) -> BugzillaClient` —
  header auth, `retry_max: 0` — `src/client/test_helpers.rs:13`
- `crate::client::test_helpers::test_client_query_param(base_url: &str) ->
  BugzillaClient` — `src/client/test_helpers.rs`
- `has_no_auth_header(req: &wiremock::Request) -> bool` and
  `has_no_auth_query_param(req: &wiremock::Request) -> bool` — already defined at
  the top of `src/client/transport_tests.rs`
- `BugzillaClient::get_json_value(path: &str) -> Result<serde_json::Value>`
- `wiremock::{Mock, MockServer, ResponseTemplate}`, `wiremock::matchers::{method,
  path, query_param}`

Published for later tasks: nothing.

### Verification

- Contract: *a retried 401 carrying code 120 is relayed*. Mode: focused-test —
  `auth_fallback_relays_a_policy_refusal_from_the_retried_body`, red before
  Task 1 step 8 with `code: 410`, green via
  `make test-one T=auth_fallback_relays_a_policy_refusal`.
- Contract: *a retried 401 carrying code 410 is not relayed*. Mode:
  focused-test — `auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in`,
  red under an inverted band check, green via
  `make test-one T=auth_fallback_keeps_the_original`.
- Contract: *the authentication band's edges*. Mode: focused-test —
  `auth_fallback_band_edges_separate_login_failure_from_refusal`, red if the band
  is written `300..400` exclusive of 399 or inclusive of 400, green via
  `make test-one T=auth_fallback_band_edges`.
- Contract: *`-1` is still reported for an envelope with no `code`*. Mode:
  focused-test — `error_body_without_a_code_reports_the_unknown_code_sentinel`,
  red if Task 1 step 5 drops the default, green via
  `make test-one T=error_body_without_a_code`.

### Steps

1. Read `src/client/transport_tests.rs` lines 119–200 to copy the existing
   two-mock LIFO idiom exactly — the file registers the success mock first and
   the 401 mock second, because wiremock checks mocks last-registered-first.

2. Append a helper to `src/client/transport_tests.rs` that mounts a 401 for the
   header-auth attempt and a caller-chosen response for the query-param retry:

   ```rust
   /// Mount the two halves of a 401 alternate-auth fallback: the first attempt
   /// gets `first`, the alternate-auth retry gets `retried`. Registration order
   /// and `up_to_n_times(1)` are the idiom the fallback tests above already use
   /// — wiremock checks mocks last-registered-first, so the capped `first` mock
   /// serves once and the retry falls through to `retried`.
   async fn mount_auth_fallback(
       mock: &MockServer,
       first: ResponseTemplate,
       retried: ResponseTemplate,
   ) {
       Mock::given(method("GET"))
           .and(path("/rest/bug/1"))
           .respond_with(retried)
           .expect(1)
           .mount(mock)
           .await;
       Mock::given(method("GET"))
           .and(path("/rest/bug/1"))
           .respond_with(first)
           .up_to_n_times(1)
           .expect(1)
           .mount(mock)
           .await;
   }

   fn bugzilla_error(code: i64, message: &str) -> serde_json::Value {
       serde_json::json!({"error": true, "code": code, "message": message})
   }

   fn login_required() -> ResponseTemplate {
       ResponseTemplate::new(401).set_body_json(bugzilla_error(410, "You must log in"))
   }
   ```

3. Append the relay test:

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

4. Append the preserved-auth-failure test:

   ```rust
   #[tokio::test]
   async fn auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in() {
       let mock = MockServer::start().await;
       mount_auth_fallback(&mock, login_required(), login_required()).await;

       let client = test_client(&mock.uri());
       let err = client.get_json_value("bug/1").await.unwrap_err();
       match err {
           BzrError::Api { code, .. } => assert_eq!(code, 410),
           other => panic!("expected Api 410, got {other:?}"),
       }
   }
   ```

5. Append the no-signal tests:

   ```rust
   #[tokio::test]
   async fn auth_fallback_keeps_the_original_401_when_the_retry_carries_no_envelope() {
       let mock = MockServer::start().await;
       mount_auth_fallback(
           &mock,
           login_required(),
           ResponseTemplate::new(401).set_body_string("<html>Proxy Authentication Required</html>"),
       )
       .await;

       let client = test_client(&mock.uri());
       let err = client.get_json_value("bug/1").await.unwrap_err();
       match err {
           BzrError::Api { code, .. } => assert_eq!(code, 410),
           other => panic!("expected the original Api 410, got {other:?}"),
       }
   }

   #[tokio::test]
   async fn auth_fallback_keeps_the_original_401_when_the_retried_envelope_has_no_code() {
       let mock = MockServer::start().await;
       mount_auth_fallback(
           &mock,
           login_required(),
           ResponseTemplate::new(401)
               .set_body_json(serde_json::json!({"error": true, "message": "refused"})),
       )
       .await;

       let client = test_client(&mock.uri());
       let err = client.get_json_value("bug/1").await.unwrap_err();
       match err {
           BzrError::Api { code, .. } => assert_eq!(code, 410),
           other => panic!("expected the original Api 410, got {other:?}"),
       }
   }
   ```

6. Append the 403 and band-edge tests:

   ```rust
   #[tokio::test]
   async fn auth_fallback_relays_a_403_policy_refusal() {
       let mock = MockServer::start().await;
       mount_auth_fallback(
           &mock,
           login_required(),
           ResponseTemplate::new(403).set_body_json(bugzilla_error(120, "refused by policy")),
       )
       .await;

       let client = test_client(&mock.uri());
       let err = client.get_json_value("bug/1").await.unwrap_err();
       match err {
           BzrError::Api { code, .. } => assert_eq!(code, 120),
           other => panic!("expected the relayed Api 120, got {other:?}"),
       }
   }

   #[tokio::test]
   async fn auth_fallback_band_edges_separate_login_failure_from_refusal() {
       // 300 and 399 are inside Bugzilla's authentication band; 299 and 400 are
       // outside it. Each retried code either keeps the original 410 or is
       // relayed as itself.
       for (retried_code, expected) in [(299, 299), (300, 410), (399, 410), (400, 400)] {
           let mock = MockServer::start().await;
           mount_auth_fallback(
               &mock,
               login_required(),
               ResponseTemplate::new(401).set_body_json(bugzilla_error(retried_code, "refused")),
           )
           .await;

           let client = test_client(&mock.uri());
           let err = client.get_json_value("bug/1").await.unwrap_err();
           match err {
               BzrError::Api { code, .. } => {
                   assert_eq!(code, expected, "retried code {retried_code}");
               }
               other => panic!("retried code {retried_code}: expected Api, got {other:?}"),
           }
       }
   }
   ```

7. Append the sentinel test:

   ```rust
   #[tokio::test]
   async fn error_body_without_a_code_reports_the_unknown_code_sentinel() {
       let mock = MockServer::start().await;
       Mock::given(method("GET"))
           .and(path("/rest/bug/1"))
           .respond_with(
               ResponseTemplate::new(400)
                   .set_body_json(serde_json::json!({"error": true, "message": "boom"})),
           )
           .mount(&mock)
           .await;

       let client = test_client(&mock.uri());
       let err = client.get_json_value("bug/1").await.unwrap_err();
       match err {
           BzrError::Api { code, message } => {
               assert_eq!(code, -1);
               assert_eq!(message, "boom");
           }
           other => panic!("expected Api -1, got {other:?}"),
       }
   }
   ```

8. Run `make test-one T=auth_fallback` and `make test-one T=error_body_without`
   bare. Expect both green.

9. Prove the relay test bites: temporarily restore the old predicate by
   replacing the `match Self::bugzilla_error_code(&body)` block in
   `retry_with_alternate_auth` with `return Ok(AlternateAuth::Original);`, run
   `make test-one T=auth_fallback_relays_a_policy_refusal` bare, and confirm it
   fails reporting `code: 410`. Revert the fault with `git checkout --
   src/client/transport.rs` only if nothing else in that file is uncommitted;
   otherwise undo the edit by hand. Re-run and confirm green.

10. Run `make lint` and `make test` bare. Expect exit 0.

### Acceptance criteria

- Seven new `#[tokio::test]` cases in `src/client/transport_tests.rs`, no inline
  `mod tests` added anywhere.
- The relay test observed red under the restored status-only predicate and green
  after reverting the fault.
- `make check-test-layout` (inside `make lint`) passes.

---

## Task 3 — pin the contract against a real Bugzilla

Where this fits: the mandatory functional tier. It cannot reproduce the masking
— a stock container authenticates the first attempt — so it pins the outcome the
user must see, in both the authenticated and the credentialless direction.

### Interfaces

Consumed from the existing functional harness (each confirmed present in
`tests/functional/phases/08e-bugs-restricted-access.sh` or
`tests/functional/lib.sh`):

- `test_begin <id> <description>`, `test_pass`, `test_fail <msg>`,
  `test_skip <msg>`
- `run_bzr <args...>`, `run_bzr_raw <args...>`, `assert_exit_code <n>`,
  `assert_success`
- `assert_stderr_json <jq-path> <expected>`, `assert_stderr_not_contains <text>`
- `unique_name <prefix>`
- `make_bug <args...>` and the `_RA` array of shared create args, both already
  defined earlier in `08e`
- `$RESTRICTED_BUG`, `$BZ_URL`, and the `public` server alias, all already
  established earlier in the run

Published for later tasks: nothing.

### Verification

- Contract: *an authenticated, policy-refused write reports the server's own
  error and never a login failure*. Mode: focused-test —
  `08e` test id `authenticated-policy-refusal-is-not-reported-as-a-login-failure`.
  Expected red: with the status-only predicate restored **and** a server that
  401s the first attempt; against a stock container it is green either way, and
  that limit is written into the phase comment rather than claimed away.
  Green command: `make functional-test`.
- Contract: *a credentialless write still reports a login failure*. Mode:
  focused-test — `08e` test id
  `credentialless-policy-refused-write-still-reports-a-login-failure`. Expected
  red if the band check is inverted so authentication codes are relayed as
  refusals. Green command: `make functional-test`.

### Steps

1. Read `tests/functional/phases/08e-bugs-restricted-access.sh` in full,
   including its header comment and its 4-space indentation.

2. Extend the header comment block with the #715 scope note, after the existing
   #504 scope note and before the `echo "── Phase 8e"` line:

   ```bash
   # #715 (ADR 0057): the 401 alternate-auth retry used to judge its outcome by
   # HTTP status alone, so a policy refusal — which Bugzilla returns at 401,
   # sharing that status with fifteen other error codes — was discarded and the
   # user was told to log in. Like #504 above, a stock Bugzilla cannot reproduce
   # the masking: it needs the first attempt to fail authentication while the
   # retry succeeds, which is the server-side condition in #713. The divergent
   # path is driven by wiremock in `src/client/transport_tests.rs`. What these
   # two tests pin is the contract against a real server, in both directions.
   ```

3. Append the fixture that creates a group the product does **not** allow, at
   the end of the file:

   ```bash
   # ── #715: a policy refusal is not a login failure ────────────────────
   UNAVAILABLE_GROUP=$(unique_name unavail-grp)

   test_begin "fixture-group-not-enabled-on-the-product" "fixture: group not enabled on the product"
   # Deliberately no `group_control_map` row: the group exists, and
   # FuncTestProd does not permit restricting bugs to it. That is the exact
   # shape Bugzilla refuses with `group_restriction_not_allowed`.
   run_bzr group create --name "$UNAVAILABLE_GROUP" --description "not enabled on any product"
   if assert_success; then test_pass; fi
   ```

4. Append the authenticated direction:

   ```bash
   test_begin "authenticated-policy-refusal-is-not-reported-as-a-login-failure" "authenticated policy refusal is not reported as a login failure"
   if [[ -n "$RESTRICTED_BUG" ]]; then
       run_bzr_raw --json bug update "$RESTRICTED_BUG" --groups-add "$UNAVAILABLE_GROUP"
       # The contract #715 broke: the server's own refusal must survive the
       # alternate-auth fallback. Assert the negative too — "must log in" is
       # the wrong answer this issue was filed about.
       if assert_exit_code 4 &&
           assert_stderr_json '.error.type' "api" &&
           assert_stderr_not_contains "must log in" &&
           assert_stderr_not_contains "You must log in"; then
           test_pass
       fi
   else test_skip "no restricted bug"; fi
   ```

   After the first real run, replace the `.error.type` assertion pair with an
   additional `assert_stderr_json '.error.api_code' "<observed>"` using the code
   the container actually returned, and record that code in a comment beside it.
   Do not guess the code before observing it.

5. Append the credentialless direction:

   ```bash
   test_begin "credentialless-policy-refused-write-still-reports-a-login-failure" "credentialless policy-refused write still reports a login failure"
   if [[ -n "$RESTRICTED_BUG" ]]; then
       run_bzr_raw --json --server public bug update "$RESTRICTED_BUG" \
           --groups-add "$UNAVAILABLE_GROUP"
       # The other direction: with no credentials there is no retry, and a
       # genuine authentication failure must still read as one.
       if assert_exit_code 4 && assert_stderr_json '.error.type' "api"; then
           test_pass
       fi
   else test_skip "no restricted bug"; fi
   ```

6. Run `make lint` bare — it includes `check-shell` and
   `check-functional-test-ids`, both of which read this file. Expect exit 0. If
   `check-functional-test-ids` objects, match the id convention it enforces
   rather than editing the check.

7. Run `make functional-test` bare, in the background, and read it on
   completion. Expect exit 0 and the two new test ids reported as passing.
   Capture the observed `api_code` from the authenticated direction and fold it
   back into step 4's assertion, then re-run `make functional-test` bare.

### Acceptance criteria

- Two new test ids in `08e`, both green against a real container.
- The authenticated direction asserts the observed `api_code`, not a guessed
  one.
- The scope limit — a stock server cannot reproduce the masking — is stated in
  the phase comment, not implied by silence.
- `make lint` and `make functional-test` are green.

---

## Deferrals carried into implementation

None yet. Any deferral a `$trial-loop` run disposes of on this branch is
appended here with its owning record path or tracker issue.
