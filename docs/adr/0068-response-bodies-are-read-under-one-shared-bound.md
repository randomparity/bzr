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
the question is what shared thing they can be made to share — and, once a bound
exists, what `bzr` reports when it fires and where it sits.

## Decision

**One helper in `src/http.rs` streams every response body under one constant,
and an over-limit body is refused as a new `BzrError::ResponseTooLarge` with
exit code 16.**

- `read_body_bounded(response, operation)` accumulates `Response::chunk()` into
  a `Vec<u8>`, refusing as soon as the total would exceed
  `MAX_RESPONSE_BODY_BYTES = 64 * 1024 * 1024`, then decodes without a second
  copy on the valid-UTF-8 path. A crate-internal
  `read_body_within(response, operation, limit_bytes)` carries the limit so the
  bound is provable without a 64 MiB fixture.
- It returns `BodyReadError::{TooLarge, Transport}`, not `BzrError`, so each
  site keeps the disposition it already has for an unreadable body. Ten sites
  propagate the refusal; three degrade — the 401 alternate-auth retry, whose
  original 401 still surfaces; `read_probe_leg`, whose leg was already
  inconclusive; and the XML-RPC HTTP-error preview, which substitutes a marker.
- **An over-limit body on an auth or version probe aborts, it does not fall
  back.** Routing `probe_whoami` and `probe_valid_login` to the existing
  `AuthRejected` would let a server choose a *response size* and thereby walk
  `bzr` through up to six further probes, three of which carry the API key in a
  URL query parameter — a credential exposure induced by no auth signal at all.
  They return a new `ProbeRefused` outcome instead. The version probe
  propagates for the same reason its TLS-certificate failure already does:
  otherwise a server picks the wire protocol by choosing a body size.
- `ResponseTooLarge` carries `operation`, `limit_bytes`, and the HTTP `status`
  when one is known, publishes them as structured keys, and is **not** a
  transport failure.

## Consequences

- **Exit code 16 and `schemas/error.json`'s `exit_code` maximum of 16 are new
  public contract.** `SCHEMA_VERSION` goes 3.0.6 → 3.0.7, a patch under
  ADR 0007's additive rule, across its twelve live pins. Two bundled readers
  must move with it: `bzr-dependency-analysis`'s envelope validator, whose
  ceiling and key set are already stale at exit 15, and `bzr-reference`'s error
  table, which asserts its own exhaustiveness. The second is outside this
  change's authorized surface and is deferred to the caller.
- **A single attachment larger than ~48 MiB can no longer be downloaded.**
  `download_attachment` reads base64 `data` out of a REST JSON body, and 4/3
  expansion puts the ceiling there. Streaming that path via `attachment.cgi` is
  operator-approved as a follow-up on #740 and had not been filed when this
  record was written; it needs an issue. A deployment setting Bugzilla's
  `maxlocalattachment` — the parameter whose purpose is to permit attachments
  above `maxattachmentsize` — can exceed the ceiling, and nothing raises it.
- **Both Hybrid fallbacks lose one edge.** A refusal is not a transport
  failure, so a REST-first read no longer answers an over-limit body by
  re-fetching it over XML-RPC — the intent — and `dispatch_xmlrpc_first`
  (`src/client/mod.rs:273-289`) likewise stops falling back from an over-limit
  XML-RPC body to REST, where today it would. That second edge is accepted:
  re-fetching a body a server chose to oversize is the cost the refusal avoids.
- **Cost per refused read is about twice the limit** — the `Vec` doubles as it
  grows, so a body refused near the limit holds roughly 96 MiB across the old
  and new buffers. The probe abort stops at the first refusal rather than
  paying that again through the chain.
- **No knob** — no flag, config key, or environment variable overrides the
  limit. The first deployment that reports 64 MiB too low reopens it.

## Considered & rejected

- **Bound inside the client's send pipeline, or on the `reqwest::Client`.**
  verified: six of the thirteen sites never enter the pipeline —
  `probe_whoami`, `probe_valid_login`, `read_probe_leg`, `detect_version`,
  `retry_with_alternate_auth`, and the XML-RPC error seam hold a
  `reqwest::Response` from a `reqwest::Client` they built themselves (bzr at
  46be02cc) — so it would leave the auth and version probes, the first traffic
  `bzr` sends a stranger, unbounded. verified: reqwest 0.12.28 exposes no
  response-body size limit on `ClientBuilder` either, and its own
  `Response::text()` is `bytes()` plus `String::from_utf8_lossy` with no
  ceiling (`src/async_impl/response.rs:163-175`).
- **Reject on `Content-Length` before reading.** judgment: the attacker writes
  that header, so it can only be a fast path in front of the real check, and a
  branch indistinguishable from the check behind it is one no test can hold
  honest. Excluded as the sole mechanism by the operator-approved exclusion set
  on #740; declined as an addition too.
- **Reuse `BzrError::HttpStatus` or `BzrError::DataIntegrity`.** verified:
  `HttpStatus` is a transport failure under `is_transport_failure`
  (`src/error.rs:233-238`), so every REST-first read — `bug.rs:398`,
  `user.rs:90`, `group.rs:90` — would answer an over-limit body by re-fetching
  it over XML-RPC, the one response to a size refusal that repeats the cost.
  judgment: `DataIntegrity` claims the data was malformed, which an over-limit
  body need not be.
- **Thirteen per-site guards.** judgment: the shape the problem already has,
  which accumulated because each site looks harmless alone.
- **8 or 16 MiB instead.** verified: Bugzilla's documented `maxattachmentsize`
  default is 1000 KB, so the largest attachment a stock server accepts is a
  ~1.3 MiB body after base64
  (<https://bugzilla.readthedocs.io/en/latest/administering/parameters.html>);
  8 MiB and 16 MiB both clear it. judgment: no measurement here establishes
  what a real deployment needs, because `bzr` never surveyed one — 64 MiB is
  headroom over an unmeasured population, chosen because the bound's job is to
  stop *unbounded* growth and 64 MiB still does that on any machine that can
  run `cargo build`. A deployment reporting it wrong in either direction moves
  it; the same answers an environment-variable escape hatch, which exclusion
  (b) on #740 permits only once a single default demonstrably fails.
- **Do nothing.** judgment: every response `bzr` has ever read has been
  unbounded, and these sites straddled two in-flight branches long enough that
  nobody fixed even one.

