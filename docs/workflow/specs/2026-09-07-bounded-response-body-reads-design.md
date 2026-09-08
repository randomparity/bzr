# Bounded response-body reads

- Issue: [#740](https://github.com/randomparity/bzr/issues/740)
- Decision: [ADR 0068](../../adr/0068-response-bodies-are-read-under-one-shared-bound.md)
- Status: Design
- Date: 2026-09-07

## Problem

`bzr` buffers every HTTP response body into memory with no ceiling. Thirteen
call sites across six files reach `reqwest::Response::text()` directly, and
together they are the whole client-side trust boundary for server-controlled
data — every REST and XML-RPC response passes through one of them. A hostile or
malfunctioning server returns an arbitrarily large body and the client buffers
all of it until the allocator or the OOM killer stops it.

Seven sites propagate a read failure with `?`. Six degrade instead:
`src/client/response.rs:456` reports `HttpStatus` with a `<failed to read…>`
preview, `src/xmlrpc/protocol/client.rs:79` substitutes the same preview,
`src/client/transport.rs:196` returns `AlternateAuth::Original`,
`src/client/version.rs:93` returns `(None, ApiMode::XmlRpc)`, and the three
auth probes in `src/client/auth/` return `NetworkError` or `None`. That split
is what the design has to preserve.

## Architecture

One helper in `src/http.rs` — the module that already owns transport constants
and body-preview primitives, and that all six files already import — replaces
`.text()` at all thirteen sites:

```rust
pub(crate) const MAX_RESPONSE_BODY_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) enum BodyReadError {
    TooLarge { operation: String, limit_bytes: u64 },
    Transport(reqwest::Error),
}

pub(crate) async fn read_body_bounded(
    response: reqwest::Response,
    operation: &str,
) -> std::result::Result<String, BodyReadError>;
```

It delegates to a private `read_body_within(response, operation, limit_bytes)`
that streams `Response::chunk()` into a `Vec<u8>`, refusing as soon as the
accumulated length would exceed `limit_bytes`, and decodes with
`String::from_utf8_lossy`. The limit parameter is what makes the bound provable
without a 64 MiB fixture.

The decode is byte-for-byte what `.text()` does today: `reqwest` is built
`default-features = false` with `["json", "rustls-tls-native-roots"]`
(`Cargo.toml`), so its `charset` feature is off and `Response::text()` reduces
to `bytes()` plus `String::from_utf8_lossy` (reqwest 0.12.28,
`src/async_impl/response.rs:163-175`). Nothing but the bound changes.

Returning `BodyReadError` rather than `BzrError` lets each site keep the
disposition it has today for an unreadable body and gain one of the same shape
for an over-limit one. `From<BodyReadError> for BzrError` maps `Transport` to
`BzrError::Http` and `TooLarge` to a new `BzrError::ResponseTooLarge`, so the
seven propagating sites keep using `?`.

## Scope

- `src/http.rs` — the limit, the error type, the helper, the `From` impl.
- The thirteen call sites. The six degrading ones gain a `TooLarge` arm that
  takes the same degraded path under a distinct `tracing` line, because an
  over-limit probe body is exactly as uninformative as an unreadable one.
- `src/error.rs` — `ResponseTooLarge { operation, limit_bytes }`, exit code 16,
  type `response_too_large`, both fields published as structured keys. It is
  **not** a transport failure, so Hybrid mode does not answer an over-limit
  REST body by re-fetching it over XML-RPC.
- `schemas/error.json` — `exit_code` maximum 15 → 16 plus the two optional
  keys; `SCHEMA_VERSION` 3.0.6 → 3.0.7 (patch: additive, ADR 0007) across its
  twelve live pins.
- `docs/bzr-cli.md` — exit-code row, structured-key rows, version pin.
- `tests/functional/phases/18b-http-error-preview.sh` — a second fixture case.

### Non-goals

Operator-approved 2026-09-07 and frozen in the issue's `WORK:SCOPE`: streaming
raw attachment downloads via `attachment.cgi` (owner: follow-up issue), so a
single attachment still arrives base64-encoded inside a REST JSON body and
inherits this bound; a user-facing limit override, deferred until a deployment
reports the default too low — this design shows one default suffices, so no
environment escape hatch is promoted either; bounding request or upload bodies,
this change being server-to-client only; and `Content-Length` pre-rejection as
the bound. The last is not implemented at all: an attacker writes that header,
so it could only ever be a fast path, and a fast path indistinguishable from
the check behind it is a branch no test can hold honest.

### Why 64 MiB

The largest legitimate body is one attachment fetch: `download_attachment`
reads `GET /rest/bug/attachment/<id>`, whose `data` field is base64
(`src/client/resources/attachment.rs:241`). Base64 costs 4/3, so 64 MiB admits
a ~48 MiB attachment — far above Bugzilla's documented `maxattachmentsize`
default of 1000 KB and above the 10240 KB Mozilla's own installation runs.
Every other large payload is smaller: attachment listings pass
`exclude_fields=data` (`get_attachments_rest`), and a comment thread is text.

## Threat model

**Boundary.** One, widened by no part of this change and narrowed by all of it:
the HTTP response body crossing from a Bugzilla server into `bzr`'s address
space. The thirteen sites are its whole surface. No new entry point, parser, or
permission.

**Actors.** The untrusted party is the configured Bugzilla server and anyone
who can answer for it — a hostile host at a URL the operator typed or imported,
a MITM on a plaintext or mistrusted-TLS connection, or a compromised or
malfunctioning server the operator does trust. `bzr` trusts the operator's
config and the TLS pin policy; it does not trust what the server sends, which
is why the body is bounded rather than assumed well-behaved.

**Control.** A streaming read that refuses past `MAX_RESPONSE_BODY_BYTES`,
applied before any parse, at the one point every body passes through. On
failure it leaks the limit and the operation name — no body bytes and no URL,
so no API key can ride out in a query parameter. It composes with the existing
controls unchanged: `redact_api_key` on every logged body,
`diagnostic_body_preview` on every embedded excerpt, and the per-request
timeout, which bounds duration but not size and so does not cover this.

**Out of scope.** Request and upload bodies (`bzr` produces them). Total memory
across concurrent requests — `bzr` issues one request at a time per command, so
the process ceiling is one body. A pathological JSON document under the bound:
`serde_json`'s recursion limit answers that, and nothing here weakens it. A
server that streams just under the bound repeatedly: the request timeout covers
it, a size bound cannot.

## Success

1. No response body is buffered past `MAX_RESPONSE_BODY_BYTES` at any of the
   thirteen sites.
2. An over-limit body on a propagating path fails with
   `BzrError::ResponseTooLarge`, exit 16, naming the limit and the operation.
3. An over-limit body on a probe path takes that probe's existing degraded path
   and logs a distinct warning.
4. `is_transport_failure` is `false` for the new variant.
5. `bzr schema error` publishes an `exit_code` maximum of 16.
6. Bodies under the limit are returned byte-identically to `.text()` today.

## Validation

| Contract | Mode | Evidence |
|---|---|---|
| Streaming bound refuses past the limit | `focused-test` | `src/http_tests.rs`: `read_body_within` at limit-1, limit, and limit+1 with a 16-byte limit; the last returns `TooLarge` carrying the limit and operation |
| Transport failure stays distinguishable | `focused-test` | `src/http_tests.rs`: a raw `TcpListener` that truncates its declared body yields `BodyReadError::Transport` |
| The wrapper binds the real constant | `focused-test` | `src/http_tests.rs`: `read_body_bounded` returns a normal body; a second case pins `MAX_RESPONSE_BODY_BYTES == 64 * 1024 * 1024` |
| Error contract of the new variant | `focused-test` | `src/error_tests.rs`: exit code 16, type `response_too_large`, structured keys, `is_transport_failure() == false` |
| Published schema admits exit 16 | `focused-test` | `src/main_tests.rs::format_dispatch_error_json_family_matches_published_schema`, extended with a `ResponseTooLarge` case, plus phase 18b's `bzr schema error` assertion |
| End-to-end refusal on a real socket | `focused-test` | `tests/functional/phases/18b-http-error-preview.sh`: a local fixture streams past the limit; `bzr` exits 16 with `.error.type == "response_too_large"` |
| No `.text()` call survives | `focused-test` | `rg -n "\.text\(\)" src/` reports no match outside `*_tests.rs` |
| Probe degradation on an over-limit body | `task-test-not-applicable` | The `TooLarge` arm enters the same degraded return each site already takes for an unreadable body, which those sites' existing tests pin; no observation distinguishes the two arms beyond the warning text, and asserting log wording tests prose |
