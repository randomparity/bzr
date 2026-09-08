# Bounded response-body reads

- Issue: [#740](https://github.com/randomparity/bzr/issues/740)
- Decision: [ADR 0068](../../adr/0068-response-bodies-are-read-under-one-shared-bound.md)
  — the limit's grounds, rejected alternatives, and accepted residuals.
- Status: Design
- Date: 2026-09-07

## Problem

`bzr` buffers every HTTP response body into memory with no ceiling. Thirteen
call sites across six files reach `reqwest::Response::text()` directly, and
together they are the whole client-side trust boundary for server-controlled
data — every REST and XML-RPC response passes through one of them. A hostile or
malfunctioning server returns an arbitrarily large body and the client buffers
all of it until the allocator or the OOM killer stops it. Seven propagate a
read failure with `?`; six degrade instead, to a `<failed to read…>` preview,
`AlternateAuth::Original`, `(None, ApiMode::XmlRpc)`, `NetworkError`, or
`None`.

## Architecture

ADR 0068 carries the rationale for every choice below; the plan carries the
exact signatures. This section states the shape.

One helper in `src/http.rs` — the module that already owns transport constants
and body-preview primitives, and that all six files already import — replaces
`.text()` at all thirteen sites. `MAX_RESPONSE_BODY_BYTES` is 64 MiB;
`read_body_bounded(response, operation)` delegates to
`read_body_within(response, operation, limit_bytes)`, which streams
`Response::chunk()` into a `Vec<u8>`, refusing as soon as the accumulated
length would exceed `limit_bytes`. The limit parameter is what makes the bound
provable without a 64 MiB fixture. The decode is what `.text()` does today,
reusing the buffer on the valid-UTF-8 path rather than copying: byte-identical
output at half the peak.

It returns a two-variant `BodyReadError` (`TooLarge`, `Transport`) rather than
a `BzrError`, so each site keeps the disposition it has today for an unreadable
body. `From<BodyReadError> for BzrError` maps `Transport` to `BzrError::Http`
and `TooLarge` to a new `BzrError::ResponseTooLarge`, so propagating sites keep
using `?`.

**Ten sites propagate; three degrade.** The auth and version probes move to
propagating — ADR 0068 records why — via a new `ProbeRefused` outcome on
`WhoamiOutcome` and `ValidLoginOutcome` that `detect_auth_method` returns. The
three that still degrade reach a value that is already a refusal or an
inconclusive leg: `AlternateAuth::Original`, `read_probe_leg`'s `None`, and the
XML-RPC HTTP-error preview string.

## Scope

- `src/http.rs` — the limit, the error type, the helper, the `From` impl.
- The thirteen call sites, plus one match arm per outcome enum in
  `src/client/auth/mod.rs`. That file is not named in the frozen surface; the
  arm is an unavoidable consequence of the charter's criterion (4), since no
  existing outcome means "stop" without carrying a `reqwest::Error` the
  refusal has not got.
- `src/error.rs` — `ResponseTooLarge { operation, limit_bytes, status }`, exit
  code 16, type `response_too_large`, all three published as structured keys
  (`status` reuses the key `schemas/error.json` already declares). **Not** a
  transport failure.
- `schemas/error.json` (`exit_code` maximum 15 → 16 plus two optional keys),
  `SCHEMA_VERSION` 3.0.6 → 3.0.7 across its twelve live pins, and the two
  bundled readers that would otherwise reject exit 16 —
  `bzr-dependency-analysis`'s `collect.py` validator and `bzr-reference`'s
  error-type table.
- `docs/bzr-cli.md` — exit-code row, structured-key rows, attachment ceiling,
  version pin — and a second fixture case in
  `tests/functional/phases/18b-http-error-preview.sh`.

### Non-goals

Operator-approved 2026-09-07 and frozen in the issue's `WORK:SCOPE`: streaming
raw attachment downloads via `attachment.cgi` (owner: follow-up issue), so a
single attachment still arrives base64-encoded inside a REST JSON body and
inherits this bound; a user-facing limit override, deferred until a deployment
reports the default too low, with an environment escape hatch promotable only
on operator sign-off; bounding request or upload bodies, this change being
server-to-client only; and `Content-Length` pre-rejection as the bound, which
ADR 0068 declines as an addition too.

## Threat model

**Boundary.** One, widened by no part of this change and narrowed by all of it:
the HTTP response body crossing from a Bugzilla server into `bzr`'s address
space. The thirteen sites are its whole surface. No new entry point, parser, or
permission.

**Actors.** The untrusted party is the configured Bugzilla server and anyone
who can answer for it — a hostile host at a URL the operator typed or imported,
a MITM on a plaintext or mistrusted-TLS connection, or a compromised or
malfunctioning server the operator does trust. `bzr` trusts the operator's
config and the TLS pin policy, and nothing the server sends — which is why the
body is bounded rather than assumed well-behaved.

**Control.** One helper, called at every one of the thirteen sites, streaming
and refusing past `MAX_RESPONSE_BODY_BYTES` before any parse. There is no
single chokepoint to place it at — ADR 0068 records why — so a fourteenth site
could reintroduce an unbounded read; the control against that is a structural
test asserting no `Response::text()` call exists in `src/` outside
`*_tests.rs`, in the shape `tools/check-no-spawn.sh` already establishes for a
rule the type system cannot carry. On failure the error leaks the limit, the
operation, and the HTTP status — no body bytes and no URL, so no API key can
ride out in a query parameter. Existing controls are unchanged:
`redact_api_key`, `diagnostic_body_preview`, and the per-request timeout, which
bounds duration but not size.

**Out of scope.** Request and upload bodies (`bzr` produces them). Total memory
across concurrent requests — `bzr` issues one request at a time per command, so
the ceiling is one bounded read, at roughly twice the limit while the buffer
doubles. A pathological JSON document under the bound: `serde_json`'s recursion
limit answers that. A server that streams just under the bound repeatedly: the
request timeout covers it, a size bound cannot.

## Success

1. No response body is buffered past `MAX_RESPONSE_BODY_BYTES` at any of the
   thirteen sites, and no fourteenth unbounded read can be added without
   failing a test.
2. An over-limit body on a propagating path fails with
   `BzrError::ResponseTooLarge`, exit 16, naming the limit, the operation, and
   the HTTP status where one is known; `is_transport_failure` is `false` for it.
3. An over-limit body on an auth or version probe aborts detection with that
   error rather than continuing the probe chain or selecting a protocol.
4. `bzr schema error` publishes an `exit_code` maximum of 16, and both bundled
   readers accept an exit-16 envelope.
5. Bodies under the limit are returned byte-identically to `.text()` today.

## Validation

| Contract | Mode | Evidence |
|---|---|---|
| Streaming bound refuses past the limit | `focused-test` | `src/http_tests.rs`: `read_body_within` at limit-1, limit, and limit+1 with a 16-byte limit; the last returns `TooLarge` carrying the limit and operation |
| Transport failure stays distinguishable | `focused-test` | `src/http_tests.rs`: a truncated-response listener in the shape of `src/client/response_tests.rs::spawn_truncated_http_error_server` yields `BodyReadError::Transport` |
| The wrapper binds the real constant | `focused-test` | `src/http_tests.rs`: `read_body_bounded` returns a normal body; a second case pins `MAX_RESPONSE_BODY_BYTES == 64 * 1024 * 1024` |
| No fourteenth unbounded read | `focused-test` | `src/http_tests.rs`: a case walking `src/` that fails on any `.text()` outside `*_tests.rs` |
| Error contract of the new variant | `focused-test` | `src/error_tests.rs`: exit code 16, type `response_too_large`, the three structured keys, `is_transport_failure() == false` |
| Probe refusal aborts rather than continues | `focused-test` | `src/client/auth/whoami_tests.rs` and `valid_login_tests.rs`: a wiremock probe answering over a test limit yields `ProbeRefused`, not `AuthRejected` |
| Published schema admits exit 16 | `focused-test` | `src/main_tests.rs::format_dispatch_error_json_family_matches_published_schema`, extended with a `ResponseTooLarge` case |
| Bundled readers accept exit 16 | `focused-test` | `content/skills/bzr-dependency-analysis/tests/test_collect.py`: an exit-16 envelope with both new keys validates; plus phase 18b's `bzr schema error` assertion |
| End-to-end refusal on a real socket | `focused-test` | `tests/functional/phases/18b-http-error-preview.sh`: a local fixture streams past the limit; `bzr` exits 16 with `.error.type == "response_too_large"` |
| The three remaining degraded arms | `task-test-not-applicable` | Each returns the same value that site's existing unreadable-body arm returns — `AlternateAuth::Original`, `None`, or a substituted preview — so no observation distinguishes them beyond the `tracing` line, and asserting log wording tests prose. The two arms whose values differed were reclassified above rather than waived |
