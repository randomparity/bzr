# ADR 0052: Detect vendor-extension support before dispatch

## Status

Accepted

## Context

python-bugzilla exposes several Bugzilla search and mutation parameters that are Red Hat
extensions rather than upstream Bugzilla API. The parity campaign tracked in
`docs/dev/python-bugzilla-parity.md` asks bzr to match that surface across five issues: #670
server-side saved searches (`savedsearch`, `sharer_id`), #671 generic arbitrary fields, #672
comment tags and minor update, #679 whiteboard match types, and #680 personal bug tags.

Issue #670 established what "unsupported" means in practice, and the finding generalises.
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
- **The probe is REST-only.** `GET /rest/extensions` is issued whatever the resolved API mode,
  so on a deployment with REST disabled the capability can never be established and the feature
  is permanently undetermined — even on a fork whose XML-RPC `Bug.search` would honour the
  parameter. The undetermined message says the probe needs the REST surface. Routing the probe
  through the resolved transport is deferred.
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
- Sibling issues #671, #672, #679 and #680 inherit this rule and the mechanism, rather than
  each re-deciding.

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
