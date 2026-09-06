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
must still be detected as header auth — including when that server refuses the anonymous
request outright.

R4. The auth-detection decision matrix in `src/client/auth/mod.rs` must describe the shipped
rule, including what happens when a leg fails or returns a non-2xx status.

R5. Unit tests cover every branch of the rule — honoured header, ignored header,
non-discriminating response, each rejected leg, and the anonymous-refusal case — and are
shown to bite via controlled faults.

R6. A functional phase script exercises auto-detected auth against a real container, and
asserts that the probe actually ran and compared, not only that the outcome was
`query_param`. The credentialless surface never reaches auth detection — the `public` server
is configured with no credentials at all (`tests/functional/phases/01-config.sh:28`), so
`detect_auth_method` is never called and this probe never runs — so the existing
`credentialless-bug-view-omits-time-fields` case is the contrast this change is measured
against and needs no change.

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

The three legs, the comparison rule, which outcomes count as evidence, and why the status is
checked but not compared are ADR 0056's Decision. This section records only what the code
shape adds.

Each leg is reduced to a `ProbeLeg { status, body }`, where `body` is the response parsed as
`serde_json::Value` when it parses and the raw text otherwise. Comparison is over `body`
alone. The ordering — header, anonymous, query-parameter — puts the two cheap disqualifiers
first, so the common case (a server that ignores the header) costs two requests and never
issues the third.

Constant reuse: `AUTH_HEADER_NAME` and `AUTH_QUERY_PARAM` from `crate::bugzilla_auth`, as
the existing probes in this module already do. No new endpoint constant is introduced —
`format!("{base}/rest/user")` matches how `valid_login` and `whoami` build their URLs.

### Error handling

There is no error type. Every failure path resolves to `false`, which returns the caller to
`AuthMethod::QueryParam`, the method `valid_login` proved. The probe can upgrade to header
auth on positive evidence and can never downgrade a working configuration. ADR 0056's
Consequences enumerates the false negatives this accepts.

Each outcome emits one `tracing::debug!` line naming which leg ended the probe, so `-vv`
shows why header auth was or was not preferred — and the functional tier asserts on one of
those lines. No URL is logged (the query-parameter leg carries the API key in its query
string); the messages name the leg, not the request.

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
| Response status → decision | Only a success, or a `401`/`403` on the anonymous leg, counts as evidence; every other status ends the probe. The header and query-parameter legs must additionally be `is_success()`. A truthy top-level `error` key fails any leg. |
| Credential → request | Header leg uses the already-validated `HeaderValue`; query-parameter leg uses `reqwest`'s `.query()` encoding. Neither is interpolated into a URL string. |
| Anonymous leg → server | Carries the configured login as `names=`. The server already holds that value. No credential is attached. |
| Probe outcome → auth selection | A `false` returns the method `valid_login` proved. Only a positive changes the selection. |

**Explicitly out of scope.** Each of these is a decision recorded in ADR 0056's Consequences
or Considered & rejected, not restated here:

- A hostile server steering bzr onto header auth by returning an authenticated-looking body
  to the header leg. It gains nothing: it is the party that would have to honour the header,
  and it already decides what every read returns.
- Existing installs already holding `auth_method = "header"`, which are never re-detected.
- The API key travelling in the query-parameter probe URL.
- The alternate-auth retry in `src/client/transport.rs` — issue #715, being fixed
  concurrently.
- The absent ad-hoc `--server-auth-method` override for the inline-server surface, raised in
  #713's follow-up comment as this defect's missing workaround.

## Testing

**Unit (`wiremock`, sibling `*_tests.rs` files).**

| Case | Server shape | Expected |
|---|---|---|
| header ignored | same body to header and anonymous, richer one to query-param | `false`, and the query-param leg is never issued |
| header honoured | rich body to header and query-param, thin body to anonymous | `true` |
| anonymous refused | rich body to header and query-param, `401` to anonymous | `true` — the refusal is the discrimination |
| endpoint cannot discriminate | the same body to all three legs | `false` |
| header leg non-2xx | `401` carrying the same body the query-param leg returns | `false`, and only one request is issued |
| query-param leg non-2xx | `403` carrying the same body the header leg returns | `false` |
| anonymous leg fails | `503` to anonymous; header and query-param 200 with the same body | `false` |
| credentialed legs carry a 200 error | header and query-param both 200 `{"error": true, …}`; anonymous the thin body | `false` |
| header matches neither | three distinct bodies | `false` |

The two existing end-to-end cases in `src/client/auth/mod_tests.rs` are re-pointed from
`rest/bug` to `rest/user`: `valid_login_query_param_but_header_works_on_api` becomes the R3
regression test, and `valid_login_query_param_and_header_fails_on_api` becomes the
non-2xx-header-leg case. A new `valid_login_query_param_survives_anonymously_readable_endpoint`
is the R1 regression test — the #713 defect itself. All three assert the detected
`AuthMethod`, which is the contract. ADR 0056 records that R3 has no functional-tier test.

**Bite check (R5).** Three controlled faults, each falsifying a different guard: (A) the
differential logic removed — a single header request, `true` on any 2xx; (B) the header
leg's `is_success()` check deleted; (C) an unconditional `false`. Every new or re-pointed
case must go red under one of them, and green after the revert. Restoring the *old endpoint*
is not a usable fault: no mock answers `/rest/bug`, wiremock returns 404, and the faulted
probe would return `false` — which is what most of these cases assert.

**Functional (R6).** `tests/functional/phases/02-server-auth.sh` uses the existing `auto`
server (configured in `01-config.sh` with no `--auth-method`, so detection runs on first
network use) to assert two things on its first use, under `RUST_LOG=bzr=debug`: that the
probe's own debug line reports it ended by matching the anonymous response — which is what
proves the probe ran, both legs completed, and the shipped `?names=` form reached a real
endpoint — and that the persisted method is `query_param`. Both are gated to `bz50` and
`bz52`: the `bz53` image serves `rest/whoami`, so `detect_whoami_auth` resolves the method
and the `valid_login` fallback this change lives in is never entered there. On `bz53` the
case skips with that reason rather than asserting a value nothing under test produced.

`08-bugs.sh` asserts that a credentialed read through the `auto` server returns the
permission-gated time fields the existing credentialless case asserts are absent. That one
is version-independent — it asserts the user-visible outcome, which must hold whichever
detection path resolved the method.

The default `test` server pins `--auth-method query_param`, which is why the existing suite
never exercised this path at all.
