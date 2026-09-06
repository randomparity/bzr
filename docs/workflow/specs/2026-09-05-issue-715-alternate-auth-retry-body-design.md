# Issue #715 — the alternate-auth retry must classify the refusal

- Date: 2026-09-05
- Issue: [#715](https://github.com/randomparity/bzr/issues/715)
- Decision record: [ADR 0057](../../adr/0057-alternate-auth-retry-classifies-the-refusal.md)
- Related: [ADR 0015](../../adr/0015-server-errors-are-never-masked.md), issue #713

## Goal

A Bugzilla refusal that has nothing to do with logging in must be reported as
what it is. Today `bzr bug update <id> --groups-add <group>` reports `api_code
410`, "You must log in", for a request the server authenticated and then refused
under error 120.

## Problem

`src/client/transport.rs`:

- `send_raw` (`:117-135`) sends the request, and on HTTP 401 calls
  `retry_with_alternate_auth`. If that returns `None` it returns the **original**
  401 response.
- `retry_with_alternate_auth` (`:141-163`) re-sends with the other auth method
  and tests the outcome with `alternate_auth_failed(retried.status())`.
- `alternate_auth_failed` (`:165-167`) is `status == UNAUTHORIZED || status ==
  FORBIDDEN`. It never reads the body.

Bugzilla maps fifteen distinct WebService error codes onto HTTP 401
(`Bugzilla/WebService/Constants.pm`, `REST_STATUS_CODE_MAP`): `102 106 109 110
113 115 120 300 301 302 303 304 410 504 505`. Six mean authentication failed;
nine mean an authenticated caller was refused. The predicate therefore cannot
answer the question it is asked, and answers it wrong for the nine.

`check_response_status` (`src/client/response.rs:450`) then turns the original
401 into `BzrError::Api { code: 410, message: "You must log in..." }`.

ADR 0015 settles the same principle for the HTTP-200 error path
(`has_data_fields`, `src/client/response.rs`) and the 100500 search fallback
(`src/client/resources/bug.rs`). It does not reach the transport and does not
decide which of two disagreeing responses wins. ADR 0057 decides that.

## Requirements

- **R1** — The retry's outcome is classified from the retried response's body,
  not from its status alone.
- **R2** — A retried 401/403 whose Bugzilla error code lies outside `300..=399`
  and is not `410` replaces the original response; the user sees that code and
  that message.
- **R3** — A retried 401/403 whose Bugzilla error code lies in `300..=399` or is
  `410` leaves the original 401 standing, unchanged from today.
- **R4** — A retried 401/403 that carries no Bugzilla error envelope, carries an
  envelope with no `code`, or whose body cannot be read, leaves the original 401
  standing. No signal means no change.
- **R5** — A retry that returns any other status keeps today's behaviour: its
  response replaces the original.
- **R6** — An anonymous client, or a request whose body cannot be cloned, keeps
  today's behaviour: no retry, original response returned.
- **R7** — A relayed refusal is reported as `BzrError::Api { code, message }`,
  built identically to what `check_response_status` produces from the same
  status and body. **Where the original 401 was itself a Bugzilla error
  envelope**, `error.type`, the structured-error key set, and the exit code are
  unchanged, and only `api_code` and `message` move.
- **R10** — Where the original 401 carried **no** Bugzilla error envelope and
  the retry's does, the reported error moves from `BzrError::HttpStatus` (exit
  5, `error.type` `"http"`, structured key `status`) to `BzrError::Api` (exit 4,
  `error.type` `"api"`, structured key `api_code`). That is accepted (ADR 0057,
  Consequences, transition (a)) and must be pinned by a test, not discovered.
- **R8** — The relayed body is redacted on the same terms as every other error
  body, and no transport error's `Display` — which carries the request URL, and
  so the API key on the query-parameter path — reaches a log or a message on the
  new code path.
- **R9** — Because `api_code` is read as control flow by
  `BzrError::is_permissive_bug_view_error` (`src/error.rs:247`), a relayed code
  102 makes a previously-fatal `bug view --permissive` / `comment list
  --permissive` batch suppressible, moving that batch's process exit from 4 to
  0. That is accepted (ADR 0057, Consequences) and must be pinned by a test, not
  discovered.

## Design

### Classification

Bugzilla's own taxonomy, quoted from `Constants.pm`: `# Authentication errors
are usually 300-400.` and `# Except, historically, AUTH_NODATA, which is 410.`
So an error code proves authentication failed when it is in `300..=399` or is
exactly `410`. The band, not an enumerated list — `REST_STATUS_CODE_MAP` is
extended at runtime through the `webservice_status_code_map` hook, so an
enumeration of the non-authentication codes could misclassify an extension's
code as a login failure, which is the defect being fixed.

