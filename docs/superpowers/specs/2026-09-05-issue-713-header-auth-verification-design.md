# Issue #713 — header-auth verification must be able to say no

Date: 2026-09-05
Issue: https://github.com/randomparity/bzr/issues/713
Decision record: [ADR 0056](../../adr/0056-differential-header-auth-verification.md)

## Problem

`verify_header_auth_via_rest` (`src/client/auth/valid_login.rs`) probes
`GET /rest/bug?limit=1` with `X-BUGZILLA-API-KEY` and treats any 2xx as proof that header
auth works. Stock Bugzilla answers that endpoint 200 anonymously, so the probe returns true
whether or not the header was honoured. Its call site (`src/client/auth/mod.rs`, the
`ValidLoginOutcome::Authenticated` arm) uses that true to discard a correct
`AuthMethod::QueryParam` result and return `AuthMethod::Header`. Every subsequent REST read
then runs unauthenticated, returning 200 with a narrower body — private comments and
time-tracking fields silently absent.

## Requirements

R1. The verification step must be capable of returning false when the server ignores the
header. A response a header-unaware server would return anonymously must not confirm header
auth.

R2. When verification cannot distinguish an authenticated response from an anonymous one,
bzr keeps the auth method `valid_login` already proved.

R3. A server that rejects header auth via `valid_login` but honours it on real API endpoints
must still be detected as header auth.

R4. The auth-detection decision matrix in `src/client/auth/mod.rs` must describe the shipped
rule, including what happens when a leg fails or returns a non-2xx status.

R5. Unit tests cover the honoured-header, ignored-header, non-discriminating-response and
failed-leg paths, and are shown to bite via a controlled fault.

R6. A functional phase script exercises auto-detected auth against a real container,
including the credentialless path.

## Design

### Component

One function, `verify_header_auth_via_rest`, in `src/client/auth/valid_login.rs`. It keeps
its module and visibility (`pub(super)`), gains `api_key: &str` and `login: &str`
parameters, and keeps returning `bool` — `true` means "prefer header", and the caller's
condition is unchanged. Nothing about the surrounding detection flow moves.

Signature:

```rust
pub(super) async fn verify_header_auth_via_rest(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
    login: &str,
) -> bool
```

Both new arguments are already in scope at the single call site in `detect_auth_method`:
`api_key` is a parameter of that function and `login` is the binding from
`if let Some(login) = email`.

### Data flow

All three requests are `GET {base}/rest/user` with `names={login}`, differing only in how
(or whether) they authenticate. Each is reduced to a `(StatusCode, ProbeBody)` pair, where
`ProbeBody` is the body parsed as `serde_json::Value` when it parses and the raw text
otherwise. The comparison is over that pair.

**A leg that does not complete with a success status ends the probe as not confirmed.** That
covers a transport error, an unreadable body, and any non-2xx status, on any of the three
legs — not only the header one. An anonymous leg that fails for a transient reason would
otherwise be unequal to the header response for a non-auth reason, and on a server whose
`rest/user` record does not discriminate the query-parameter leg then matches the header
leg, confirming header auth on the strength of an error.

1. **Header request** — `X-BUGZILLA-API-KEY: {api_key}`.
2. **Anonymous request** — no credentials. Pair equal to the header pair → `false` (the
   header changed nothing, or the endpoint cannot discriminate).
3. **Query-parameter request** — `Bugzilla_api_key={api_key}`. Return `true` if and only if
   its pair equals the header pair.

The ordering is deliberate: the two cheap disqualifiers run first, so the common case (a
server that ignores the header) costs two requests and never issues the third.

Constant reuse: `AUTH_HEADER_NAME` and `AUTH_QUERY_PARAM` from `crate::bugzilla_auth`, as
the existing probes in this module already do. No new endpoint constant is introduced —
`format!("{base}/rest/user")` matches how `valid_login` and `whoami` build their URLs.

### Error handling

There is no error type. Every failure path resolves to `false`, which returns the caller to
`AuthMethod::QueryParam`, the method `valid_login` proved. The probe can upgrade to header
auth on positive evidence and can never downgrade a working configuration. ADR 0056's
Consequences enumerates the false negatives this accepts.

Each outcome emits one `tracing::debug!` line naming which leg ended the probe, so `-vv`
shows why header auth was or was not preferred. No URL is logged (the query-parameter leg
carries the API key in its query string); the messages name the leg, not the request.

### Observability

