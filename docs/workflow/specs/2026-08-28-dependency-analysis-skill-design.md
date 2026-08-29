# Dependency-analysis skill design

## Goal

Add an embedded `bzr-dependency-analysis` skill that teaches an agent to collect a
bounded Bugzilla dependency graph, preserve incomplete evidence, analyze structural
delivery constraints, and render safe artifacts without mutating Bugzilla.

The skill is an agent workflow, not a native graph engine. It may compose released `bzr`
commands and local artifact tools, but graph construction, cycle detection, ordering, and
critical-path interpretation remain outside the Rust CLI.

## Existing primitive investigation

At commit `16b26c6`, neither existing retrieval path is sufficient for the complete
workflow:

- `bzr bug links` batches a breadth-first traversal, but emits each node only once with its
  discovery edge. Diamonds, back-edges, and cycle-closing edges are absent, and inaccessible
  related bugs are silently skipped.
- `bzr bug view ID... --permissive` preserves per-bug failures and exposes `blocks` and
  `depends_on`, but fetches every requested ID sequentially.

The skill can still deliver correct bounded analysis by fetching each discovered bug once
with single-ID structured requests and retaining every returned adjacency list. A bundled Python
standard-library helper analyzes captured node records deterministically; when Python is
unavailable, the skill may perform the same steps directly but must report that the executable
helper was unavailable. It must explicitly warn that collection can make one request per bug.
A separate narrowly scoped issue will
request a deterministic batched adjacency primitive that preserves all requested IDs and
their failures. Adoption of any released primitive belongs to a later skill update; this version
must not describe an unreleased command or flag.

## Workflow

### Inputs and defaults

Before retrieval, the skill resolves:

- one configured server per input scope; multi-server analysis is a union of separately
  fetched server-qualified graphs;
- roots from bug IDs, one alias per server, a named saved query, a Custom Search URL, a target
  milestone, or a version;
- direction: `depends_on`, `blocks`, or both;
- maximum depth and maximum distinct nodes, both required and positive; optional positive
  `max_relationships` defaults to `max_nodes`; node and relationship bounds are at most 9,999,
  and the node ceiling keeps every component in the four-digit `cNNNN` namespace;
- whether resolved nodes are included and traversed or included but not traversed;
- optional product, milestone, or query-membership restriction;
- stale threshold, analysis timestamp, and requested output formats.

If the user omits bounds, the skill proposes conservative defaults of depth 5, 200 nodes, and 200
relationships and states them before fetching. An explicit relationship bound may raise that
budget independently. The collector rejects a node or relationship bound above 9,999, duplicate
policy keys, and invalid UTF-8 before invoking a runner. It never starts an unbounded walk. Refresh is an
explicit new collection run; within one run a server-qualified bug identity is fetched at most
once.

### Collection

Scope commands use versioned JSON and request only fields needed by the selected analysis.
Named queries use `bzr query run`; Custom Search URLs use `bzr bug search --from-url`;
milestone/version scopes use `bzr bug list`. Every scope and restriction request overrides saved
or URL order with `--sort bug_id --order asc` and uses stable offsets. Starting scopes request at most `node_cap + 1`
rows without `--paginate`; the extra row detects truncation and becomes aggregate scope metadata,
not a node. A query used as a membership restriction is enumerated manually in pages whose total
requested rows never exceeds `node_cap + 1`; the collector stops on the extra row and asks the user
to narrow the query or raise the cap. It never uses `--count` as an upper-bound proof because the
server may cap that result. If a server returns fewer rows than requested, the collector advances
the offset and probes once more; an empty page proves completion, while the cumulative extra row
proves oversize. This bounds both membership storage and requests by the chosen cap.

