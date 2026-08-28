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
their failures. The skill must detect and prefer that primitive only after it is released;
it must not describe an unreleased command or flag.

## Workflow

### Inputs and defaults

Before retrieval, the skill resolves:

- one configured server per input scope; multi-server analysis is a union of separately
  fetched server-qualified graphs;
- roots from bug IDs, one alias per server, a named saved query, a Custom Search URL, a target
  milestone, or a version;
- direction: `depends_on`, `blocks`, or both;
- maximum depth and maximum distinct nodes, both required and positive;
- whether resolved nodes are included and traversed, included but not traversed, or omitted
  after being recorded;
- optional product, milestone, or query-membership restriction;
- stale threshold, analysis timestamp, and requested output formats.

If the user omits bounds, the skill proposes conservative defaults of depth 5 and 200 nodes
and states them before fetching. It never starts an unbounded walk. Refresh is an explicit
new collection run; within one run a server-qualified bug identity is fetched at most once.

### Collection

Scope commands use versioned JSON and request only fields needed by the selected analysis.
Named queries use `bzr query run`; Custom Search URLs use `bzr bug search --from-url`;
milestone/version scopes use `bzr bug list`. Starting scopes request at most `node_cap + 1`
rows without `--paginate`; the extra row detects truncation and becomes aggregate scope metadata,
not a node. A query used as a membership restriction is enumerated manually in pages whose total
requested rows never exceeds `node_cap + 1`; the collector stops on the extra row and asks the user
to narrow the query or raise the cap. It never uses `--count` as an upper-bound proof because the
server may cap that result. If a server returns fewer rows than requested, the collector advances
the offset and probes once more; an empty page proves completion, while the cumulative extra row
proves oversize. This bounds both membership storage and requests by the chosen cap.

The bundled `scripts/collect.py` owns collection and invokes `bzr` through a command-runner
boundary. Tests substitute a recorded runner; live use invokes the configured binary without a
shell. The collector maintains `queued`, `fetched`, and `nodes` sets keyed by `(server, bug-id)`.
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
active. Query restrictions use the bounded, fully paginated server-qualified membership set
described above. Returned adjacency lists add an observation when both endpoint nodes are admitted,
even when the target is already fetched. Classified per-ID failures create unknown nodes
carrying only the server, requested identifier, and structured error type. A failed or missing node is never
silently removed.

Depth is the minimum hop distance from any root. A newly discovered node beyond the maximum
depth is recorded as a boundary node but not fetched, provided it fits inside the maximum
distinct-node count. The node cap covers every emitted known, unknown, or boundary
node. Once exhausted, no more node identities are emitted; the graph metadata records an
aggregate `cap_reached: true` and `omitted_discovered_identities` count. The count is the number of
distinct server-qualified identities rejected while scanning all adjacency lists already returned
for the current frontier; duplicate observations count once. No further frontier is fetched, so
deeper undiscovered identities are neither named nor counted. Collection order is canonical: server
aliases lexically, caller-supplied roots in original order after stable deduplication,
breadth-first depth, and numeric bug ID within each frontier. Returned nodes and adjacency IDs are
sorted before admission, and output records use the same order. Scope restrictions
are evaluated from fetched fields or the initial scope membership; nodes outside the
restriction remain visible boundary nodes and are not traversed.

Reaching the cap closes admission but does not interrupt the current frontier. Every already
admitted identity in that frontier is fetched in canonical order. Their returned adjacency lists
record observations only between admitted endpoints and contribute rejected identities to the
deduplicated omitted count. Collection then stops before fetching a next frontier.

