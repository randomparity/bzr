# ADR 0052: Detect vendor-extension support before dispatch

## Status

Accepted

Amended 2026-09-06 (issue #724): the probe now follows the transport in use rather than
always issuing a REST request. The original decision below stands unchanged; see
[Amendment (2026-09-06)](#amendment-2026-09-06-the-probe-follows-the-transport-in-use).

## Context

python-bugzilla exposes several Bugzilla search and mutation parameters that are Red Hat
extensions rather than upstream Bugzilla API. The parity campaign tracked in
`docs/dev/python-bugzilla-parity.md` asks bzr to match that surface across five issues: #670
server-side saved searches (`savedsearch`, `sharer_id`), #671 generic arbitrary fields, #672
comment tags and minor update, #679 whiteboard match types, and #680 personal bug tags. They
are not all the same kind of gap — #671 and #672 turned out to be core Bugzilla rather than
vendor extensions, which is what the scope boundary at the end of this record exists to
capture.

Issue #670 established what "unsupported" means for the vendor-extension kind.
Neither `savedsearch` nor `sharer_id` appears anywhere under `Bugzilla/` in the project's
functional images (5.0.6, 5.2, 5.3.3+); `Bugzilla::WebService::Bug::search` passes its
parameter hash straight into `Bugzilla::Search->new`, which ignores keys it does not know.
Live probes confirm the behaviour: `GET /rest/bug?savedsearch=<name>` returned results
byte-identical to an unfiltered search on all three images, and an XML-RPC `Bug.search`
carrying the same member — authenticated as the owner of a named query seeded to match
nothing — returned the same. Upstream Bugzilla does not reject these parameters. It accepts
them and silently discards them.

So a client that simply forwards them produces, on every server the suite tests against, a
result that looks like a successful filtered search and is actually an unfiltered one. The
user asked for a subset and received the whole set, with a zero exit code and nothing in the
response to distinguish the two. python-bugzilla behaves exactly this way today.

Every one of the five issues meets this same question, so answering it once is the point of
this record.

## Decision

**bzr establishes that the server supports a vendor extension before dispatching a request
whose vendor parameter bzr itself constructs, and fails with an actionable error when the
server does not advertise it.**

That qualifier is load-bearing. bzr also forwards unrecognised URL parameters verbatim
(`bzr bug search --from-url`, and the saved queries `--save-as` persists for `bzr query run`),
and those paths are not gated — see Consequences.

Support is determined from `GET /rest/extensions`, which bzr already consumes through
`BugzillaClient::server_extensions()` into `ServerExtensions`. Red Hat Bugzilla advertises a
`RedHat` extension there; stock Bugzilla advertises none.

Three consequences of that rule are part of the decision, not implementation detail:

1. **The detected capability is cached in the per-server configuration**, beside the
   auth-method, API-mode and server-version detection state that `ServerConfig` already
   persists. A capability is the same kind of fact as those, and re-probing on every
   invocation would add a round trip to commands that need none.

2. **A failed probe is not an absent extension.** An unreachable or erroring
   `/rest/extensions` yields a distinct error saying support could not be determined, naming
   the transport failure. It never renders as "your server does not support this". Both paths
   refuse to dispatch, so neither produces a silently wrong result, but they do not claim to
   know the same thing. This follows ADR 0015: bzr does not mask what the server actually did.

3. **The error names the operation, the server, the capability, and the remedy**, per the
   repository's fail-fast rule. It carries a machine-readable `capability` and
   `capability_status` (`absent` or `undetermined`) in its structured detail, so a consumer can
   tell a settled refusal from a retryable one without parsing the message.

## Consequences

- A user on a stock Bugzilla asking for a saved search gets a refusal that explains itself,
  instead of an unfiltered result they may not notice is unfiltered. This is the whole point:
  a wrong answer that looks right is worse than a refusal.
- bzr and python-bugzilla now differ deliberately on these parameters. The parity report
  records that difference as bzr's behaviour rather than as a gap, because returning
  unfiltered results for a filtered query is not a capability bzr is missing.
- Detection keys on the `RedHat` extension being advertised, which is a **proxy** for the
  server having patched `Bug.search`, not proof of it. The vendor documents `sharer_id` as "a
  Red Hat Extension", which is Bugzilla's phrase for "not upstream API" and does not establish
  that the `extensions/RedHat` plugin is what implements it; Red Hat's source was not read. A
  fork that implements saved searches without advertising `RedHat` is therefore refused a
  request its server would have honoured. That false negative is the accepted cost, and it is
  the mirror of what passthrough gets wrong: passthrough is silently wrong on most servers,
  detection is loudly wrong on few.
- **The proxy also fails in the other direction, and the gate does not catch it.** A server that
  advertises `RedHat` but whose `Bug.search` does not implement saved searches — a partial
  deployment, a fork of the fork, a future version that drops the parameter — passes the gate,
  and bzr then dispatches and returns whatever comes back. The same holds for a name the server
  does implement but cannot resolve. Nothing verifies the *result*, and nothing should: proving a
  saved search was applied would mean re-running the query unfiltered and comparing, which costs
  more than the risk it removes. So the guarantee this decision buys is precise — bzr will not
  send the parameter to a server that says it lacks the extension — and it is not a guarantee
  that a dispatched saved search was resolved.
- `ServerConfig` gains a persisted field, and a cached answer has no TTL. A server *upgraded in
  place* to add the extension keeps being refused until the cache is cleared. The refusal message
  says so and names the remedy the tool actually supports: delete the `server_extensions` key for
  that server in `config.toml`. It deliberately does **not** point at `bzr config set-server`,
  which replaces the whole entry — including the keyring reference, TLS pin and TOFU issuer state
  — and would silently unpin a server the user pinned. This staleness is worse than a stale
  `api_mode` or `auth_method`, which surface as a visible connection failure rather than a
  plausible-looking refusal. Only
  capabilities bzr acts on are stored — the probe response is server-controlled and unbounded,
  and persisting it verbatim would write arbitrary server text into the user's config. The cached
  answer is bound to the URL it was probed from and discarded when that changes, so a server name
  re-pointed at a different host cannot inherit the old host's capabilities and let the gate pass
  for a server that never advertised them.
- ~~**The probe is REST-only.**~~ `GET /rest/extensions` was issued whatever the resolved API
  mode, so on a deployment with REST disabled the capability could never be established and the
  feature was permanently undetermined — even on a fork whose XML-RPC `Bug.search` would honour
  the parameter. Routing the probe through the resolved transport was deferred here and is no
  longer deferred: see the amendment below.
- **The gate checks capability, not identity.** `bug search` is an anonymous-capable command, and
  the gate does not change that. A saved search names a query stored in an account, so on a server
  that does implement the extension an uncredentialed `--saved-search` is still dispatched and the
  server decides what an anonymous caller may resolve. Requiring a credential for this one flag is
  a separate decision from requiring the capability, and is not taken here.
- **Raw URL parameters stay ungated.** `bzr bug search --from-url` classifies any unmodelled
  key as a raw passthrough, so a `buglist.cgi` URL carrying `savedsearch`/`sharer_id` — or the
  `cmdtype=runnamed&namedcmd=<name>` form the Bugzilla UI actually produces — reaches the server
  without a capability check, and can be persisted by `--save-as` and replayed by
  `bzr query run`. Gating raw passthrough would require bzr to model every vendor parameter it
  currently forwards blindly; this decision does not attempt that, and the documentation says so
  rather than implying the protection is universal.
- Sibling issues that ship a genuine *vendor extension* inherit this rule and the mechanism.
  #679 and #680 are the current candidates. #671 and #672 are **not** — see the scope boundary
  below, which is what makes the rule usable rather than a blanket instruction.
- bzr has no runtime version-gating mechanism to hang a version-floor decision on. The
  `supports_*` flags in `src/types/capabilities.rs` derive from `ApiMode` via
  `supports_rest_surface(mode)`, not from the server version, and `server_version`
  (`src/config/model.rs`, `src/client/auth/mod.rs`) only feeds API-mode selection. The
  repository's one precedent for a version-gated field, `require_version 520` in
  `tests/functional/lib.sh`, gates the *test*, not the client. So the alternative path this
  decision declines to take is genuinely unbuilt, which is part of why the version-floor case
  below resolves differently.

## Scope: vendor extensions, not core version floors

This decision governs **vendor extensions** — parameters that only ever work on one vendor's
fork. Sending one is a bet about whose server this is, and the bet is unresolvable from the
version, so refusing when the fork does not announce itself is the only honest answer.

It does **not** govern a **core feature behind a version floor**. That works on any server new
enough, and the floor moves in the user's favour with every upgrade. Refusing a request the
user's next upgrade makes valid is not a service to them, so the right response there is to
proceed and warn, not to error.

Two worked counter-examples, both core and both outside this ADR, and they fall outside for
different reasons — which is the point of naming both:

- **`minor_update` (#672) — core, gated by server version.** Verified against Bugzilla source
  in the project's own containers: it is core `Bug.update` plumbing, present in 5.3.3+ at
  `Bugzilla/WebService/Bug.pm` `sub update` and tracked through `has_unsent_changes` into
  `send_changes`, and simply absent from 5.0.6 and 5.2. It never lives under `extensions/`, so
  `GET /rest/extensions` would not report it either way. #672 ships it with a stderr warning
  below the floor rather than an error, and that is the correct call for this shape.
- **`cf_*` custom-field writes (#671) — core, varying by per-installation schema.** Custom
  fields are stock upstream Bugzilla: real columns on the `bugs` table with a dedicated branch
  in `Bugzilla::Bug::set_all`. They are neither a fork's parameter nor version-gated, and
  decisively for this ADR's mechanism, `GET /rest/extensions` **cannot report them at all**,
  because they are per-installation schema rather than an extension. There is nothing for the
  probe to return, so #671 cannot inherit this mechanism even in principle. bzr has no field
  catalogue today either — `src/client/resources/field.rs` exposes only
  `get_field_values(field_name)`, and `src/types/field.rs` models a field's *values*, not the
  field itself, with no `is_custom` and no `type` — so per-installation schema is currently
  outside bzr's reach entirely. Whatever mechanism serves it will be a separate one, not an
  extension of this.

The test for whether this ADR applies is therefore not "is the parameter missing upstream?" but
"is it missing because of *whose* server this is?" `savedsearch` and `sharer_id` sit cleanly on
the vendor-extension side; the two above do not.

## Considered & rejected

- **Forward the parameter and document the silent no-op.** verified: this is what
  python-bugzilla does — `build_query` places `savedsearch` and `sharer_id` directly into the
  `Bug.search` query with no capability check (python-bugzilla 3.3.0, `bugzilla/base.py`) — and
  live probes on all three functional images show the resulting request returning an unfiltered
  result with exit 0. judgment: rejected by the operator. Documentation in `--help` and the CLI
  reference reaches only the user who reads it before running the command, and the failure it
  is warning about is invisible in the output: a filtered query and an unfiltered one differ
  only in a row count the user has no baseline for. Disclosure puts the burden on the person
  least able to detect the problem. This was the alternative originally designed for #670 and
  recommended; it is recorded here at length because it is the one a later reader is most
  likely to re-propose.
- **Forward the parameter and warn on stderr.** judgment: a warning that always fires on stock
  servers and never on Red Hat ones is a capability check that has already been performed —
  having paid for the detection, refusing is strictly more useful than proceeding with a result
  known to be wrong. It also leaves the exit code at 0, so no script can act on it.
- **Fail closed when the probe itself fails.** judgment: rejected as the *sole* behaviour, not
  as a path — conflating an unreachable `/rest/extensions` with an absent extension turns a
  transient network fault into a confident statement about the server's capabilities. Decision
  point 2 keeps both refusals while keeping them distinguishable.
- **Resolve the capability client-side instead of asking the server.** verified: for saved
  searches specifically, upstream exposes named queries only through
  `buglist.cgi?cmdtype=runnamed`; no module under `Bugzilla/WebService/` in any supported image
  references the `namedqueries` table. judgment: scraping a CGI page outside the API surface bzr
  is built on, to emulate a vendor extension, is a larger and less durable commitment than
  refusing.
- **Reuse an existing `BzrError` variant for the refusal.** verified: all eighteen were
  checked against `exit_code()`/`error_type()` in `src/error.rs`. `InputValidation` blames
  well-formed input; `Api` and `XmlRpc` mean the server reported a fault, and here it reported
  nothing — bzr decided; `NotFound`, `Config` and `Auth` describe unrelated conditions.
  judgment: an unsupported-server condition is its own class and gets its own variant and exit
  code, because stretching a variant makes both its exit code and its `error_type` lie to every
  consumer of the structured error contract.
- **Do nothing — leave the five gaps open.** judgment: the campaign exists to close the surface
  difference with python-bugzilla, and four of the five capabilities work correctly on the
  servers that implement them. Declining to ship them to avoid one documented behavioural
  difference trades five working features for one paragraph.

## Amendment (2026-09-06): the probe follows the transport in use

Issue #724. The original decision above is unchanged; this amends only *how* support is
established.

**What this amendment does not touch, stated first because it is what a reader most needs.**
This changes **where the capability answer comes from**, never **whether an undetermined
capability may proceed**. An undetermined capability still refuses. The refusal, not a warning,
is the decision — argued at length under *Considered & rejected* above, in the two bullets that
reject forwarding the parameter with documentation and with a stderr warning. Both remain the
governing answer and neither is reopened here.

That is worth repeating rather than assuming, because "warn instead of refusing" is the
*locally obvious* design: it regenerates from the code alone, and three independent review
passes over the sibling work in #710 proposed it afresh with no knowledge of each other. A
reader arriving at this record through the transport dispatch below will meet the same
intuition. The answer is the one the original decision gives: a warning that always fires on
stock servers and never on Red Hat ones is a capability check already performed, and it leaves
the exit code at 0 so no script can act on it.

### Context

The Consequences section above deferred routing the probe through the resolved transport and
left open whether it was worth doing at all. **That was a fair question, honestly asked**: the
behaviour failed closed with an actionable error, the limitation was disclosed rather than
hidden, and REST-disabled deployments are genuinely rare. What answers it is not that the
question was confused but that the cost side of its trade-off was overstated — most of the
XML-RPC path was already built. Three measurements settle it, taken 2026-09-06 against the
project's own functional images and a proxy that forwards `/xmlrpc.cgi` to bz50 while answering
`503` on every `/rest/` path:

- The XML-RPC search path already carries the vendor parameters —
  `src/xmlrpc/resources/bug.rs` inserts `savedsearch` and `sharer_id` into `Bug.search` — and
  `bzr --api xmlrpc bug search` returns real bugs against the REST-disabled proxy. The gate is
  the only thing blocking the feature there, so this was an incomplete implementation rather
  than an unbuilt one.
- `Bugzilla.extensions` answers over XML-RPC on bz50, bz52 and bz53 unauthenticated, returning
  the same `{extensions: {...}}` shape REST returns.
- Against a fully REST-enabled bz50, `bzr --api xmlrpc bug search --saved-search <name>` still
  issues `GET /rest/extensions`. `--api` is documented as a *preference* with acknowledged
  per-operation exceptions (`docs/bzr-cli.md`, and the `--api` doc comment in `src/cli/mod.rs`,
  which names comments and attachments as standing exceptions), so this was a defensible
  exception rather than a broken promise — which is what made the issue's "is this worth fixing
  at all?" a real question. What settles it is that the exception is now the *only* REST
  dependency left on an otherwise complete XML-RPC path, and removing it costs about forty
  lines.
- **Nothing here was hidden, and that is the point.** The *undetermined* message already told
  the user the probe needs the REST surface even under `--api xmlrpc`, and the limitation was
  recorded as a consequence of this very record. The gap was stated in the user-facing error,
  written down as a known cost, and still unactionable — because the only escape it offered was
  a transport the affected deployment does not expose. A limitation a user can read and cannot
  act on is a reason to close it, not evidence that disclosure was sufficient.

### Decision

**`BugzillaClient::server_extensions()` establishes the extension list over the transport in
use.** `ApiMode::Rest` probes REST; `ApiMode::XmlRpc` calls `Bugzilla.extensions`;
`ApiMode::Hybrid` probes REST first and falls back to XML-RPC when the REST probe returns **any**
error. That is the same *order* `search_bugs_hybrid` uses, though not the same trigger: that
method's own XML-RPC retry fires on an empty structured-filter result and it has no
failure-driven fallback at all.

Three properties of the amendment are part of the decision:

1. **The Hybrid fallback deliberately does not use `BzrError::is_transport_failure()`**, the
   predicate the rest of the client uses for "the other transport might do better". That
   predicate matches `Http | HttpStatus | XmlRpc` (`src/error.rs`), and
   `error_from_status_body` (`src/client/response.rs`) classifies *any* REST error whose body
   parses as a Bugzilla error envelope as `BzrError::Api` instead — which Bugzilla sends even
   for a 404 on an absent endpoint, verified against 5.0.6 and 5.3.3+. So a transport-failure
   guard would not fire for the commonest shape of "REST did not serve this endpoint", which is
   the case the fallback exists for.

   Widening the predicate to add `BzrError::Api` was considered and rejected: it would leave
   every *other* variant outside the fallback — a wrong-shaped REST body yields `Deserialize`,
   a socket-level failure `Io` — so it fails less often without being right, and waits for the
   next variant nobody enumerated. **The fallback is therefore deliberately unconditional**, on
   any `Err`. That is not a looser rule than a predicate; it is the absence of a rule that could
   be wrong.

   Falling back on any error is safe here in a way it is not in general, and the reason is
   specific to this probe: an error *from the extensions endpoint* is a statement about that
   endpoint, never about the capability. The XML-RPC probe returns the server's own extension
   list, so falling back more often can only make the verdict more accurate — it can never
   fabricate a capability the server does not advertise, and both transports failing still
   yields *undetermined*. **Do not "fix" this into consistency with `is_transport_failure()`**;
   doing so silently restores the defect, and the evidence that it would is above: an enveloped
   404 surfaces as `Api`, so the obvious predicate misses this record's own motivating case.
2. **The cached answer gains no transport dimension.** `Bugzilla.extensions` and
   `/rest/extensions` are two views of the same Bugzilla handler, so the advertised list is
   treated as a property of the server rather than of the transport. Note the limit of the
   evidence: all three images return an *empty* list on both transports, which confirms both
   probes reach the endpoint and cannot confirm that non-empty lists would agree — an empty map
   matches an empty map whatever either probe does. The URL and capability-allowlist binding
   specified above is unchanged, and a third dimension would only cause redundant probes.
3. **A malformed XML-RPC response is an error, not an empty or partial map.** Three shapes: a
   missing `extensions` member, an `extensions` member that is not a struct, and an extension
   whose own value is not a struct. An empty map renders as *absent* — a settled refusal — and a
   map that keeps a name whose value bzr could not read renders as *advertised*; the REST path's
   serde decode fails on all three inputs and renders *undetermined*. The transports must reach
   the same verdict from the same evidence, in the conservative direction, so the adapter fails
   rather than inventing a list. The same rule extends to a present-but-non-string `version`,
   which `Option<String>` rejects on the REST side.

   Those four are the shapes **both transports can produce**, not every conceivable divergence.
   One residual runs the other way and is left alone deliberately: an XML-RPC `<value><nil/></value>`
   `version` fails the parser globally (`Value` has no nil variant) and renders *undetermined*,
   where serde decodes JSON `null` into `version: None` and renders *advertised*. It is
   conservative, and no Bugzilla emits it — the version comes from a Perl `VERSION` constant and
   is always a string — so handling it would grow the adapter for an input that does not occur.

### Consequences

- The refusal messages no longer name `/rest/extensions`, because the probe no longer always
  goes there. Nor do the three other places that described the REST-only probe: the
  `bug search --saved-search` note in `docs/bzr-cli.md`, the `bug search` long-about in
  `src/cli/bug/search.rs` (which is `--help` and the man page), and the rustdoc on
  `ServerConfig::server_extensions`. **In Hybrid the *undetermined* message names both attempts**
  rather than only detailing the XML-RPC failure. The REST error is also logged at `warn`, but a
  log line is a trace event and a `--json` consumer reads the error body and never the trace, so
  without this a user on a REST-first connection would see only an XML-RPC failure and
  reasonably conclude bzr never tried REST. A **successful** fallback is
  not silent either: the REST failure is logged at `warn`, because it fires only when the probe
  actually failed and would otherwise be the one case with no signal at all — papering over a
  degraded REST surface on every invocation, and masking a bzr-side decode defect that the
  unconditional fallback would route around.
- **`bzr server info` is the other consumer and its behaviour improves.** It reaches this method
  through `server_info()`. On a server whose `/rest/version` works but whose `/rest/extensions`
  does not, it previously failed outright and now returns the list over XML-RPC. It cannot
  produce wrong data — both transports report the same server's own advertised list — and
  nothing changes where REST works, or on a fully REST-disabled deployment, where
  `server_version()` still fails first.
- Fail-closed is preserved: the three outcomes, the error variant, the exit code and the
  `capability_status` values are untouched, and no path is added on which an undetermined
  capability dispatches the parameter.
- **The Hybrid fallback covers a broken extensions endpoint, not a REST-disabled deployment.**
  `search_bugs_hybrid` `?`-propagates its REST call, so on a Hybrid connection to a
  REST-unreachable server the gate now resolves over XML-RPC and `bug search` then fails at its
  own REST call — a different error, not a working search. What the arm actually buys is the
  case where `/rest/extensions` specifically fails (a transient 503, a proxy allowlisting
  `/rest/bug` but not `/rest/extensions`) while REST search works. A REST-disabled deployment is
  `--api xmlrpc`, which detection already selects when version probing fails.
- **The Hybrid fallback deliberately keeps the full request timeout**, rather than the 8-second
  `XMLRPC_FALLBACK_TIMEOUT` the analogous `search_bugs_hybrid` retry uses. That constant exists
  to stop an opportunistic retry making the user pay the ceiling twice *when REST already
  produced a usable answer*. Here REST produced no answer at all, so there is nothing to
  protect and a short cap would convert a slow-but-working XML-RPC probe into a spurious
  *undetermined* — a refusal. The cost is that a server stalling both surfaces holds the
  command for roughly double the previous wait before refusing; that is the right trade for a
  gate whose failure mode is refusing work the server would have honoured.
- **A cached answer is now transport-agnostic.** Before this change every probe went over REST,
  so every cached answer was a REST answer. Now an answer probed under `--api xmlrpc` is served
  to a later `--api rest` invocation and the reverse. The fail-open direction is already covered
  above by the `RedHat`-as-proxy consequence; the new direction is a cached *absent* written
  over one transport that goes on refusing the other, with the same absence of a TTL this record
  already discusses for a server upgraded in place. The remedy is the one the refusal message
  already names: delete the server's `server_extensions` key in `config.toml`.
- **The `version` field is read strictly on both transports**, so a present-but-non-string
  `version` is *undetermined* rather than an extension advertised with no version. This is
  parity for its own sake rather than a reachable hazard — a server sending it is already
  advertising the extension — but the fail-closed property above is stated as complete, and a
  reader will rely on that completeness instead of re-deriving it.
- A cached answer written by this code can differ from one the previous code could have
  written: on a REST-unreachable server the probe now reaches a settled *absent*, where before
  it could only fail undetermined and cache nothing. So reverting this change does not restore
  the previous behaviour on such a server until that cached entry is cleared, by the same
  `server_extensions` config remedy the refusal message already names.
- **The positive verdict over XML-RPC is not covered by a container test, and the reason is
  narrower than "no image advertises the extension".** No supported image does, which is why
  the phase tier here covers the probe mechanism and the *negative* verdict over a real XML-RPC
  wire — discriminating on which transport carried the probe, and on *absent* versus
  *undetermined*, since both exit 15. The positive verdict and the REST-disabled scenario are
  covered at the wiremock tier, which can mount a `RedHat`-advertising XML-RPC response beside
  a failing REST surface.

  **[ADR 0061](0061-prove-vendor-extension-behaviour-against-a-shaped-proxy.md) has since built
  the mechanism that closes the other half**, and this record would be stale without saying so:
  `tests/functional/redhat-shape-proxy.py` rewrites a real `/rest/extensions` response to
  advertise `RedHat`, so a *positive* verdict is provable against a shaped proxy rather than
  only against a stock image. The phase tier can reach it — `lib.sh` exports
  `redhat_shape_start`/`redhat_shape_stop`, and they are called from nine scripts under
  `tests/functional/phases/` (`rg -l 'redhat_shape_start|redhat_shape_stop'`, 2026-09-06;
  fourteen across all of `tests/functional/`). So this is **not** a tier boundary.

  What blocks the XML-RPC positive path is the proxy's own shape: **it contains no XML-RPC
  handling at all.** `REWRITE_HOOKS` is a tuple of matcher/transformer pairs whose matchers key
  on `/rest/*` paths and whose transformers are JSON rewriters end to end. Shaping
  `Bugzilla.extensions` therefore means intercepting `POST /xmlrpc.cgi`, parsing an XML
  `methodResponse` struct, injecting a `RedHat` member and re-serialising — and discriminating
  by RPC method name *inside the request body*, which the existing matcher contract
  (`method`, `path`, `enabled_modes`) cannot see. That is a new capability in a proxy that has
  never touched XML, with its own fault surface and its own cases in that file's self-test.

  So criterion 2's strongest reading is **satisfiable and deliberately deferred, not
  impossible** — sized as a design pass over the proxy rather than a call to an existing
  helper. Deferred to the operator batch rather than taken here.

  **That reason covers the positive verdict only, and one other gap must not borrow it.** The
  Hybrid arm — REST probe fails, XML-RPC answers — has no container-tier coverage either, and
  it is *not* blocked by XML-RPC shaping: it needs only the REST probe to fail, which is a
  body-only rewrite of `GET /rest/extensions` into a Bugzilla error envelope and is exactly
  what the proxy's existing `(matcher, transformer)` contract already does. The verdict
  discriminates on its own, since without the fallback the run reports *undetermined* and with
  it reports *absent* over the real XML-RPC wire. So this gap is cheap to close and is left
  open only because the proxy is outside the surface #724 was dispatched with — which is a
  scope reason, not a technical one, and is recorded as such so it is not mistaken for the
  harder deferral above.

  The limit is stated rather than papered over, because a probe tested only against servers
  that all lack the capability is otherwise an oracle that cannot fail.
- `BugzillaClient::server_version()` stays REST-only, so `bzr server info` against a
  REST-disabled deployment still fails at the version step. The capability gate never calls it,
  so it was left out of this amendment rather than folded in.

### Considered & rejected

- **Leave it REST-only and document the limitation.** verified: against the REST-disabled
  proxy, `bzr --api xmlrpc bug search "test" --limit 1` returns a bug at exit 0 while
  `--saved-search` returns `capability_status: undetermined` at exit 15 (bzr at `dcaf259f`,
  macOS, bz50 image). judgment: documenting a refusal the tool could simply stop issuing, on a
  path whose other half is already built, spends a paragraph to avoid ~40 lines of adapter.
- **Use the existing `dispatch_xmlrpc_first` helper for all three modes.** verified: Hybrid
  `bug search` is REST-first — `search_bugs_configured` routes `ApiMode::Hybrid` to
  `search_bugs_hybrid`, which calls `search_bugs_rest` first (`src/client/resources/bug.rs`).
  judgment: XML-RPC-first would change the probe's transport for the default mode on 5.0/5.2 —
  most users — for no requirement, and would put the gate on a different transport from the
  command it gates.
- **Add a generic REST-first dispatch helper beside `dispatch_xmlrpc_first`.** judgment: one
  caller. Three lines inline in the resource that needs them, until a second caller exists.
- **Also route `server_version()` through the transport in use.** judgment: correct, and not
  reachable from any #724 criterion — the gate never calls it. Widening the change to fix an
  adjacent command is scope this record does not own.
