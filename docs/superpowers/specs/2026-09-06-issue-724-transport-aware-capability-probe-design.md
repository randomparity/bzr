# Issue #724 — the vendor-extension capability probe becomes transport-aware

Design record for issue #724. Decision, with its grounds and rejected alternatives:
the 2026-09-06 amendment to
[ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md). This spec
records what changes and how it is proven; it does not re-argue the decision.

## Problem

ADR 0052 gates `bzr bug search --saved-search` on the server advertising the `RedHat`
extension, and establishes that fact through `GET /rest/extensions`
(`BugzillaClient::server_extensions`, `src/client/resources/server.rs`). The probe is
REST-only whatever `--api` says, so a deployment with REST disabled can never establish the
capability and the gate refuses the write.

The issue asks whether this is worth fixing at all, and treats that as open rather than
settled.

## Measurement

Taken 2026-09-06 against the running functional containers on this machine — bz50, bz52,
bz53 — and against a throwaway proxy that forwards `/xmlrpc.cgi` to bz50 and answers `503`
on every `/rest/` path, emulating a REST-disabled deployment.

| Probe | bz50 | bz52 | bz53 |
|---|---|---|---|
| `POST /xmlrpc.cgi` `Bugzilla.extensions`, unauthenticated | `<struct><member><name>extensions</name><value><struct /></value></member></struct>` | same | same |
| `GET /rest/extensions` | `{"extensions":{}}` | same | same |

Against the REST-disabled proxy, with `bzr` built at `dcaf259f`:

| Command | Result |
|---|---|
| `bzr --api xmlrpc bug search "test" --limit 1` | exit 0, one real bug returned |
| `bzr --api xmlrpc bug search --saved-search nope` | exit 15, `capability_status: "undetermined"` |

And against a REST-**enabled** container (bz50), with `RUST_LOG=bzr=debug`:

| Command | Observed request |
|---|---|
| `bzr --api xmlrpc bug search --saved-search nope` | `API response url="http://…/rest/extensions" status=200 OK` |

The three facts these establish, and why together they settle the issue's open question, are
the ADR amendment's Context section. Two of them bear directly on the change below and are
repeated only as pointers: `src/xmlrpc/resources/bug.rs:29` and `:54` already insert
`savedsearch` and `sharer_id` into `Bug.search`, and `src/xmlrpc/protocol/parsing.rs:63`
already normalises the self-closing `<struct />` the containers send.

## Change

`BugzillaClient::server_extensions()` establishes the extension list over the transport in
use instead of always over REST: `ApiMode::Rest` probes REST, `ApiMode::XmlRpc` calls
`Bugzilla.extensions`, and `ApiMode::Hybrid` probes REST first and falls back to XML-RPC on a
transport failure. The ADR amendment carries the grounds for each arm, including why Hybrid
does not use the existing `dispatch_xmlrpc_first` helper and what its fallback does and does
not buy. The plan carries the code.

`is_transport_failure()` (`src/error.rs:233`) covers `Http`, `HttpStatus` and `XmlRpc`, so a
REST surface that is absent (404), refusing (503) or unreachable falls back, while a
`Deserialize` failure on a REST body that did arrive does not.

### The XML-RPC adapter

New `src/xmlrpc/resources/server.rs`, mirroring `src/xmlrpc/resources/group.rs`:
`XmlRpcClient::server_extensions() -> Result<ServerExtensions>` calls `Bugzilla.extensions`
with no parameters and maps `{extensions: {Name: {version: String}}}` onto the existing
`ServerExtensions` / `ExtensionInfo` types (`src/types/server_info.rs`), which are already
what the REST path deserialises into. Registered with one `mod server;` line in
`src/xmlrpc/resources/mod.rs`.

**A malformed response is an error, not an empty or partial map**, in the two shapes where
the adapter would otherwise be more permissive than serde:

- a **missing `extensions` member**, where an empty map would render as *absent* — a settled
  "your server does not support this" — while the REST path's decode of a body with no
  `extensions` key fails and renders *undetermined*;