The bundled `scripts/collect.py` owns collection and invokes `bzr` through a command-runner
boundary. Tests substitute a recorded runner; live use invokes the configured binary without a
shell. Before other retrieval, it preflights every declared server exactly once with released
read-only `bug list --limit 1 --offset 0 --fields id --sort bug_id --order asc`. Any preflight
failure is command-fatal. The collector maintains `queued`, `fetched`, and `nodes` sets keyed by
`(server, bug-id)`.
Admission immediately creates a `boundary` node with `boundary_reason: pending_fetch`; every
admitted identity therefore has a legal record before its command runs.
Root processing follows server aliases lexically, with the optional alias root before numeric roots
for each server regardless of caller input order. Before every alias lookup, the collector
reserves and consumes one node-cap slot and creates its pending boundary record; a failed lookup
retains that slot. When no slot remains, later aliases are not looked up. A successful alias response
must contain one numeric ID. The collector re-keys the temporary alias record to
`(server, returned_id)`, sets canonical `requested` to the decimal numeric ID, preserves the original
alias only in a sorted `requested_aliases` array, and stable-deduplicates numeric roots and later
adjacency against that key without another fetch or cap charge. More than one alias for the same
server is rejected before collection because the released surface cannot prove two aliases differ
without fetching the same underlying bug twice.
Because alias roots are canonicalized first, a matching numeric root is never already admitted: the
temporary reservation becomes the canonical node's single slot, and the later numeric root merges
without reserving or releasing another slot. DA-18 permutes caller root order at caps 1 and 2 and
expects the same one-slot result.
The current fallback fetches each ID separately with `bzr --json bug view ID --fields
id,summary,status,resolution,assigned_to,last_change_time,blocks,depends_on`, because only a
command-level failure exposes the versioned structured error envelope. It therefore makes at most
one command per admitted bug. The field list additionally
requests `product`, `target_milestone`, or `version` when the corresponding restriction is
active. A released `bzr` command produces a complete JSON response, which the collector captures
and parses before applying `max_relationships`. The relationship cap therefore bounds only the
post-parse relationship records retained, staged, discovered from, and traversed. It does not bound
the upstream Bugzilla/`bzr` response size, JSON-decoding work, or peak parse memory; bounded
upstream retrieval is deferred to issue #573. Both adjacency fields are fetched so reciprocal
evidence can be normalized. The selected direction controls discovery, admission, node-cap
consumption, and omitted-identity counts:
`depends_on` selects dependency neighbors, `blocks` selects blocking neighbors, and `both` selects
their union. For a one-direction policy, the collector stages the selected field before optional
reciprocal evidence. For `both`, the canonical order is `blocks` then `depends_on`. Within a field,
the collector preserves response order until the aggregate relationship budget is consumed; it
never retains or discovers from the unprocessed suffix. After all admission and fetching finishes,
a deterministic second pass first
filters staged observations to pairs whose endpoints are admitted, establishes canonical edges
from selected observations, then attaches unselected observations only when they normalize onto
one of those established edges. Thus
unselected evidence can enrich provenance but cannot admit an endpoint, create an edge, or alter
analysis, regardless of fetch order. Query restrictions
use the bounded, fully paginated server-qualified membership set
described above. Returned adjacency lists add an observation when both endpoint nodes are admitted,
even when the target is already fetched. Classified per-ID failures create unknown nodes
carrying only the server, requested identifier, and structured error type. A failed or missing node is never
silently removed.

Empirical functional-server evidence fixes the fallback boundary. Across the repository's stock
Bugzilla 5.0.6, 5.2, and 5.3.3+ harnesses, single-ID reads return structured API code 101 for a missing ID and 102 for an
inaccessible restricted bug; a connection refusal is a global HTTP failure. Multi-ID
`--permissive` continues after codes 101/102 but exposes only an untyped prose `error`, and an
alias plus its numeric ID returns the same bug twice. Consequently collection uses single-ID
commands for typed classification and performs its own alias/numeric collapse. `bug links` batches
each breadth-first frontier efficiently, but omits the root, collapses revisited observations, and
cannot expose inaccessible endpoints, so it is discovery evidence rather than a complete graph
inventory. Stock Bugzilla rejects circular dependency mutations; cycle behavior remains a
deterministic helper fixture rather than a live-server fixture.

Depth is the minimum hop distance from any root. A newly discovered node beyond the maximum
depth is recorded as a boundary node but not fetched, provided it fits inside the maximum
distinct-node count. The node cap covers every emitted known, unknown, or boundary
node. Once exhausted, no more node identities are emitted; the graph metadata records an
aggregate `cap_reached: true` and `omitted_discovered_identities` count. The count is the number of
distinct server-qualified identities rejected while scanning all adjacency lists already returned
for the current frontier; duplicate observations count once. No further frontier is fetched, so
deeper undiscovered identities are neither named nor counted. Collection order is canonical: server
aliases lexically, numeric roots sorted ascending after stable deduplication,
breadth-first depth, and numeric bug ID within each frontier. Output records use canonical sorted
order. Relationship exhaustion stops further staging, discovery, and fetching, preserves the
already admitted graph, and adds limitation `relationship_cap`. Its
`omitted_relationships_lower_bound` counts only visibly skipped entries in fetched arrays and is
explicitly a lower bound because unprocessed nodes may contain more. Scope restrictions
are evaluated from fetched fields or the initial scope membership; nodes outside the
restriction remain visible boundary nodes and are not traversed.

Reaching the cap closes admission but does not interrupt the current frontier. Every already
admitted identity in that frontier is fetched in canonical order. Their returned adjacency lists
record observations only between admitted endpoints and contribute rejected identities to the
deduplicated omitted count. Collection then stops before fetching a next frontier.