### Where the body is read

Establishing the classification consumes the retried `reqwest::Response`, which
cannot then be handed back to `check_response_status`. So the retry reports an
outcome rather than an `Option<Response>`:

```rust
enum AlternateAuth {
    Replace(reqwest::Response),
    Refused(BzrError),
    Original,
}
```

`send_raw` returns the `Replace` response, returns the `Refused` error, and on
`Original` falls through to the original 401 exactly as today. `Refused` carries
`BzrError::Api`, which `send`'s transient-error arm does not treat as retryable,
so retry behaviour is untouched.

### Shared error construction

`check_response_status`'s status-and-body → `BzrError` logic is extracted to
`BugzillaClient::error_from_status_body(status, body) -> BzrError`, including
its redacted debug line. `check_response_status` calls it; the retry calls it
for `Refused`. One construction site satisfies R7 by construction rather than by
duplication that can drift.

`BugzillaClient::bugzilla_error_code(body) -> Option<i64>` returns the code an
error envelope carries, reusing the existing `ErrorResponse` struct unchanged.
That struct defaults a missing `code` to `-1`, and `bugzilla_error_code` reads
that sentinel back as `None` — "the server said nothing that distinguishes an
authentication failure from a refusal". Bugzilla emits no error code of `-1`, so
the sentinel cannot collide with a real one. Leaving `ErrorResponse` alone
matters beyond diff size: the same struct is read by
`check_bugzilla_200_error`, the ADR-0015-governed HTTP-200 error path, and
changing its `code` to `Option<i64>` would ripple into that path and its tests
for no gain here.

### Failure modes

ADR 0057's Decision items 1–5 enumerate the classification and are the record;
R1–R6 above are the same rules stated as testable requirements. The only case
whose behaviour changes is the fourth: a 401 or 403 carrying a Bugzilla error
code outside `300..=399` and not `410`, which becomes
`Refused(Api { code, message })` instead of the discarded response it is today.
Every other case — a non-401/403 retry, an authentication code, a missing
envelope, a missing `code`, an unreadable body, an anonymous client, a
non-cloneable body — keeps exactly the behaviour it has now.

## Threat model

**Boundary inventory.** This change adds no boundary. It widens one: the
retried response's body, previously discarded unread, now reaches the user as an
error message and reaches `tracing` as a debug preview.

**Actor model.** The untrusted party is the Bugzilla server the operator
configured — including a hostile or compromised one, and any proxy between. bzr
trusts the operator's choice of server URL and its TLS policy (`src/tls/`); it
does not trust the server's response content. The API key is bzr's own secret,
never the server's, so the risk is bzr echoing its own credential back to the
operator's terminal or log, not the server disclosing one.

**Control per boundary.**

- *Retried body → user-facing error message.* Governed by
  `error_from_status_body`, the same function the original-response path uses,
  and then by `BzrError`'s own `Display`, which applies
  `crate::bugzilla_auth::redact_api_key` to `Api.message` (`src/error.rs:16`)
  and to `HttpStatus.body` (`src/error.rs:37`). That existing control is what
  holds R8's user-facing half, and it is also why
  `check_response_status`'s adjacent `<failed to read response body: {e}>` arm
  — which does interpolate a `reqwest::Error` and so its URL — does not leak:
  the text lands in `HttpStatus.body` and is redacted at display. Length is
  bounded by `crate::http::diagnostic_body_preview` for the `HttpStatus` arm.
  The `Api` arm relays the server's `message` field, exactly as it already does
  for every other Bugzilla error; nothing about a 401 makes that body more
  dangerous than the 400 bodies already relayed.
- *Retried body → debug log.* Governed by
  `crate::bugzilla_auth::redact_api_key` over
  `crate::http::utf8_prefix(body, BODY_PREVIEW_MAX_BYTES)` — moved into the
  shared helper unchanged, so the retried body gets the identical treatment.
- *Transport error while reading the retried body.* `reqwest::Error`'s `Display`
  appends ` for url ({url})` with the query string intact
  (`reqwest-0.12.28/src/error.rs:267`), which on the query-parameter auth path
  carries the API key. The body-read failure path therefore logs nothing derived
  from that error, and returns `Original`. The URL it does log goes through
  `Self::safe_url`, which drops the query string entirely. This arm is verified
  by inspection rather than by test: wiremock cannot construct a mid-body
  transport failure, and the plan says so instead of leaving the gap silent.