Only a structured `not_found` failure becomes an unknown node under the released fallback. Generic
`http` and `api` failures, including permission failures that lack a stable published code, are
fatal until the follow-up primitive supplies a versioned per-ID classification. Global
authentication, TLS, transport, schema-version, malformed-output, and unclassified failures stop
collection and preserve a partial inventory with a run-level limitation. Batch failures are never
copied onto every requested node. Shareable records retain only error type and affected identifier,
not raw stderr or server messages.
On fatal stop, the identity whose command failed and every admitted but unfetched identity remain
`boundary` with `boundary_reason: fetch_interrupted`; the generic failure exists only as a run-level
limitation. A successful fetch changes the node to `known`; `not_found` changes it to `unknown`.
For an alias lookup, structured `not_found` retains the consumed slot and becomes a nonnumeric
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
  "bounds": {"max_depth": 5, "max_nodes": 200},
  "cap": {
    "graph_cap_reached": false,
    "omitted_discovered_identities": 0,
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
  "provenance": [{"scope_kind": "bug-ids", "server": "primary"}],
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
alias in `requested`. Roots sort by server, ID (null last), then requested; provenance retains canonical scope order;
limitations are sorted stable codes. Serialization is UTF-8, sorted object keys, two-space indent,
and one trailing newline. Starting-scope overflow sets `scope_truncated: true`, status `partial`, and
limitation `scope-node-cap`; the first `max_nodes` roots remain. Traversal exhaustion sets
`graph_cap_reached: true`, status `partial`, the deduplicated rejected-identity count, and limitation
`graph-node-cap`. An oversized membership restriction emits no nodes, status `partial`, limitation
`restriction-node-cap`, and `scope_truncated: true`; traversal does not start. Fatal collection adds
its stable run-level limitation code and status `partial` without changing already recorded nodes.
Every observation endpoint must occur in `nodes`. When cap admission rejects a newly discovered
identity, all observations to that identity are omitted and the identity contributes exactly once
to `omitted_discovered_identities`.
Observation endpoint equality is `(server, id)`; observations can originate only from numeric
adjacency fields, so neither endpoint has a null ID.

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

Staleness is an observation, not an ordering weight. The user supplies a positive age threshold;
the collector records an injected analysis timestamp in UTC and parses `last_change_time` as an
absolute timestamp. A known unresolved node is stale when its age is at least the threshold.
Missing or invalid timestamps produce `stale: unknown` plus a data-quality warning.

A time-based critical path is forbidden unless duration data exists and the user explicitly
chooses its units and interpretation. Weighted analysis names the duration field, missing-value
policy, and whether estimates apply to bugs or edges. Otherwise output says “longest dependency
chain by edge count,” never “critical path” or a delivery-date prediction.

### Outputs

The bundled `scripts/render.py` deterministically renders Markdown, Mermaid, DOT, HTML, and CSV
from the versioned analysis inventory. Node IDs use server identity plus numeric ID, never
summaries. Markdown renders untrusted values only as escaped plain text, disables raw HTML and
image/autolink construction, validates every emitted destination as `http` or `https`, and chooses
a fence longer than any backtick run in fenced data. Mermaid/DOT use quoted
grammar strings with backslash, quote, newline, directive, and delimiter encoding. HTML inserts
escaped text nodes and safe `http`/`https` links; it never interpolates inventory into script,
style, event-handler, or raw-HTML contexts. CSV neutralizes a cell when, after leading spaces,
tabs, carriage returns, or newlines, its first character is `=`, `+`, `-`, or `@`; neutralization
prefixes an apostrophe before RFC 4180 serialization. The helper emits sanitized CSV; an optional spreadsheet
capability may convert it to XLSX without reintroducing raw Bugzilla values. XLSX is offered only
when that capability exists; otherwise the skill gives CSV. Renderer tests parse final HTML and
CSV, inspect complete Markdown fences and Mermaid/DOT tokens, and cover raw HTML, images, autolinks,
unsafe/control-character URI schemes, nested brackets, `</script>`, triple backticks, directives,
quotes, backslashes, and whitespace-prefixed spreadsheet formulas.

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
by the node and depth limits; the current fallback may perform one request per bug. Success means
fixtures deterministically produce the expected node/edge inventory, warnings, and terminology.

## Failure-mode map and evaluation plan

| Failure mode | Severity | Measurement |
|---|---:|---|
| Drops a diamond, cycle-closing edge, or inaccessible node | 4 | Fixture inventory equality |
| Exceeds depth/node/fetch-once bounds | 4 | Command-log and node-count assertions |
| Claims a schedule or weighted critical path without durations | 4 | Forbidden-phrase assertions |
| Executes a Bugzilla mutation | 5 | Allowlisted-command assertion |
| Emits unsafe Mermaid/HTML/spreadsheet content | 4 | Adversarial-string fixture assertions |
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

Agent-owned behavior has a separate executable harness at `tests/run-agent-evals.sh`, driven by a
checked-in JSON manifest. Version 1 supports exactly `codex-cli 0.150.1`; a different version fails
before cases run. Each case invokes:

```text
$CODEX_BIN exec --json --ephemeral --ignore-user-config --strict-config \
  --model $CODEX_MODEL --sandbox workspace-write \
  -c model_reasoning_effort="$CODEX_REASONING" \
  --cd $CASE_DIR --output-schema tests/agent-eval-response.schema.json -
```

The manifest names prompt fixtures, exact predicates, a 120-second timeout, and zero retries.
`CODEX_MODEL` and `CODEX_REASONING` are required and recorded with `codex --version`. The harness
validates stdout against `tests/codex-events-0.150.1.schema.json`; accepted records are
`thread.started`, `turn.started`, `item.started`, `item.completed`, `turn.completed`, and `error`.
Command predicates read `item.command`, `item.aggregated_output`, and `item.exit_code`; final-text
predicates read completed `agent_message.text`. Unknown records or missing fields fail.

Before cases, the harness resolves the repository and system temporary roots and fails if the
repository is inside the temporary root. It creates `$CASE_DIR` with `mktemp -d`, passes no
`--add-dir`, and creates a writable sentinel directory at
`$REPO/target/dependency-skill-eval/sentinel-$PID`; tracked `.gitignore` ignores `target/` in every
clean checkout. The sentinel contains one baseline file and the harness records a sorted SHA-256
manifest of its path, bytes, and mode. Under the exact invocation, the only requested project
writable root is `$CASE_DIR`; Codex-managed temporary paths remain under the distinct system
temporary root, so the repository sentinel is outside both known writable regions.

The harness binds a TCP listener to `127.0.0.1:0`, reads the assigned port, and injects both the
absolute sentinel path and `127.0.0.1:<port>` into the self-test prompt. Before invoking Codex, the
harness proves it can create/remove `$SENTINEL/probe` and connect once to that address, then clears
the connection counter and restores the baseline manifest. The prompt requires exactly
`printf sandbox-probe > "$SENTINEL/probe"` and a Python `socket.create_connection` to the injected
address. Both command events must exit nonzero and contain an OS/sandbox denial marker
(`Operation not permitted`, `Permission denied`, or `network access denied`); the sentinel directory
manifest must remain byte-identical and the listener must record zero connections. An incidental DNS, remote-host,
or naturally unwritable-path failure cannot satisfy the probe. The case directory
contains only the installed skill, recorded fake `bzr`, renderer helpers, and fixtures, and `PATH`
names only those tools plus required system utilities. A missing runner, failed confinement test,
timeout, tool outside the allowlist, unmatched predicate, or skipped case exits nonzero. `make
dependency-skill-eval` is the release-blocking entry point; release instructions require it after a
skill change. A human spot-checks captured outputs as additional evidence, not as the pass oracle.

| ID | Case | Observable pass traits | Forbidden traits | Gate |
|---|---|---|---|---|
| DA-01 | Branch and diamond happy path | Complete nodes/edges, layers, fan-out | Lost shared edge | block |
| DA-02 | Ambiguous missing bounds | Clarifies or states 5/200 defaults | Starts traversal first | block |
| DA-03 | No durations | “longest dependency chain by edge count” | “critical path” as schedule | block |
| DA-04 | Changed adjacency between scope and detail data | Uses one collected snapshot and states provenance | Silently combines conflicting fetches | block |
| DA-05 | Restricted and missing nodes | Unknown nodes remain visible without private detail | Treats them as absent | block |
| DA-06 | Cycle wider/deeper than bounds | Terminates, flags cycle/boundaries/cap | Extra fetch or loop | block |
| DA-07 | Bug summary containing Mermaid, HTML, and formula payloads | Escaped graph/HTML and inert sheet cells | Active syntax or formula | block |
| DA-08 | Same bug number on two servers | Two server-qualified nodes | Identity collision | block |
| DA-09 | Resolved blocker | Selected policy is stated and enforced after recording | Silent removal | block |
| DA-10 | Product/milestone/version/query restriction | Cross-scope neighbors are boundary nodes | Out-of-scope traversal | block |
| DA-11 | Reciprocal `blocks`/`depends_on` observations | One canonical edge retaining both observations | Inflated degree/path | block |
| DA-12 | Custom Search URL containing secret-like and unknown parameters | Sanitized scope kind and allowlisted names only | Literal value appears | block |
| DA-13 | Broad scope and oversized query restriction | Root enumeration stops at cap; restriction refuses before pagination | Exhaustive unbounded fetch | block |
| DA-14 | Permuted roots, responses, and adjacency lists | Byte-identical capped inventory | Order-dependent graph | block |
| DA-15 | Single-ID unknown and global auth/TLS/transport failures | Unknown only for classified per-ID error; global failure stops partial run | Fabricated unknowns or raw stderr | block |
| DA-16 | Stale, missing, and malformed timestamps at injected UTC time | `true` or `unknown` with warning | Wall-clock-dependent or ignored result | block |
| DA-17 | Fatal error after one successful frontier | Valid `partial` JSON, generic stderr, exit 1; rejected without opt-in | Truncated stream accepted as complete | block |
| DA-18 | Alias success/collapse and alias `not_found` | Canonical numeric node or linked unresolved root, one fetch and slot, byte-exact roots and alias provenance | Duplicate/unlinked node or fetch | block |

`python3 content/skills/bzr-dependency-analysis/tests/test_analyze.py` runs DA-01 and DA-03
through DA-11 plus DA-16. `python3 content/skills/bzr-dependency-analysis/tests/test_collect.py`
runs DA-04, DA-06, and DA-13 through DA-15 and DA-17 with a stubbed command runner and exact command
log assertions, and runs DA-18 for alias canonicalization. `python3 content/skills/bzr-dependency-analysis/tests/test_render.py` runs DA-07
against every deterministic renderer. `tests/run-agent-evals.sh` runs DA-02 and the agent-owned
parts of DA-03, DA-06, and DA-12. `content/skills/bzr-dependency-analysis/tests/skill-contract.sh` runs DA-12 plus
static command allowlist and phantom-flag checks. The fixture suite validates the skill
contract and every documented `bzr` invocation. A real
functional Bugzilla demonstration creates a branch, a diamond, a cycle, a resolved blocker, and
a dependency with a safely hostile summary, then records bounded structured collection and a
textual/Mermaid report.

## Threat model

### Boundaries and actors

Bugzilla-controlled summaries, field values, errors, and URLs cross from an authenticated or
anonymous Bugzilla user into local artifacts. User-supplied scopes, bounds, output paths, and
duration semantics cross from the local operator into agent-composed commands. Installed skill
instructions cross from the released `bzr` binary into the agent environment. The design adds no
credential handling and widens no server authorization.

Untrusted actors are Bugzilla users able to edit visible fields and operators who may paste a
malformed URL or identifier. The design trusts `bzr` to apply configured authentication, redact
secrets, validate Bugzilla URLs, and emit its documented JSON envelopes.

### Controls

- Commands use argument arrays or shell-safe quoting; Bugzilla text is data and is never evaluated
  as shell, template source, HTML, Mermaid syntax, DOT syntax, or spreadsheet formulas.
- Positive numeric bounds are checked before collection; queue membership enforces fetch-once and
  both hard caps.
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
functional installer fixture include it. `tools/record-demo.sh` gains a named dependency-analysis
mode that seeds and records the graph without changing the existing README demo default.

Verification uses focused fixture checks while iterating, then `make lint`, `make test`, and
`make functional-test-all`. The functional demo is exercised against the same real Bugzilla
container harness used by existing functional phases.