The existing `tracing::info!` at the call site ("header auth works on API endpoints despite
valid_login rejecting it; preferring header") stays and now fires only on real evidence.

## Threat model

**Boundary inventory.** This change adds no boundary. It crosses the one existing boundary —
bzr to the configured Bugzilla server, over the connection's TLS policy — twice more per
uncached detection in the `valid_login` branch. What enters bzr is an HTTP status and a
response body from the configured server. The body is now *parsed* (as untyped JSON) rather
than only compared, which is the one control change this design makes.

**Actor model.** The untrusted party is the configured Bugzilla server, and anyone able to
answer for it — a trust the TLS layer establishes, not this probe. The user's credential and
email are already trusted inputs from `config.toml` or the environment. This design places
its trust in exactly one place: that a server able to serve `rest/user` differently to an
authenticated caller than to an anonymous one has thereby demonstrated it read the
credential. A server that controls all three responses can make them say anything; it can
already do that, and it already decides what bzr may read.

**Control per boundary.**

| Boundary | Control |
|---|---|
| Response body → parse | `serde_json::from_str::<serde_json::Value>`, whose 128-level recursion limit bounds a nested server-controlled body. A parse failure is not an error: the body falls back to raw-text comparison. No value is deserialized into a typed struct, rendered, or logged. |
| Response body → buffering | `Response::text()` buffers the whole body with no size cap; the only bound is the connection's `request_timeout` (`src/tls/mod.rs`). This is identical to the existing `whoami` and `valid_login` probes, which call `.text()` the same way — no new exposure, and no size control claimed. |
| Response status → decision | Every leg must be `is_success()`; otherwise the probe ends. A non-2xx on any leg never contributes to a positive. |
| Credential → request | Header leg uses the already-validated `HeaderValue`; query-parameter leg uses `reqwest`'s `.query()` encoding. Neither is interpolated into a URL string. |
| Anonymous leg → server | Carries the configured login as `names=`. The server already holds that value. No credential is attached. |
| Probe outcome → auth selection | A `false` returns the method `valid_login` proved. Only a positive changes the selection. |

**Explicitly out of scope.**

- A hostile server that returns an authenticated-looking body to the header leg to steer bzr
  onto header auth. It gains nothing: it is the party that would have to honour the header,
  and it already decides what every read returns.
- **Existing installs already holding `auth_method = "header"`.** A server with both
  `auth_method` and `api_mode` cached is never re-detected, so a user the old probe already
  misconfigured keeps the wrong value after upgrading and keeps losing data silently. The
  remedy is re-running that server's full `bzr config set-server` line, which replaces the
  entry and resets detected settings. Automatic invalidation would be a persisted-config
  migration — a separate decision, recorded in ADR 0056 rather than taken here.
- The alternate-auth retry in `src/client/transport.rs` that judges a retry on HTTP status
  alone — issue #715, being fixed concurrently.
- The absent ad-hoc `--server-auth-method` override for the inline-server surface, raised in
  #713's follow-up comment as this defect's missing workaround.
- The API key travelling in the query-parameter probe URL. Unchanged from the existing
  `whoami` and `valid_login` query-parameter probes and inherent to probing that method.

## Testing

**Unit (`wiremock`, sibling `*_tests.rs` files).**

| Case | Server shape | Expected |
|---|---|---|
| header ignored | `rest/user` returns the same body to header and anonymous, a richer one to query-param | `false` → `QueryParam` |
| header honoured | rich body to header and query-param, thin body to anonymous | `true` → `Header` |
| endpoint cannot discriminate | the same body to all three legs | `false` |
| header leg non-2xx | 401 to the header leg | `false`, and the other legs are never issued |
| anonymous leg fails | 503 to the anonymous leg; header and query-param both 200 with the same body | `false` |
| header matches neither | three distinct bodies | `false` |

The two existing end-to-end cases in `src/client/auth/mod_tests.rs` —
`valid_login_query_param_but_header_works_on_api` and
`valid_login_query_param_and_header_fails_on_api` — are re-pointed from `rest/bug` to
`rest/user` with the new response shapes; they continue to assert the detected
`AuthMethod`, which is the contract, and are the R3 and R1 regression tests respectively.

**Bite check (R5).** Replace the probe body with a single header request to
`rest/user?names={login}` returning `true` on any 2xx — the differential logic removed while
the endpoint stays the one the tests mock — confirm the ignored-header and
non-discriminating cases go red, then revert. Restoring the *old endpoint* would not work as
a fault: no mock answers `/rest/bug`, wiremock returns 404, and the faulted probe would
return `false`, which is what those tests assert.

**Functional (R6).** `tests/functional/phases/02-server-auth.sh` uses the existing `auto`
server (configured in `01-config.sh` with no `--auth-method`, so detection runs) to assert
that a stock container resolves to `query_param`, and `08-bugs.sh` asserts that a
credentialed read through that server returns the permission-gated time fields the existing
credentialless case asserts are absent. The default `test` server pins
`--auth-method query_param`, which is why the existing suite never exercised this path.