After successful server preflight, the single-ID structured error envelope makes Bugzilla API
codes 100/101 (`not_found`) and 102 (`inaccessible`) per-resource failures; each becomes an unknown
node with that stable `error_type`. Structured `type=not_found,resource=bug` is also `not_found`,
including in XML-RPC mode. Code 102 without successful preflight is ambiguous and fatal.
Other `api` failures and every `http` failure are fatal. Global
authentication, TLS, transport, schema-version, malformed-output, and unclassified failures stop
collection and preserve a partial inventory with a run-level limitation. Batch failures are never
copied onto every requested node. Shareable records retain only error type and affected identifier,
not raw stderr or server messages.
On fatal stop, the identity whose command failed and every admitted but unfetched identity remain
`boundary` with `boundary_reason: fetch_interrupted`; the generic failure exists only as a run-level
limitation. A successful fetch changes the node to `known`; a classified per-resource failure
changes it to `unknown`. For an alias lookup, structured `not_found` or `inaccessible` retains the
consumed slot and becomes a nonnumeric
`unknown` node with null `id`, the original alias in `requested`, an empty `requested_aliases`, and
no run-fatal limitation. Every other alias failure takes the fatal `fetch_interrupted` transition.

The collector atomically writes one `bzr-dependency-collection/v1` JSON document. It contains
`status` (`complete` or
`partial`), nodes, observed edges, bounds, sanitized provenance, omitted counts, and limitations.
A complete run exits 0. A fatal run still writes a valid `partial` document, writes one generic
error-type line to stderr, and exits 1. A consumer rejects `partial` input unless the operator
passes `--allow-partial`, so no prefix stream can be mistaken for completion. `analyze.py` reads
only that schema and atomically writes one `bzr-dependency-analysis/v1` JSON document, preserving
status and limitations. Both readers reject truncated JSON and unknown schema versions.

The normative collection shape is:

```json
{
  "analysis_timestamp": "2026-08-28T12:00:00Z",
  "bounds": {"max_depth": 5, "max_nodes": 200, "max_relationships": 200},
  "cap": {
    "graph_cap_reached": false,
    "omitted_discovered_identities": 0,
    "omitted_relationships_lower_bound": 0,
    "relationship_cap_reached": false,
    "scope_truncated": false
  },
  "limitations": [],
  "nodes": [{
    "assigned_to": null,
    "boundary_reason": null,
    "depth": 0,
    "error_type": null,
    "id": 1200,
    "last_change_time": null,
    "provenance": {"command": "bug view", "server": "primary"},
    "requested": "1200",
    "requested_aliases": [],
    "resolution": null,
    "server": "primary",
    "state": "known",
    "status": "NEW",
    "summary": "Example"
  }],
  "observations": [{
    "field": "depends_on",
    "source": {"id": 1200, "server": "primary"},
    "target": {"id": 1199, "server": "primary"}
  }],
  "provenance": [{"parameter_names": [], "scope_kind": "bug-ids", "source": null, "server": "primary"}],
  "policy": {
    "direction": "both",
    "duration": null,
    "resolved_mode": "include-no-traverse",
    "resolved_statuses": ["RESOLVED"],
    "stale_after_days": 14
  },
  "roots": [{"id": 1200, "requested": "1200", "server": "primary"}],
  "schema": "bzr-dependency-collection/v1",
  "status": "complete"
}
```

All displayed keys are required. Node `state` is `known`, `unknown`, or `boundary`; `requested` is
the decimal ID for a resolved node and otherwise the original string identifier. `id` is required for every numeric requested or discovered
identity, including `unknown` nodes, and is null only for an unresolved nonnumeric root alias.
After successful alias resolution, `requested` is the decimal numeric ID and every original alias
appears only in sorted `requested_aliases`.
Known nodes require the fetched fields and null `error_type`; unknown nodes require a stable
`error_type`, null fetched fields, and null `boundary_reason`; boundary nodes require null fetched
fields and null `error_type` plus one reason from `pending_fetch`, `depth_limit`, `scope_restriction`, or
`fetch_interrupted`. Provenance contains only the allowlisted command name and server alias. Nodes sort by
server, depth, resolved ID (null last), then requested string; observations sort by source server,
source ID, target server, target ID, then field. Every root entry requires `server`, nullable `id`,
and `requested`; it links to a node by `(server, id)` when ID is non-null and otherwise by
`(server, requested)`. A successfully resolved alias root serializes with numeric `id` and the same
decimal ID in `requested`; alias provenance exists only on the linked node's sorted
`requested_aliases`. An unresolved `not_found` alias root serializes with null `id` and the original
alias in `requested`. Roots sort by server, ID (null last), then requested. Every provenance entry
has exactly `parameter_names`, `scope_kind`, `source`, and `server`. `parameter_names` is a sorted
unique list of allowlisted names; `source` is the saved-query name only for `saved-query` and null
otherwise. Entries deduplicate by exact object equality and sort by server, then scope-kind rank
`bug-ids`, `alias`, `saved-query`, `custom-search`, `product`, `milestone`, `version`,
`restriction`, then null-last source, then the parameter-name list, all strings by Unicode code
point. Equivalent mixed scopes therefore serialize identically regardless of caller order;
limitations are sorted stable codes. Serialization is UTF-8, sorted object keys, two-space indent,
and one trailing newline. Starting-scope overflow sets `scope_truncated: true`, status `partial`, and
limitation `scope-node-cap`; the first `max_nodes` roots remain. Traversal exhaustion sets
`graph_cap_reached: true`, status `partial`, the deduplicated rejected-identity count, and limitation
`graph-node-cap`. An oversized membership restriction emits no nodes, status `partial`, limitation
`restriction-node-cap`, and `scope_truncated: true`; traversal does not start. Fatal collection adds
its stable run-level limitation code and status `partial` without changing already recorded nodes.
Relationship exhaustion sets `relationship_cap_reached: true`, limitation `relationship_cap`, and
the explicit lower-bound omission count without discarding already admitted nodes or observations.
The complete version 1 limitation vocabulary is `collection-api`, `collection-auth`,
`collection-http`, `collection-malformed-output`, `collection-schema-version`, `collection-tls`,
`collection-transport`, `collection-unclassified`, `graph-node-cap`, `relationship_cap`,
`restriction-node-cap`, and `scope-node-cap`; analyzer and renderer reject every other value.
Every observation endpoint must occur in `nodes`. When cap admission rejects a newly discovered
identity, all observations to that identity are omitted and the identity contributes exactly once
to `omitted_discovered_identities`.
Observation endpoint equality is `(server, id)`; observations can originate only from numeric
adjacency fields, so neither endpoint has a null ID.

