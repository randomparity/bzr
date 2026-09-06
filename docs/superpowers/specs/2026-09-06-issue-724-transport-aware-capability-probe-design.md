# Issue #724 — the vendor-extension capability probe becomes transport-aware

Design record for issue #724. Decision: the 2026-09-06 amendment to
[ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md).

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

Three facts follow, and together they settle the issue's open question.

1. **The XML-RPC search path already carries the parameter.**
   `src/xmlrpc/resources/bug.rs:29` inserts `savedsearch` into `Bug.search` and `:54`
   inserts `sharer_id`. Plain XML-RPC search works end-to-end against a server whose REST
   surface is gone. The gate is the only thing standing between an XML-RPC-only deployment
   and a working `--saved-search`; this is an incomplete implementation rather than an
   absent feature.
2. **`Bugzilla.extensions` is reachable over XML-RPC on every supported image**, needs no
   credential, and returns the same shape as REST. `src/xmlrpc/protocol/parsing.rs:63`
   already normalises the self-closing `<struct />` the containers actually send.
3. **The defect is not confined to REST-disabled deployments.** On a fully REST-enabled
   server, `--api xmlrpc` still issues `GET /rest/extensions`. `--api` is a documented
   contract and bzr violates it on every server. The issue's framing — that REST-disabled
   deployments may be too rare to justify the path — is the wrong axis to decide on, because
   the contract violation is universal.

## Change

`BugzillaClient::server_extensions()` establishes the extension list over the transport in
use instead of always over REST.

### Dispatch

```rust
// src/client/resources/server.rs
pub async fn server_extensions(&self) -> Result<ServerExtensions> {
    match self.api_mode {
        ApiMode::Rest => self.get_json("extensions").await,
        ApiMode::XmlRpc => self.xmlrpc_client().server_extensions().await,
        // REST first, matching `search_bugs_hybrid` — the transport actually in
        // use for the command this gate guards. XML-RPC only when REST could not
        // be reached at all, so a REST body bzr failed to parse still surfaces as
        // undetermined rather than being silently re-asked elsewhere.
        ApiMode::Hybrid => match self.get_json("extensions").await {
            Err(e) if e.is_transport_failure() => {
                self.xmlrpc_client().server_extensions().await
            }
            other => other,
        },
    }
}
```

Hybrid deliberately does **not** use the existing `dispatch_xmlrpc_first` helper. Hybrid is
the default for Bugzilla 5.0 and 5.2 — most users — and flipping the extensions probe to
XML-RPC-first there is a behavioural change nothing in the issue asks for. It is also less
faithful to the issue's criterion: `bug search` in Hybrid is REST-first
(`search_bugs_hybrid`, `src/client/resources/bug.rs:270`), so REST-first is what "the
transport in use" means for the command this gate guards. Three lines inline are cheaper
than a generic REST-first helper that would have exactly one caller.

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

**A missing `extensions` member is an error, not an empty map.** This is the one place the
adapter could quietly break fail-closed: an empty map renders as *absent* — a settled "your
server does not support this" — whereas the REST path's serde decode of a body with no
`extensions` key fails and renders as *undetermined*. The two transports must agree, so the
adapter returns `BzrError::XmlRpc` when the member is missing or is not a struct. An
`extensions` member that is present and empty is a legitimate empty map, which is what all
three containers actually send.

### Messages

Both refusal messages in `src/commands/runtime/shared/capability.rs` currently name REST and
become false:

- *absent* says `(not advertised at /rest/extensions)` → becomes
  `(not advertised in the server's extension list)`.
- *undetermined* says `reading /rest/extensions failed ({error}). The probe needs the
  server's REST surface even when --api xmlrpc is in use.` → becomes
  `reading the server's extension list failed ({error}).` with the REST sentence removed.
  The embedded `{error}` already names the transport that failed, so nothing actionable is
  lost.

Everything else about the gate is unchanged: the same three outcomes, the same
`UnsupportedServerCapability` variant, the same exit code 15, the same
`capability_status` values.

### The cache gains nothing

The persisted answer stays keyed on URL and capability allowlist exactly as ADR 0052
specifies. The advertised extension list is a fact about the server, not about the
transport, and both transports return the same list — so a transport dimension would add a
cache key that can only cause redundant probes. An answer probed over XML-RPC and later read
under `--api rest` is the same fact.

## Fail-closed

The property ADR 0052 exists to guarantee survives by construction, and this design changes
nothing that could weaken it:

- A probe that fails is still `Err`, never an empty list, so "could not ask" cannot be read
  as "not supported" (`resolve_extensions`, unchanged).
- The three outcomes stay distinct, and `absent` versus `undetermined` remains the
  machine-readable distinction in `capability_status`.
- The new adapter's missing-member rule above is the only new way fail-closed could have
  been lost, and it is closed explicitly.
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
- *Server text into an error message.* An XML-RPC fault string reaches the *undetermined*
  message the same way a REST body preview does today. The existing controls apply
  unchanged: `crate::http::diagnostic_body_preview` bounds the body and
  `crate::bugzilla_auth::redact_api_key` scrubs diagnostics
  (`src/xmlrpc/protocol/client.rs`).
- *Credential placement.* `XmlRpcClient::call` carries the API key in the POST body rather
  than a URL query parameter. That is the existing, documented XML-RPC behaviour for every
  other call and is no worse than the REST query-param path it replaces here.

**Explicitly out of scope.** The `RedHat`-as-proxy false negative and false positive
(ADR 0052 consequences, and named as out of scope by issue #724); raw `--from-url`
passthrough remaining ungated (same); and any change to what a credential is required for —
the gate checks capability, not identity, and still does.

## Tests

The honest limit first: **no supported container can produce a positive capability verdict.**
The Red Hat extension is absent from and inert on bz50, bz52 and bz53 — established by
quest-710 and re-confirmed by the measurement above. So "cover the XML-RPC path against a
real container" is satisfiable for the probe *mechanism* and the *negative* verdict, and not
for a positive one. The coverage splits across two tiers accordingly, and neither tier is
reported as the other.

A capability probe exercised only against servers that all lack the capability is an
empty-versus-empty oracle by default: a test asserting "bzr correctly determines the
extension is absent" passes identically whether the probe works, silently fails, or is never
issued. Every test below is required to discriminate, on one of two axes.

**Which transport was used.** Wiremock mounts both surfaces and asserts the request count on
each, so "the probe went to the other transport" and "no probe was issued" both fail. The
functional tier uses the `RUST_LOG=bzr=debug` trace, which is real wire evidence: the
XML-RPC arm must show the `Bugzilla.extensions` call and must not show `/rest/extensions`,
and the REST arm on a cache miss must show `/rest/extensions`. The two arms differ on data
the containers actually produced.

**Absent versus undetermined.** Both refuse with exit 15, so exit code alone proves nothing.
Asserting the *absent* wording is what proves the XML-RPC response was received and parsed;
a broken adapter flips the verdict to *undetermined* and the assertion fails.

| Tier | File | What it establishes |
|---|---|---|
| unit | `src/xmlrpc/resources/server_tests.rs` | advertised extensions parse; `<struct />` parses as an empty map; a missing or non-struct `extensions` member is an error, not an empty map |
| unit | `src/client/resources/server_tests.rs` | `XmlRpc` probes only `/xmlrpc.cgi`; `Rest` probes only `/rest/extensions`; `Hybrid` prefers REST; `Hybrid` falls back to XML-RPC when REST answers 503 |
| unit | `src/commands/runtime/shared/capability_tests.rs` | the issue itself: REST answering 503 while XML-RPC advertises `RedHat` establishes the capability under `ApiMode::XmlRpc` — the positive verdict, at the only tier that can express it; plus the absent and undetermined paths over XML-RPC |
| functional | `tests/functional/phases/08f-bug-saved-search.sh` | against a real container: the probe goes over XML-RPC under `--api xmlrpc` and over REST under `--api rest`, and the XML-RPC response really parsed |

The functional phase's existing line 31 asserts `${BZ_URL}/rest/extensions` under
`--api xmlrpc` — it pins the current REST-only behaviour as intended. That assertion
inverts; it is not being deleted because it was wrong.

Every new functional assertion goes through `test_pass` / `test_fail`, because a bare `[[ ]]`
conjunct with no `else` moves no suite counter and renders no result. Each new test is proved
to bite by seeding a controlled fault (an adapter that returns an empty map for a missing
`extensions` member; a dispatch arm that stays on REST), observing red, and reverting.

## Out of scope

`BugzillaClient::server_version()` (`src/client/resources/server.rs:39`) stays REST-only, so
`bzr server info` against a REST-disabled deployment still fails at the version step.
`Bugzilla.version` exists over XML-RPC and the fix would be near-identical, but the
capability gate never calls `server_version()`, so it is not a consequence of any #724
criterion. Reported to the campaign orchestrator rather than fixed here.