- an **extension whose value is not a struct** (`{"RedHat": 5}`), where absorbing the shape
  error into `version: None` would keep `RedHat` in the map and render as *advertised*, while
  serde fails the whole `HashMap<String, ExtensionInfo>` decode and again renders
  *undetermined*.

Both return `BzrError::XmlRpc`. An `extensions` member that is present and empty is a
legitimate empty map, which is what all three containers actually send.

### Messages and documentation

Three places currently describe a REST-only probe and become false.

- `src/commands/runtime/shared/capability.rs`, the *absent* message:
  `(not advertised at /rest/extensions)` → `(not advertised in the server's extension list)`.
- The same file, the *undetermined* message:
  `reading /rest/extensions failed ({error}). The probe needs the server's REST surface even
  when --api xmlrpc is in use.` → `reading the server's extension list failed ({error}).`,
  with the REST sentence removed and the closing remedy changed from `check that REST is
  reachable` to `check that the server is reachable over the API mode in use`. The embedded
  `{error}` already names the transport that failed.
- `docs/bzr-cli.md:538`, the `bug search --saved-search` note, which tells users "bzr checks
  `/rest/extensions` before searching". That is the repository's designated CLI reference, and
  a doc that fails when followed is a defect fixed in the change that exposes it. It becomes
  "bzr checks the server's advertised extension list over the API mode in use before
  searching", plus one clause recording that `bzr server info`'s version step still needs
  REST.

Everything else about the gate is unchanged: the same three outcomes, the same
`UnsupportedServerCapability` variant, the same exit code 15, the same
`capability_status` values. The persisted cache is unchanged too — see the ADR amendment's
first decision point for why it gains no transport dimension.

## Fail-closed

The property ADR 0052 exists to guarantee survives, and this design changes nothing that
could weaken it:

- A probe that fails is still `Err`, never an empty list, so "could not ask" cannot be read
  as "not supported" (`resolve_extensions`, unchanged).
- The three outcomes stay distinct, and `absent` versus `undetermined` remains the
  machine-readable distinction in `capability_status`.
- The new adapter is the only new surface on which the two transports could reach different
  verdicts from the same evidence, and both shapes where that was possible — the missing
  member and the non-struct value — are closed above, in the conservative direction.
- No path is added on which an undetermined capability dispatches the parameter.

## Threat model

The change adds one boundary: bzr now parses an XML-RPC response it did not produce, on a
path where previously it parsed JSON.

**Boundaries added.** The `Bugzilla.extensions` response body, under the control of whoever
operates the server bzr was pointed at.

**Boundaries widened.** None. The credential already reaches this server on this same
connection; no new entry point, permission, or grant.

**Actor model.** The untrusted party is the operator of the configured Bugzilla server — a
server the user chose and, where TLS pinning is configured, already pinned. A network
attacker is out of scope for this change on the same terms as every other request bzr makes:
TLS and the pin/TOFU machinery in `src/tls/` govern it, not this code.

**Control per boundary.**

- *Parse.* `parse_response` (`src/xmlrpc/protocol/parsing.rs`) is the existing parser every
  other XML-RPC resource already trusts; this adapter adds no parsing of its own beyond
  struct member lookups. Unexpected shapes become `BzrError::XmlRpc`, which the gate maps to
  *undetermined* — the fail-closed direction.
- *Unbounded server text into the config.* Already controlled and unchanged: only names in
  `KNOWN_CAPABILITIES` are persisted (`capability.rs`), so an adversarial server advertising
  a megabyte of extension names writes nothing.
- *Server text into an error message.* A `faultString` becomes `BzrError::Api` verbatim via
  `fault_to_error` (`src/xmlrpc/protocol/fault.rs`) and reaches the *undetermined* message's
  `{error}`, stderr, and the JSON error body. **Neither transport bounds that string today**:
  `diagnostic_body_preview` applies only to the non-envelope `HttpStatus` path in
  `src/xmlrpc/protocol/client.rs`, and REST's own error envelopes build `BzrError::Api` from
  the response body with the same absence of bounding (`src/client/response.rs`). API-key
  redaction does hold, through `BzrError::Api`'s `Display` (`src/error.rs`). So the two
  transports are symmetric here and this change introduces no new exposure; the shared
  unboundedness is pre-existing and outside #724.