The normative analyzer output is one `bzr-dependency-analysis/v1` document:

```json
{
  "analysis_timestamp": "2026-08-28T12:00:00Z",
  "bounds": {"max_depth": 5, "max_nodes": 200, "max_relationships": 200},
  "cap": {"graph_cap_reached": false, "omitted_discovered_identities": 0, "omitted_relationships_lower_bound": 0, "relationship_cap_reached": false, "scope_truncated": false},
  "components": [
    {"cyclic": false, "id": "c0001", "nodes": [{"id": 1199, "requested": null, "server": "primary"}]},
    {"cyclic": false, "id": "c0002", "nodes": [{"id": 1200, "requested": null, "server": "primary"}]}
  ],
  "edges": [{
    "observations": ["depends_on"],
    "predecessor": {"id": 1199, "server": "primary"},
    "successor": {"id": 1200, "server": "primary"}
  }],
  "findings": {
    "bottlenecks": [],
    "execution_order": {"assumptions": ["resolved-include-no-traverse"], "component_order": ["c0001", "c0002"], "cycle_impediments": [], "incomplete_boundaries": []},
    "structural_leaves": [{"id": 1200, "requested": null, "server": "primary"}],
    "structural_roots": [{"id": 1199, "requested": null, "server": "primary"}],
    "stale_blockers": [],
    "unassigned_blockers": []
  },
  "layers": [["c0001"], ["c0002"]],
  "limitations": [],
  "longest_chain": {"kind": "edge_count", "length": 1, "path": ["c0001", "c0002"]},
  "nodes": [
    {"assigned_to": null, "boundary_reason": null, "depth": 1, "error_type": null, "id": 1199, "last_change_time": null, "provenance": {"command": "bug view", "server": "primary"}, "requested": "1199", "requested_aliases": [], "resolution": "FIXED", "server": "primary", "stale": false, "state": "known", "status": "RESOLVED", "summary": "Foundation"},
    {"assigned_to": null, "boundary_reason": null, "depth": 0, "error_type": null, "id": 1200, "last_change_time": "2026-08-27T12:00:00Z", "provenance": {"command": "bug view", "server": "primary"}, "requested": "1200", "requested_aliases": [], "resolution": null, "server": "primary", "stale": false, "state": "known", "status": "NEW", "summary": "Delivery"}
  ],
  "policy": {"direction": "both", "duration": null, "resolved_mode": "include-no-traverse", "resolved_statuses": ["RESOLVED"], "stale_after_days": 14},
  "provenance": [{"parameter_names": [], "scope_kind": "bug-ids", "source": null, "server": "primary"}],
  "roots": [{"id": 1200, "requested": "1200", "server": "primary"}],
  "schema": "bzr-dependency-analysis/v1",
  "status": "complete",
  "warnings": []
}
```

