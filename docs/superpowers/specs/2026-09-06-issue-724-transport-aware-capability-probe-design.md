# Issue #724 — the vendor-extension capability probe becomes transport-aware

Design record for issue #724.

**The decision, its grounds, and its rejected alternatives live in the 2026-09-06 amendment to
[ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md), and only
there.** This spec does not restate them — an earlier draft did, and both blocking findings of
the third review round were drift between the two copies. What follows is what the ADR does
not carry: the measurement record, the threat model, and why each test discriminates.

## Problem

ADR 0052 gates `bzr bug search --saved-search` on the server advertising the `RedHat`
extension, and establishes that fact through `GET /rest/extensions`
(`BugzillaClient::server_extensions`, `src/client/resources/server.rs`). The probe is
REST-only whatever `--api` says, so a deployment with REST disabled can never establish the
capability and the gate refuses the write. Whether that was worth fixing at all was open, and
the ADR amendment answers it.

## Measurement

Taken 2026-09-06 against the running functional containers — bz50, bz52, bz53 — and against
two throwaway proxies in front of bz50: one answering `503` on every `/rest/` path
(a REST-disabled deployment), one answering `/rest/extensions` with a Bugzilla error envelope
at `404` while forwarding the rest of REST (a front-end blocking just that endpoint).

| Probe | bz50 | bz52 | bz53 |
|---|---|---|---|
| `POST /xmlrpc.cgi` `Bugzilla.extensions`, unauthenticated | `<struct><member><name>extensions</name><value><struct /></value></member></struct>` | same | same |
| `GET /rest/extensions` | `{"extensions":{}}` | same | same |
| `GET /rest/<absent path>` | `404` + `{"error":true,"code":32614,"message":"A REST API resource was not found…"}` | same | same, `code` as a string |

With `bzr` built at `dcaf259f` (pre-change):

| Server | Command | Result |
|---|---|---|
| REST-disabled proxy | `--api xmlrpc bug search "test" --limit 1` | exit 0, one real bug |
| REST-disabled proxy | `--api xmlrpc bug search --saved-search nope` | exit 15, `undetermined` |
| envelope-404 proxy | `--api {rest,hybrid,xmlrpc} bug search --saved-search nope` | exit 15, `undetermined`, message `… (Bugzilla API error: …)` |
| bz50, `RUST_LOG=bzr=debug` | `--api xmlrpc bug search --saved-search nope` | `API response url="…/rest/extensions"` |

Three consequences, all load-bearing for the ADR and none restated there in this detail:

- The XML-RPC search path already carries the parameters (`src/xmlrpc/resources/bug.rs:29`
  and `:53-54`), and `src/xmlrpc/protocol/parsing.rs:66` already normalises the self-closing
  `<struct />` the containers send. The gate is the last REST dependency on that path.
- A 404 does **not** degrade to `absent`. It reaches `undetermined`, so fail-closed already
  held before this change.
- But it arrives as `BzrError::Api`, not `HttpStatus` — `error_from_status_body`
  (`src/client/response.rs`) classifies any REST error carrying a Bugzilla envelope that way,
  and `is_transport_failure()` (`src/error.rs`) does not match it. This is why the Hybrid arm
  falls back unconditionally; ADR decision point 1 carries the reasoning and the warning
  against narrowing it.

## Change

`BugzillaClient::server_extensions()` establishes the extension list over the transport in
use, per the ADR amendment's Decision. The plan carries the code.

Five statements describe a REST-only probe and become false. The last two are outside the
surface #724 was dispatched with; the campaign orchestrator granted the widening on
2026-09-06, because fixing the CLI reference while leaving `--help` false would ship the
phantom-doc defect this change itself creates, in the higher-traffic surface, having seen it.

- `src/commands/runtime/shared/capability.rs`, the *absent* message:
  `(not advertised at /rest/extensions)` → `(not advertised in the server's extension list)`.
- The same file, the *undetermined* message: the REST-surface sentence is removed and the
  remedy generalised. **In Hybrid the message must name both attempts**, not only detail the
  XML-RPC failure — the REST error is logged at `info`, which is invisible at the default
  `bzr=warn`, and a user on a REST-first connection reading an XML-RPC error would reasonably
  conclude bzr never tried REST.
