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

ADR 0015 settles the same principle for `src/client/resources/bug.rs`; it does
not reach the transport and does not decide which of two disagreeing responses
wins. ADR 0057 decides that.

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
  status and body. Exit code, `error.type`, and the structured-error key set are
  unchanged.
- **R8** — The relayed body is redacted on the same terms as every other error
  body. No new unredacted logging of a body, URL, or transport error.

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
error envelope carries. `ErrorResponse::code` becomes `Option<i64>`, so "no
`code` field" is representable rather than colliding with the `-1` sentinel;
`error_from_status_body` applies the `-1` default at the point of use, which is
the only place it was ever observable.

### Failure modes

| Retry outcome | Result |
|---|---|
| 2xx / 3xx | `Replace` — unchanged |
| 4xx/5xx other than 401/403 | `Replace` — unchanged |
| 401/403, code in `300..=399` or `410` | `Original` — unchanged |
| 401/403, any other Bugzilla code | `Refused(Api{code, message})` — **new** |
| 401/403, no envelope or no `code` | `Original` — unchanged |
| 401/403, body unreadable | `Original` — unchanged, fail-safe |
| no credentials, or non-cloneable body | `Original` — unchanged |

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
  `error_from_status_body`, the same function the original-response path uses.
  Length is bounded by `crate::http::diagnostic_body_preview` for the
  `HttpStatus` arm. The `Api` arm relays the server's `message` field, exactly
  as it already does for every other Bugzilla error; nothing about a 401 makes
  that body more dangerous than the 400 bodies already relayed.
- *Retried body → debug log.* Governed by
  `crate::bugzilla_auth::redact_api_key` over
  `crate::http::utf8_prefix(body, BODY_PREVIEW_MAX_BYTES)` — moved into the
  shared helper unchanged, so the retried body gets the identical treatment.
- *Transport error while reading the retried body.* `reqwest::Error`'s `Display`
  includes the request URL, which on the query-parameter auth path carries the
  API key. The body-read failure path therefore logs nothing derived from that
  error, and returns `Original`. The URL that is logged goes through
  `Self::safe_url`, which strips the query string.
- *Classification input.* `bugzilla_error_code` parses with `serde_json` into a
  fixed struct; a body that is not JSON, not an object, or not an error envelope
  yields `None` and the pre-existing behaviour.

**Explicitly out of scope.**

- Whether a Bugzilla error message itself discloses more than the operator would
  like. ADR 0015 accepted that deliberately and this change does not revisit it.
- The header-auth verification probe that makes the fallback run on every write
  (#713), owned by a separate change.
- `BzrError::Http`'s own `Display` on other transport paths, which is
  pre-existing and untouched here.

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
  both 401 with code 410. Expect `Api { code: 410 }`.
- `auth_fallback_keeps_the_original_401_when_the_retry_carries_no_error_envelope`
  — retry 401 with a non-JSON body. Expect the original's code.
- `auth_fallback_relays_a_403_policy_refusal` — retry 403 with code 120.
- `auth_fallback_keeps_the_original_401_when_the_retried_envelope_has_no_code` —
  retry 401 `{"error":true,"message":"..."}`. Expect the original's code.
- Boundary cases for the band: 300 and 399 keep the original; 299 and 400 are
  relayed.

**Functional (`tests/functional/phases/08e-bugs-restricted-access.sh`).** Pins
the contract against a real server. A stock Bugzilla authenticates the first
attempt, so it cannot reproduce the masking — the same scope limit ADR 0015's
own phase records for #504. Two directions:

- *authenticated policy refusal* — the admin adds a bug to a group that exists
  but is not enabled on `FuncTestProd`. Expect exit 4, `error.type == "api"`,
  the server's own `api_code`, and stderr that does not say "log in".
- *credentialless* — the same write through `--server public`. Expect exit 4 and
  a login-required answer, proving the genuine-auth-failure direction is intact.

## Non-goals

- Inspecting the original 401's body to skip the retry (ADR 0057, rejected).
- A new `BzrError` variant or exit code for policy refusals (ADR 0057,
  rejected).
- Any change under `src/client/auth/` — owned by #713.
- The `docs/adr/README.md` index row — owned by the campaign orchestrator.