Required top-level keys are exactly those shown: `analysis_timestamp`, `bounds`, `cap`, `components`,
`edges`, `findings`, `layers`, `limitations`, `longest_chain`, `nodes`, `policy`, `provenance`,
`roots`, `schema`, `status`, and `warnings`. The analyzer copies `analysis_timestamp`, `bounds`,
`cap`, `limitations`, `policy`, `provenance`, `roots`, and `status` byte-semantically from the
collection plus complete node records; each
copied node retains every collection key and adds required `stale` (`true`, `false`, or `unknown`).
Every component node exists in `nodes`, every node belongs to exactly one component, and every
layer/longest-chain component exists in `components`.
Component members and warning node lists use exactly the identity-reference object
`{"id": NUMBER_OR_NULL, "requested": STRING_OR_NULL, "server": STRING}`. `requested` is null
when `id` is numeric and is the unresolved original identifier when `id` is null. This same object
links an identity reference to exactly one analysis node. Edge endpoints remain the exact numeric
object `{"id": NUMBER, "server": STRING}` because observations cannot have null-ID endpoints.
The total analysis identity key is server alias by Unicode code-point order, then numeric ID; a
null-ID unresolved root sorts after numeric IDs by its `requested` string. Edges, warning node lists,
SCC minima/members, component numbering, and every tie break use this key; collection depth order
does not affect analysis identity. Warnings are stable objects `{code, nodes}` sorted by code then
node identity. The closed version 1 warning-code vocabulary is `stale-timestamp-future` and
`stale-timestamp-unknown`; the renderer rejects every other code. Missing/invalid timestamps use
`stale-timestamp-unknown`. Canonical edges sort by predecessor then successor and
contain sorted unique observation names. Strongly connected components sort by their smallest node
identity and receive IDs `c0001`, `c0002`, and so on through `c9999`; component node lists use node
order. Layers
use condensation-graph topological order with component-ID tie breaking. Longest-chain ties choose
the lexicographically smallest component-ID path. Cyclic components set `cyclic: true`; their
internal ordering is not presented as executable order. Partial collection input is rejected unless
`--allow-partial`; when allowed, status and limitations remain partial and all analyses are labelled
structural over incomplete evidence. Serialization uses the collection document's canonical JSON
rules.

`findings` has exactly the six keys shown. Structural roots have zero incoming canonical edges;
structural leaves have zero outgoing canonical edges. Both lists contain identity references in
identity order. A bottleneck is a node with more than one outgoing canonical edge and has the exact
shape `{"fan_out": NUMBER, "node": IDENTITY_REFERENCE}`; bottlenecks sort by descending `fan_out`
then identity. An unassigned blocker is a known unresolved node with null `assigned_to` and at
least one outgoing canonical edge; its list uses identity references in identity order.
A stale blocker is a node with `stale: true` and at least one outgoing canonical edge; its list
uses identity references in identity order. Stale nodes with no successors are not blockers.
`execution_order.component_order` is the concatenation of `layers`; `cycle_impediments` contains
cyclic component IDs in component order; and `incomplete_boundaries` contains every `unknown` or
`boundary` identity in identity order. `assumptions` contains stable sorted codes: the selected
resolved mode as `resolved-<mode>`, `partial-evidence` when status is partial, and
`cycles-prevent-total-node-order` when cycle impediments are nonempty. This is a component order,
not an order among members of a cyclic component.

For a legal zero-node partial input accepted with `--allow-partial`, `components`, `edges`,
`layers`, all five findings lists, `execution_order.component_order`, and
`execution_order.cycle_impediments` are empty; assumptions are
`["partial-evidence", "resolved-<mode>"]` in lexical order; `longest_chain` is exactly
`{"kind": "edge_count", "length": 0, "path": []}`; warnings follow the normal state rules.

Resolved-node policy is applied after recording the node and incoming edge. Status meanings
are installation-specific, so the skill asks which statuses count as resolved or states the
chosen assumption.

### Graph model and analysis

Each node contains server identity, bug ID or unresolved input, summary, status, resolution,
assignee, last-change time, fetch state (`known`, `unknown`, or `boundary`), and
the provenance command. Each directed edge contains its source, target, and Bugzilla field.
For scheduling orientation, `A depends_on B` means B precedes A; `A blocks B` is normalized
to the same predecessor relation. A canonical scheduling edge is keyed by
`(server, predecessor, successor)` and retains a set of source observations (`depends_on`,
`blocks`, or both). Degree, path, component, and ordering algorithms operate on canonical edges,
so reciprocal fields never double-count a relationship.

The agent runs standard graph operations over the collected inventory:

- strongly connected components identify cycles; cyclic components remain in output and are
  collapsed only for condensation/topological analysis;
- roots, leaves, in/out degree, and fan-out identify structural blockers and bottlenecks;
- topological layers of the condensed graph identify parallelizable groups;
- longest dependency chain uses edge count and is always labelled structural;
- suggested execution order lists assumptions, unknown/truncated boundaries, resolved-node
  policy, and every cyclic component that prevents a complete order.