- `docs/bzr-cli.md:538`, the `bug search --saved-search` note ("bzr checks `/rest/extensions`
  before searching"). The repository's designated CLI reference.
- `src/cli/bug/search.rs:32`, the `bug search` long-about — `--help` **and** the generated man
  page, so it reaches more users than the CLI reference does.
- `src/config/model.rs:41`, rustdoc on the persisted `server_extensions` field.

### The other consumer

`bzr server info` (`src/commands/server.rs:16`) reaches `server_extensions()` through
`server_info()`, and its behaviour changes — the plan's "no caller changes" means signatures,
not behaviour. The change is a strict improvement and cannot produce wrong data: on a server
whose `/rest/version` works but whose `/rest/extensions` does not, `server info` previously
failed outright and now returns the extension list over XML-RPC. Both transports report the
same server's own advertised list, so a mixed-transport result is the same fact by two routes.
Nothing changes where REST works, and nothing changes on a fully REST-disabled deployment,
where `server_version()` still fails first — which is the residual recorded under *Out of
scope*.

## Fail-closed

The property ADR 0052 exists to guarantee survives, and this design changes nothing that
could weaken it:

- A probe that fails is still `Err`, never an empty list, so "could not ask" cannot be read
  as "not supported" (`resolve_extensions`, unchanged).
- The three outcomes stay distinct, and `absent` versus `undetermined` remains the
  machine-readable distinction in `capability_status`.
- Every shape where the two transports could reach different verdicts from the same evidence
  is closed in the conservative direction — the four the ADR's decision point 3 enumerates.
- Widening the Hybrid fallback moves only *when* it fires. It cannot grant a capability,
  because the XML-RPC probe returns the server's own list, and both transports failing still
  yields `undetermined`.

## Threat model

The change adds one boundary: bzr now parses an XML-RPC response it did not produce, on a
path where previously it parsed JSON.

**Boundaries added.** The `Bugzilla.extensions` response body, under the control of whoever
operates the server bzr was pointed at.

**Boundaries widened.** None. The credential already reaches this server on this same
connection; no new entry point, permission, or grant.

**Actor model.** The untrusted party is the operator of the configured Bugzilla server — a
server the user chose and, where TLS pinning is configured, already pinned. A network
attacker is out of scope on the same terms as every other request bzr makes: TLS and the
pin/TOFU machinery in `src/tls/` govern it, not this code.

**Control per boundary.**

- *Parse.* `parse_response` (`src/xmlrpc/protocol/parsing.rs`) is the existing parser every
  other XML-RPC resource already trusts; this adapter adds no parsing of its own beyond struct
  member lookups. Unexpected shapes become `BzrError::XmlRpc`, which the gate maps to
  *undetermined* — the fail-closed direction.
- *Unbounded server text into the config.* Already controlled and unchanged: only names in
  `KNOWN_CAPABILITIES` are persisted (`capability.rs`), so an adversarial server advertising a
  megabyte of extension names writes nothing.
- *Server text into an error message.* A `faultString` becomes `BzrError::Api` verbatim via
  `fault_to_error` (`src/xmlrpc/protocol/fault.rs`) and reaches the *undetermined* message's
  `{error}`, stderr, and the JSON error body. **Neither transport bounds that string today**:
  `diagnostic_body_preview` applies only to the non-envelope `HttpStatus` path in
  `src/xmlrpc/protocol/client.rs`, and REST's own error envelopes build `BzrError::Api` from
  the response body with the same absence of bounding (`src/client/response.rs`). API-key
  redaction does hold, through `BzrError::Api`'s `Display` (`src/error.rs`). The two transports
  are therefore symmetric and this change introduces no new exposure; the shared unboundedness
  is pre-existing and outside #724.
- *Credential placement.* Under `ApiMode::XmlRpc` the client already carries the key in the
  request body for every call on that connection — `src/client/mod.rs` logs exactly this,
  overriding configured header auth for XML-RPC — so routing the probe there adds no placement
  the connection did not already have. Under `ApiMode::Rest` the probe is unchanged. The Hybrid
  fallback moves this one probe's credential from a header (the common case, not a query
  parameter) into a request body, and only after REST has already failed.