- *Classification input.* `bugzilla_error_code` parses with `serde_json` into a
  fixed struct; a body that is not JSON, not an object, or not an error envelope
  yields `None` and the pre-existing behaviour.

**Explicitly out of scope.**

- Whether a Bugzilla error message itself discloses more than the operator would
  like. ADR 0015 accepted that deliberately and this change does not revisit it.
- The header-auth verification probe that makes the fallback run on every write
  (#713), owned by a separate change.
- Hardening `redact_api_key` itself. Every `BzrError` variant that can carry
  server or transport text already routes through it —
  `Http` via `format_http_error` (`src/error.rs:193`), `Api` (`:16`) and
  `HttpStatus` (`:37`) via their own `#[error]` attributes — so this change
  inherits a control it does not need to add. Whether that redactor covers every
  encoding an API key could take is a pre-existing question about the redactor,
  not about this path.

## Testing

**Unit (`src/client/transport_tests.rs`, wiremock).** These drive the divergent
path directly, which no container can: a 401 from the first auth method and a
different 401 from the second.

- `auth_fallback_relays_a_policy_refusal_from_the_retried_body` — first attempt
  401 `{"error":true,"code":410,...}`, retry 401 `{"error":true,"code":120,
  "message":"you are not allowed to restrict bugs to this group..."}`. Expect
  `BzrError::Api { code: 120 }` and the 120 message. **This is the test that
  bites**: restoring the status-only predicate makes it report 410.
- `auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in` —
  first attempt 401 code 410, retry 401 code **300** (`invalid_login_or_password`).
  Expect `Api { code: 410 }` and the original's message. The two bodies must
  differ: with identical bodies both branches build the identical error and the
  test discriminates nothing.
- `relayed_per_resource_refusal_makes_permissive_view_exit_zero_not_four` — first
  attempt 401 code 410, retry 401 code 102, through `bug view --permissive`.
  Expect the batch to complete with the bug listed as failed, pinning R9.
- `auth_fallback_keeps_the_original_401_when_the_retry_carries_no_envelope` —
  retry 401 with a non-JSON body. Expect the original's code.
- `auth_fallback_relays_a_403_policy_refusal` — retry 403 with code 120.
- `auth_fallback_keeps_the_original_401_when_the_retried_envelope_has_no_code` —
  retry 401 `{"error":true,"message":"..."}`. Expect the original's code.
- `auth_fallback_band_edges_separate_login_failure_from_refusal` — 300 and 399
  keep the original; 299 and 400 are relayed.
One further case goes in `src/client/response_tests.rs`, beside the rest of that
family rather than with the fallback tests, because no fallback is involved:
`error_body_without_a_code_reports_the_unknown_code_sentinel` — a plain HTTP 400
error envelope with no `code` still reports `-1`, pinning that moving the
construction into `error_from_status_body` preserved `ErrorResponse`'s default.

**Functional (`tests/functional/phases/08e-bugs-restricted-access.sh`).** Pins
the contract against a real server. A stock Bugzilla authenticates the first
attempt, so it cannot reproduce the masking — the same scope limit ADR 0015's
own phase records for #504. Two directions:

- *authenticated policy refusal* — the admin adds a bug to a group that exists
  but is not enabled on `FuncTestProd`. Expect a non-zero exit, `error.type ==
  "api"`, the server's own `api_code`, and stderr that does not say "log in".
- *credentialless* — the same write through `--server public`. **Measured
  against Bugzilla 5.0.6:** `bug update` requires credentials and refuses
  locally with exit 3 and `error.type == "config"` before any HTTP request, so
  there is no server answer on this path and the fallback is never reached. The
  test pins that negative — a credentialless write keeps reporting the local
  credential precondition and does not start reporting an api/auth error.

Both directions pin an **observed** exit code, not a predicted one. The
authenticated direction was measured at exit 4 with `api_code` 120
(`group_restriction_not_allowed`), which is the issue's own reported case; the
credentialless prediction in an earlier draft of this spec — that it would
surface the server's answer — was refuted by the run. Neither functional test
can be reddened by mutating `code_proves_auth_failure`: the credentialless path
never reaches the client, and the authenticated path does not reach the
divergent case on a stock server. What they pin is the contract; the mutation
proof lives in the wiremock tests above.

## Non-goals

- Inspecting the original 401's body to skip the retry (ADR 0057, rejected).
- A new `BzrError` variant or exit code for policy refusals (ADR 0057,
  rejected).
- Any change under `src/client/auth/` — owned by #713.
- The `docs/adr/README.md` index row — owned by the campaign orchestrator.