Staleness is an observation, not an ordering weight. The user supplies a positive age threshold.
`collect.py` captures the current instant exactly once for a live run and canonicalizes it to
second-precision UTC RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`). Tests and replay may supply
`--analysis-timestamp` only in that exact form; malformed values fail before collection. The collector parses `last_change_time` as an
absolute timestamp. The analyzer derives every comparison solely from the collection's validated
`analysis_timestamp`. Staleness is total over legal nodes:

| Node case | `stale` | Warning |
| --- | --- | --- |
| known, unresolved, valid timestamp not in the future | age at least threshold | none |
| known, resolved, any timestamp value | `false` | none |
| known, unresolved, missing or invalid timestamp | `unknown` | `stale-timestamp-unknown` |
| known, unresolved, valid future timestamp | `unknown` | `stale-timestamp-future` |
| unknown | `unknown` | none |
| boundary (`pending_fetch`, `depth_limit`, `scope_restriction`, or `fetch_interrupted`) | `unknown` | none |

Schema-intentional null timestamps on resolved, unknown, and boundary nodes never emit timestamp
warnings. Warning node lists use the identity-reference object above, are deduplicated, and contain
exactly the affected known unresolved nodes.

Version 1 does not ingest duration data and always emits `policy.duration: null`. It reports only
“longest dependency chain by edge count,” never “critical path” or a delivery-date prediction.
If a user supplies estimates, the skill explains that weighted critical-path analysis is
unsupported until a later version defines units, bug-versus-edge interpretation, missing-value
policy, and a versioned input/output shape; it does not improvise those semantics.

### Outputs

The bundled `scripts/render.py` deterministically renders Markdown and Mermaid from the versioned
analysis inventory. Node IDs use server identity plus numeric ID, never summaries. Markdown renders
untrusted values only as escaped plain text, disables raw HTML and image/autolink construction, and
chooses a fence longer than any backtick run in fenced data. Mermaid uses quoted grammar strings
with Mermaid-safe `#quot;` entities for quotes and numeric entities for backslashes, controls,
directives, and delimiters. Renderer tests inspect complete
Markdown fences and Mermaid tokens and cover raw HTML, images, autolinks, nested brackets,
`</script>`, triple backticks, directives, quotes, backslashes, and newlines. DOT, HTML, CSV, XLSX,
and PDF are not v1 contracts; the skill may suggest external conversion only after the user chooses
a capable tool and understands that converter's safety boundary.

Every output includes bounds, sanitized scope provenance, server provenance, timestamp,
resolved-node policy, duration assumptions, unknown/boundary counts, and collection command
names. Provenance records server aliases, scope kind, saved-query name, and allowlisted filter
names, never literal credentials, unknown URL parameters, or unsanitized Custom Search URLs.
No workflow command mutates Bugzilla.

## AI-SPEC

The user is a project manager, release lead, or engineer who asks an agent to analyze Bugzilla
dependencies. The trigger is an explicit dependency-analysis request and the inputs are the
selected Bugzilla scopes, traversal policy, and output request. Outputs are bounded graphs,
structural findings, execution groups, and optional artifacts. Allowed sources are deterministic
`bzr` structured output and user-supplied interpretation rules; inaccessible data and unsupported
artifact capabilities must remain explicit. The agent may not mutate Bugzilla, invent missing
nodes or estimates, call structural chains schedules, or traverse beyond stated bounds. Its
fallback is a partial Markdown report with unknown/boundary nodes and limitations. Cost is bounded
for retained graph traversal and request count by the node and depth limits; individual upstream
response size and parsing are not bounded by `max_relationships`. The current fallback may perform
one request per bug. Success means
fixtures deterministically produce the expected node/edge inventory, warnings, and terminology.

## Failure-mode map and evaluation plan

| Failure mode | Severity | Measurement |
|---|---:|---|
| Drops a diamond, cycle-closing edge, or inaccessible node | 4 | Fixture inventory equality |
| Exceeds depth/node/fetch-once bounds | 4 | Command-log and node-count assertions |
| Claims a schedule or weighted critical path without durations | 4 | Forbidden-phrase assertions |
| Executes a Bugzilla mutation | 5 | Allowlisted-command assertion |
| Emits unsafe Markdown/Mermaid content | 4 | Adversarial-string fixture assertions |
| Merges same numeric ID across servers | 4 | Server-qualified identity assertion |
| Treats stale/conflicting or missing source data as fact | 4 | Required limitation/unknown assertions |
| Loops on cycles or a wide graph | 4 | Hard-cap and completion assertions |

Algorithm and collection evaluations are deterministic fixture checks against bundled helpers;
no model judges its own
output. `scripts/collect.py` accepts explicit policy JSON, invokes an injectable command runner,
and atomically emits one `bzr-dependency-collection/v1` JSON document. `scripts/analyze.py`
consumes that document and atomically emits one `bzr-dependency-analysis/v1` JSON inventory.
Neither helper can issue a
Bugzilla mutation; the live collector allowlists only `bug view`, `bug list`, `bug search`, and
`query run`. Static shell checks validate the skill's documented commands and safety rules;
behavioral Python tests compare helper output against fixture oracles byte-for-byte.