**Explicitly out of scope.** The `RedHat`-as-proxy false negative and false positive
(ADR 0052 consequences, and named as out of scope by issue #724); raw `--from-url` passthrough
remaining ungated (same); the unbounded error-message text above, pre-existing and symmetric;
and any change to what a credential is required for — the gate checks capability, not identity,
and still does.

## Tests

The honest limit, and what has since changed about it, is recorded in the ADR amendment's
Consequences: no supported *image* advertises the extension, so this phase covers the probe
mechanism and the negative verdict; ADR 0061's shaped proxy can now produce a positive verdict,
but it shapes `/rest/extensions` only and contains no XML-RPC handling at all, so reaching
this amendment's XML-RPC path is a design pass over that proxy rather than a call to an
existing helper. What follows is why each test here discriminates.

A capability probe exercised only against servers that all lack the capability is an
empty-versus-empty oracle by default: a test asserting "bzr correctly determines the extension
is absent" passes identically whether the probe works, silently fails, or is never issued.
Every test is required to discriminate, on one of two axes.

**Which transport was used.** Wiremock mounts both surfaces and asserts the request count on
each, so "the probe went to the other transport" and "no probe was issued" both fail. The
functional tier uses the `RUST_LOG=bzr=debug` trace, which is real wire evidence: the XML-RPC
arm must show the `Bugzilla.extensions` call and not `/rest/extensions`, and the REST arm the
reverse.

**Absent versus undetermined.** Both refuse with exit 15, so exit code alone proves nothing.
Asserting the *absent* wording is what proves the XML-RPC response was received and parsed; a
broken adapter flips the verdict to *undetermined* and the assertion fails.

| Tier | File | What it establishes |
|---|---|---|
| unit | `src/xmlrpc/resources/server_tests.rs` | `Bugzilla.extensions` is the method called; advertised extensions parse; `<struct />` parses as an empty map; and each malformed shape is an error rather than a map — missing `extensions` member, non-struct `extensions` member, non-struct extension value, non-string `version` |
| unit | `src/client/resources/server_tests.rs` | `XmlRpc` probes only `/xmlrpc.cgi`; `Rest` probes only `/rest/extensions`; `Hybrid` prefers REST; `Hybrid` falls back for **both** REST failure shapes — an enveloped 404 (`BzrError::Api`) and a bodyless 503 (`HttpStatus`); and both transports failing yields `undetermined` |
| unit | `src/commands/runtime/shared/capability_tests.rs` | the issue itself: REST failing while XML-RPC advertises `RedHat` establishes the capability under `ApiMode::XmlRpc` — the positive verdict, at the only tier that can express it; plus the absent and undetermined paths, and that the Hybrid *undetermined* message names both attempts |
| functional | `tests/functional/phases/08f-bug-saved-search.sh` | against a real container: the probe goes over XML-RPC under `--api xmlrpc` and over REST under `--api rest`, and the XML-RPC response really parsed |

**The two Hybrid failure shapes are kept even though no predicate now distinguishes them.**
They exercise different `BzrError` variants, so together they *prove* the unconditional
fallback rather than assume it, and they pin it against a future edit reintroducing a
predicate. Testing only the bodyless 503 is what let the original guard defect through, because
it passes under either rule.

The functional phase's line 31 asserts `${BZ_URL}/rest/extensions` under `--api xmlrpc`, and
its comment at lines 21–26 calls that "the REST-only `/rest/extensions` probe". Both pin the
current behaviour as intended; they change because the behaviour changed, not because they were
wrong.

Every new functional assertion goes through `test_pass` / `test_fail`. Each new test is proved
to bite by seeding a controlled fault, observing red, and reverting.

## Out of scope

`BugzillaClient::server_version()` (`src/client/resources/server.rs:35`) stays REST-only, so
`bzr server info` against a REST-disabled deployment still fails at the version step.
`Bugzilla.version` exists over XML-RPC and the fix would be near-identical, but the capability
gate never calls `server_version()`, so it is not a consequence of any #724 criterion. Reported
to the campaign orchestrator for its operator batch. The `docs/bzr-cli.md` note does not carry
this residual today: that note is held behind #718 and lands separately, and the sentence it
gains there is the one describing the probe, not this.
