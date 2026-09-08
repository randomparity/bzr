# 0068 — Response bodies are read under one shared bound

- Status: Accepted
- Date: 2026-09-07
- Issue: #740
- Related: [0007](0007-json-output-schema-version-envelope.md),
  [0057](0057-alternate-auth-retry-classifies-the-refusal.md)

## Context

Thirteen call sites across six files read a Bugzilla response body with
`reqwest::Response::text()` and no ceiling — the parse and HTTP-error seams in
`src/client/response.rs` and `src/xmlrpc/protocol/client.rs`, the three auth
probes, the 401 alternate-auth retry, and the version probe. Together they are
the whole client-side trust boundary for server-controlled data.

They do not share a function. The client's send pipeline covers only some of
them; the auth and version probes build their own requests against a bare
`reqwest::Client`, and `reqwest` offers no per-client response-size limit. So
the question is what shared thing they can be made to share — and, once the
bound exists, what `bzr` reports when it fires and where the bound sits.

## Decision

**One helper in `src/http.rs` streams every response body under one constant,
and an over-limit body is refused as a new `BzrError::ResponseTooLarge` with
exit code 16.**

- `read_body_bounded(response, operation)` accumulates `Response::chunk()` into
  a `Vec<u8>`, refusing as soon as the total would exceed
  `MAX_RESPONSE_BODY_BYTES = 64 * 1024 * 1024`, then decodes with
  `String::from_utf8_lossy`. All thirteen sites call it. A private
  `read_body_within(response, operation, limit_bytes)` carries the limit so the
  bound is provable without a 64 MiB fixture.
- It returns `BodyReadError::{TooLarge, Transport}`, not `BzrError`, so each
  site can keep the disposition it already has for an unreadable body. The
  seven propagating sites convert with `From`; the six probe sites take their
  existing degraded path — `AlternateAuth::Original`, `ApiMode::XmlRpc`,
  `NetworkError`, `None`, or a substituted preview — under a distinct warning.
- `ResponseTooLarge` carries `operation` and `limit_bytes`, publishes them as
  structured keys, and is **not** a transport failure, so Hybrid mode does not
  answer it by re-fetching the same body over XML-RPC.

## Consequences

- **Exit code 16 and `schemas/error.json`'s `exit_code` maximum of 16 are new
  public contract.** `SCHEMA_VERSION` goes 3.0.6 → 3.0.7, a patch under
  ADR 0007's additive rule, across its twelve live pins.
- **A single attachment larger than ~48 MiB can no longer be downloaded.**
  `download_attachment` reads base64 `data` out of a REST JSON body, and 4/3
  expansion puts the ceiling there. Streaming that path via `attachment.cgi` is
  the operator-approved follow-up recorded on #740.
- **A hostile server can still cost the operator 64 MiB and one timeout.** The
  bound stops unbounded growth, not the cost of one bounded read.
- **No knob.** No flag, config key, or environment variable overrides the
  limit; the first deployment that reports 64 MiB too low is what reopens it.

## Considered & rejected

- **Bound inside the client's send pipeline.** verified: six of the thirteen
  sites never enter it — `probe_whoami`, `probe_valid_login`, `read_probe_leg`,
  `detect_version`, `retry_with_alternate_auth`, and the XML-RPC error seam hold
  a `reqwest::Response` from a `reqwest::Client` they built themselves (bzr at
  46be02cc). It would leave the auth and version probes — the first traffic
  `bzr` sends a stranger — unbounded.
- **Configure the limit on the `reqwest::Client`.** verified: reqwest 0.12.28
  exposes no response-body size limit on `ClientBuilder`, and its own
  `Response::text()` is `bytes()` plus `String::from_utf8_lossy` with no
  ceiling (`src/async_impl/response.rs:163-175`).
- **Reject on `Content-Length` before reading.** judgment: the attacker writes
  that header, so it can only be a fast path in front of the real check, and a
  branch indistinguishable from the check behind it is one no test can hold
  honest. Excluded as the sole mechanism by the operator-approved exclusion set
  on #740; declined as an addition too.
- **Reuse `BzrError::HttpStatus` or `BzrError::DataIntegrity`.** verified:
  `HttpStatus` is a transport failure under `is_transport_failure`
  (`src/error.rs:233-238`), so Hybrid mode would answer an over-limit REST body
  by re-fetching it over XML-RPC — the one response to a size refusal that
  repeats the cost. judgment: `DataIntegrity` claims the data was malformed,
  which an over-limit body need not be.
- **Thirteen per-site guards.** judgment: that is the shape the problem already
  has, and it accumulated because each site looks harmless alone.
- **A smaller limit, say 8 MiB.** verified: Bugzilla's documented
  `maxattachmentsize` default is 1000 KB and Mozilla's installation runs 10240
  KB (<https://bugzilla.readthedocs.io/en/latest/administering/parameters.html>),
  both of which 8 MiB clears — but base64 expansion makes 8 MiB refuse an
  ordinary 6 MiB attachment. judgment: 64 MiB still stops unbounded growth on
  any machine that can run `cargo build`.
- **An environment-variable escape hatch.** judgment: exclusion (b) on #740
  permits one only if no single default suffices, and 64 MiB clears the largest
  legitimate payload by six times the most generous real deployment found.
- **Do nothing.** judgment: every response `bzr` has ever read has been
  unbounded, and these sites straddled two in-flight branches long enough that
  nobody fixed one.