| ID | Case | Observable pass traits | Forbidden traits | Gate |
|---|---|---|---|---|
| DA-01 | Branch and diamond happy path | Complete nodes/edges, layers, fan-out | Lost shared edge | block |
| DA-02 | Missing bounds in documented workflow | Skill states 5/200 defaults before its first collection command | Unbounded or unstated collection | block |
| DA-03 | No durations | “longest dependency chain by edge count” | “critical path” as schedule | block |
| DA-04 | Changed adjacency between scope and detail data | Uses one collected snapshot and states provenance | Silently combines conflicting fetches | block |
| DA-05 | Restricted and missing nodes | Unknown nodes remain visible without private detail | Treats them as absent | block |
| DA-06 | Cycle wider/deeper than bounds | Terminates, flags cycle/boundaries/cap | Extra fetch or loop | block |
| DA-07 | Bug summary containing Markdown, Mermaid, and HTML-like payloads | Inert escaped Markdown and Mermaid | Active syntax or directive | block |
| DA-08 | Same bug number on two servers | Two server-qualified nodes | Identity collision | block |
| DA-09 | Resolved blocker | Selected policy is stated and enforced after recording | Silent removal | block |
| DA-10 | Product/milestone/version/query restriction | Cross-scope neighbors are boundary nodes | Out-of-scope traversal | block |
| DA-11 | Reciprocal `blocks`/`depends_on` observations | One canonical edge retaining both observations | Inflated degree/path | block |
| DA-12 | Custom Search URL containing secret-like and unknown parameters | Sanitized scope kind and allowlisted names only | Literal value appears | block |
| DA-13 | Broad scope and oversized query restriction | Root enumeration stops at cap; restriction refuses before pagination | Exhaustive unbounded fetch | block |
| DA-14 | Permuted mixed scopes, roots, responses, adjacency lists, and saved-query order around cap | Canonical deduplicated provenance, explicit bug-ID sort, and byte-identical capped inventory | Order-dependent graph or provenance | block |
| DA-15a | Single-ID API 100/101 or structured bug `not_found` | Each becomes nonfatal sanitized `not_found`, remains an unknown node, and traversal continues | Fatal missing node, fabricated detail, or raw stderr | block |
| DA-15b | Successful preflight then single-ID API 102 inaccessible | Becomes nonfatal sanitized `inaccessible`, remains an unknown node, and traversal continues | Fatal inaccessible node, fabricated detail, or raw stderr | block |
| DA-15c | Other API and global auth/TLS/connection/transport failures | Stops with a partial run-level limitation and fetch-interrupted boundaries | Fabricated unknown or continued global failure | block |
| DA-15d | Failed preflight or API 102 without successful preflight | Command-fatal sanitized API limitation before resource classification | Invalid credentials classified as inaccessible | block |
| DA-16 | Stale, missing, and malformed timestamps at injected UTC time | `true` or `unknown` with warning | Wall-clock-dependent or ignored result | block |
| DA-17 | Fatal error after one successful frontier | Valid `partial` JSON, generic stderr, exit 1; rejected without opt-in | Truncated stream accepted as complete | block |
| DA-18 | Alias success/collapse and alias `not_found` | Canonical numeric node or linked unresolved root, one fetch and slot, byte-exact roots and alias provenance | Duplicate/unlinked node or fetch | block |
| DA-19 | PM findings with unresolved roots, branch, and cycle | Roots preserved; deterministic structural roots/leaves, bottlenecks, unassigned blockers, and execution assumptions | Missing or ambiguous finding | block |
| DA-20 | Zero-node partial restriction result | Exact empty findings and zero-length empty path with partial assumption | Rejection or fabricated path | block |
| DA-21 | Direction isolation at cap | Only selected adjacency admits nodes and consumes cap for `depends_on`, `blocks`, and `both` | Unselected neighbor changes inventory | block |
| DA-22 | Wide adjacency beyond `max_relationships` | Retains/traverses only the deterministic post-parse bounded prefix, prioritizes the selected direction, and reports a lower-bound omission count | Unbounded retained/traversed state or hidden omission | block |

`python3 content/skills/bzr-dependency-analysis/tests/test_analyze.py` runs DA-01 and DA-03
through DA-11 plus DA-16, DA-19, and DA-20. `python3 content/skills/bzr-dependency-analysis/tests/test_collect.py`
runs DA-04, DA-06, DA-13 through DA-15c, and DA-17 with a stubbed command runner and exact command
log assertions, and runs DA-18 for alias canonicalization, DA-21 for direction isolation, and DA-22
for aggregate relationship bounding. `python3 content/skills/bzr-dependency-analysis/tests/test_render.py` runs DA-07
against every deterministic renderer. `content/skills/bzr-dependency-analysis/tests/skill-contract.sh`
runs DA-02 and DA-12 plus static command allowlist and phantom-flag checks. The fixture suite validates the skill
contract and every documented `bzr` invocation. A real
functional Bugzilla demonstration creates a branch, a diamond, a resolved blocker, and
a dependency with a safely hostile summary, then records bounded structured collection and a
textual/Mermaid report. The installed-copy fixture pipeline separately runs the deterministic cycle
fixture through the analyzer and asserts cycle reporting because stock Bugzilla rejects circular
dependency mutations.

