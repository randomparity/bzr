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
  table, which asserts its own exhaustiveness. No gate scans that table, so its
  row is verified by hand against `src/error.rs` and `schemas/error.json`.
- **A REST-delivered attachment larger than about 48 MiB can no longer be
  downloaded; `bzr attachment download` exits 16 on it.** `download_attachment`
  reads base64 `data` out of a REST JSON body, and 4/3 expansion puts the
  ceiling at roughly three quarters of the limit. The refusal message says the
  limit is a fixed cap rather than a transient failure and names the attachment
  case, so an operator can tell it from a bug without reading source. Streaming
  that path via `attachment.cgi` is operator-approved as a follow-up on #740
  and is not filed: issue creation in this campaign needs an operator
  confirmation collected at its enqueue gate. A deployment setting Bugzilla's
  `maxlocalattachment` — the parameter whose purpose is to permit attachments
  above `maxattachmentsize` — can exceed the ceiling, and nothing raises it.
- **The Hybrid fallbacks gated on `is_transport_failure` lose one edge each.**
  A REST-first read no longer answers an over-limit body by re-fetching it over
  XML-RPC — the intent — and `dispatch_xmlrpc_first` (`src/client/mod.rs:273-289`)
  likewise stops falling back from an over-limit XML-RPC body to REST, where
  today it would. That second edge is accepted: re-fetching a body a server
  chose to oversize is the cost the refusal avoids. `server_extensions`
  (`src/client/resources/server.rs:59-88`) is the exception, and reads no
  classification at all: it falls back on a bare `Err`, so it still retries, and
  when both legs oversize its `map_err` reports `XmlRpc` — exit 4, with
  `operation` and `limit_bytes` surviving only inside the message. It is left
  alone because it sits outside this change's authorized surface, not because
  ADR-0052 settles it: that record decides *whether* the arm falls back, on the
  ground of a 404-with-error-envelope shape, and says nothing about how a
  combined failure is named. Making the refusal survive that `map_err` is a
  one-line guard and a good follow-up.
  That exception is documented in `docs/bzr-cli.md`'s exit-16 row.
- **Criterion (6)'s "real container" is met by a loopback fixture, by ruling.**
  No Bugzilla container emits a 96 MiB response on demand, so a literal reading
  of that criterion is unsatisfiable. Phase 18b runs the real `bzr` binary over
  a real socket through the same production path a user takes
  (`server info --api rest`, into `parse_json` and the shared bounded read),
  inside the container-based functional tier, against a local fixture — the
  shape that phase already uses for #512. The campaign orchestrator ratified
  the substitution explicitly on 2026-09-08, conditioned on the fixture
  exercising the bound through the production path rather than a helper in
  isolation, which it does. Recorded here so the substitution is auditable
  rather than looking like a gap.
- **Peak allocation is stated per path, not flat.** A refused read holds about
  1.5x the limit — roughly 96 MiB, the old 32 MiB buffer beside the new 64 MiB
  one — while the `Vec` doubles. An accepted body
  of valid UTF-8 holds the limit once, because `String::from_utf8` reuses the
  buffer. An accepted body that is *not* valid UTF-8 costs what
  `Response::text()` costs today: the failed `String` keeps the buffer alive
  while the lossy decode builds a replacement of up to three bytes per input
  byte, peaking near seven times the limit. That last case is not a regression
  and is still bounded — which is the point — but a flat "twice the limit" would
  be wrong. The probe abort stops at the first refusal rather than paying any of
  this again through the chain.
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