- *Credential placement.* `XmlRpcClient::call` carries the API key in the POST body rather
  than a URL query parameter. That is the existing, documented XML-RPC behaviour for every
  other call and is no worse than the REST query-param path it replaces here.

**Explicitly out of scope.** The `RedHat`-as-proxy false negative and false positive
(ADR 0052 consequences, and named as out of scope by issue #724); raw `--from-url`
passthrough remaining ungated (same); the unbounded error-message text above, which is
pre-existing and symmetric across transports; and any change to what a credential is required
for — the gate checks capability, not identity, and still does.

## Tests

The honest limit is recorded in the ADR amendment's Consequences: **no supported container
can produce a positive capability verdict**, so "cover the XML-RPC path against a real
container" is satisfiable for the probe *mechanism* and the *negative* verdict, and not for a
positive one. Coverage splits across two tiers accordingly, and neither tier is reported as
the other.

A capability probe exercised only against servers that all lack the capability is an
empty-versus-empty oracle by default: a test asserting "bzr correctly determines the
extension is absent" passes identically whether the probe works, silently fails, or is never
issued. Every test below is required to discriminate, on one of two axes.

**Which transport was used.** Wiremock mounts both surfaces and asserts the request count on
each, so "the probe went to the other transport" and "no probe was issued" both fail. The
functional tier uses the `RUST_LOG=bzr=debug` trace, which is real wire evidence: the
XML-RPC arm must show the `Bugzilla.extensions` call and must not show `/rest/extensions`,
and the REST arm must show `/rest/extensions` and not the XML-RPC call.

**Absent versus undetermined.** Both refuse with exit 15, so exit code alone proves nothing.
Asserting the *absent* wording is what proves the XML-RPC response was received and parsed;
a broken adapter flips the verdict to *undetermined* and the assertion fails.

| Tier | File | What it establishes |
|---|---|---|
| unit | `src/xmlrpc/resources/server_tests.rs` | advertised extensions parse; `<struct />` parses as an empty map; a missing `extensions` member, a non-struct `extensions` member, and a non-struct extension value are each an error rather than a map |
| unit | `src/client/resources/server_tests.rs` | `XmlRpc` probes only `/xmlrpc.cgi`; `Rest` probes only `/rest/extensions`; `Hybrid` prefers REST; `Hybrid` falls back to XML-RPC when REST answers 503 |
| unit | `src/commands/runtime/shared/capability_tests.rs` | the issue itself: REST answering 503 while XML-RPC advertises `RedHat` establishes the capability under `ApiMode::XmlRpc` — the positive verdict, at the only tier that can express it; plus the absent and undetermined paths over XML-RPC |
| functional | `tests/functional/phases/08f-bug-saved-search.sh` | against a real container: the probe goes over XML-RPC under `--api xmlrpc` and over REST under `--api rest`, and the XML-RPC response really parsed |

The functional phase's line 31 asserts `${BZ_URL}/rest/extensions` under `--api xmlrpc`, and
its comment at lines 21–26 calls that "the REST-only `/rest/extensions` probe" — both pin the
current behaviour as intended. They change because the behaviour changed, not because they
were wrong.

Every new functional assertion goes through `test_pass` / `test_fail`, because a bare `[[ ]]`
conjunct with no `else` moves no suite counter and renders no result. Each new test is proved
to bite by seeding a controlled fault, observing red, and reverting.

## Out of scope

`BugzillaClient::server_version()` (`src/client/resources/server.rs:39`) stays REST-only, so
`bzr server info` against a REST-disabled deployment still fails at the version step.
`Bugzilla.version` exists over XML-RPC and the fix would be near-identical, but the
capability gate never calls `server_version()`, so it is not a consequence of any #724
criterion. Reported to the campaign orchestrator rather than fixed here, and recorded in the
`docs/bzr-cli.md` note so an affected operator is not left guessing.