## Threat model

### Boundaries and actors

Bugzilla-controlled summaries, field values, errors, and URLs cross from an authenticated or
anonymous Bugzilla user into local artifacts. User-supplied scopes, bounds, and output paths cross
from the local operator into agent-composed commands. Installed skill
instructions cross from the released `bzr` binary into the agent environment. The design adds no
credential handling and widens no server authorization.

Untrusted actors are Bugzilla users able to edit visible fields and operators who may paste a
malformed URL or identifier. The design trusts `bzr` to apply configured authentication, redact
secrets, validate Bugzilla URLs, and emit its documented JSON envelopes.

### Controls

- Commands use argument arrays or shell-safe quoting; Bugzilla text is data and is never evaluated
  as shell, template source, HTML, or Mermaid syntax.
- Positive numeric bounds, including the 9,999 node and relationship ceilings, duplicate-free keys,
  and valid UTF-8 are checked before collection; queue membership enforces fetch-once and the hard
  depth and node caps, while the post-parse relationship cap bounds retained/staged/traversed
  relationship records.
- Server-qualified identities prevent cross-server collisions.
- Only documented read commands are allowlisted; the skill refuses mutation requests during an
  analysis.
- Generated links accept only `http` and `https`; output-path selection remains with the local
  operator and existing artifact tools.
- Unknown/restricted records disclose only an allowlisted structured error type and affected
  identifier, never raw server messages, stderr, or auth/config data.
- Shareable provenance stores only server alias, scope kind, saved-query name, and allowlisted
  parameter names. It drops parameter values from pasted URLs and never reproduces the literal URL
  or command line.

Out of scope are a malicious local agent runtime, compromised `bzr` binary, arbitrary artifact-tool
vulnerabilities, and access control on files after the operator chooses their destination.

## Documentation and verification

The new skill lives at `content/skills/bzr-dependency-analysis/SKILL.md`, with deterministic
fixtures, `scripts/collect.py`, `scripts/analyze.py`, `scripts/render.py`, Python behavioral tests, and a shell contract
check under the same directory. The embedded-name unit test and
functional installer fixture include it. The existing `18c-skills-install.sh` phase verifies every
dependency-analysis payload path after installation and runs one fixture-based
collect→analyze→render pipeline from the installed destination. A new
`18d-dependency-analysis.sh` functional phase creates live branches, a diamond, resolved
blocker, and hostile summary. It selects the freshly installed
`$SKILLS_PROJECT/.agents/skills/bzr-dependency-analysis` root, asserts the real paths of collector,
analyzer, renderer, and fixtures remain beneath it, and runs the complete installed
collect→analyze→render chain through the real `bzr` binary. It asserts server-qualified edges,
resolved handling, caps, and inert rendered text without resolving any stage from the checkout.
The phase also analyzes the installed deterministic cycle fixture and asserts its cyclic component;
it does not attempt an impossible live circular mutation.
Using explicit credentialless REST and XML-RPC server profiles plus the restricted fixture created
by the existing functional phases, 18d collects real nonexistent starting IDs in both API modes and
a real inaccessible starting ID beside visible roots in both modes. It asserts `not_found` and
`inaccessible` unknown nodes, continued collection, valid partial-safe output, and absence of raw
server messages. Deliberately rejected credentials in both modes additionally prove that preflight
is sanitized, command-fatal, and completes before any root is admitted or resource read begins.
It also runs the installed pipeline with a one-relationship budget and asserts the bounded admitted
prefix, `relationship_cap`, and lower-bound omission metadata.
Phase 18d gives the live graph a stable, unique whiteboard marker so a later read-only demonstration
can discover the already-provisioned IDs. Fixture creation belongs only to the disposable functional
harness; neither the installed skill nor the displayed analysis workflow performs a mutation.
`tools/record-demo.sh` gains a named dependency-analysis mode that discovers that marked graph with
read commands and records it as
`docs/assets/bzr-dependency-analysis-demo.cast` and
`docs/assets/bzr-dependency-analysis-demo.gif` without changing the existing README demo default.
If the marked fixture is absent, the recorder stops with an instruction to run the functional setup;
it never creates replacement bugs or dependency links.
`docs/bzr-dependency-analysis.md` embeds the GIF and links the cast. The shell contract check
asserts both references so a recorded but unpublished demonstration fails verification.

Verification uses focused fixture checks while iterating, then `make lint`, `make test`, and
`make functional-test-all`. The functional demo is exercised against the same real Bugzilla
container harness used by existing functional phases.
