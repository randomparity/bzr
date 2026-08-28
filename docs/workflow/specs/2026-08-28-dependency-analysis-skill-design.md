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
- roots from bug IDs or aliases, a named saved query, a Custom Search URL, a target
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
not a node. A query used as a membership restriction is counted first. If its count exceeds the
node cap, the skill refuses exhaustive materialization and asks the user to narrow the query or
raise the cap; otherwise it paginates the complete membership.

The bundled `scripts/collect.py` owns collection and invokes `bzr` through a command-runner
boundary. Tests substitute a recorded runner; live use invokes the configured binary without a
shell. The collector maintains `queued`, `fetched`, and `nodes` sets keyed by `(server, bug-id)`.
The current fallback fetches each ID separately with `bzr --json bug view ID --fields
id,summary,status,resolution,assigned_to,last_change_time,blocks,depends_on`, because only a
command-level failure exposes the versioned structured error envelope. It therefore makes at most
one command per admitted bug. The field list additionally
requests `product`, `target_milestone`, or `version` when the corresponding restriction is
active. Query restrictions use the bounded, fully paginated server-qualified membership set
described above. Returned adjacency lists add
edges even when the target is already fetched. Classified per-ID failures create unknown nodes
carrying only the server, requested identifier, and structured error type. A failed or missing node is never
silently removed.

Depth is the minimum hop distance from any root. A newly discovered node beyond the maximum
depth is recorded as a boundary node but not fetched, provided it fits inside the maximum
distinct-node count. The node cap covers every emitted known, unknown, boundary, or truncated
node. Once exhausted, no more node identities are emitted; the graph metadata records an
aggregate omitted-reference count and collection stops. Collection order is canonical: server
aliases lexically, caller-supplied roots in original order after stable deduplication,
breadth-first depth, and numeric bug ID within each frontier. Returned nodes and adjacency IDs are
sorted before admission, and output records use the same order. Scope restrictions
are evaluated from fetched fields or the initial scope membership; nodes outside the
restriction remain visible boundary nodes and are not traversed.

Only structured per-ID not-found or permission failures become unknown nodes. A single-ID failure
is classified from the versioned structured error envelope using the same error types. Global
authentication, TLS, transport, schema-version, malformed-output, and unclassified failures stop
collection and preserve a partial inventory with a run-level limitation. Batch failures are never
copied onto every requested node. Shareable records retain only error type and affected identifier,
not raw stderr or server messages.

The collector atomically writes one `bzr-dependency-collection/v1` JSON document. It contains
`status` (`complete` or
`partial`), nodes, observed edges, bounds, sanitized provenance, omitted counts, and limitations.
A complete run exits 0. A fatal run still writes a valid `partial` document, writes one generic
error-type line to stderr, and exits 1. A consumer rejects `partial` input unless the operator
passes `--allow-partial`, so no prefix stream can be mistaken for completion. `analyze.py` reads
only that schema and atomically writes one `bzr-dependency-analysis/v1` JSON document, preserving
status and limitations. Both readers reject truncated JSON and unknown schema versions.

Resolved-node policy is applied after recording the node and incoming edge. Status meanings
are installation-specific, so the skill asks which statuses count as resolved or states the
chosen assumption.

### Graph model and analysis

Each node contains server identity, bug ID or unresolved input, summary, status, resolution,
assignee, last-change time, fetch state (`known`, `unknown`, `boundary`, or `truncated`), and
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
summaries. Markdown chooses a fence longer than any backtick run in data. Mermaid/DOT use quoted
grammar strings with backslash, quote, newline, directive, and delimiter encoding. HTML inserts
escaped text nodes and safe `http`/`https` links; it never interpolates inventory into script,
style, event-handler, or raw-HTML contexts. CSV neutralizes a cell when, after leading spaces,
tabs, carriage returns, or newlines, its first character is `=`, `+`, `-`, or `@`; neutralization
prefixes an apostrophe before RFC 4180 serialization. The helper emits sanitized CSV; an optional spreadsheet
capability may convert it to XLSX without reintroducing raw Bugzilla values. XLSX is offered only
when that capability exists; otherwise the skill gives CSV. Renderer tests parse final HTML and
CSV, inspect complete Markdown fences and Mermaid/DOT tokens, and cover `</script>`, triple
backticks, directives, quotes, backslashes, and whitespace-prefixed spreadsheet formulas.

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
checked-in JSON manifest. The manifest names each prompt fixture, objective predicates over tool
events and final text, a 120-second case timeout, and zero retries. Required environment inputs are
`CODEX_BIN` and `CODEX_MODEL`; the harness records the resolved CLI version, model, and reasoning
setting. It runs from a fresh temporary project with network disabled, write access limited to that
project, and only the recorded fake `bzr` plus artifact helpers available. It consumes the Codex
CLI's checked-in supported JSONL event schema and fails on unknown events. A missing runner, timeout,
tool outside the allowlist, unmatched predicate, or skipped case exits nonzero. `make
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

`python3 content/skills/bzr-dependency-analysis/tests/test_analyze.py` runs DA-01 and DA-03
through DA-11 plus DA-16. `python3 content/skills/bzr-dependency-analysis/tests/test_collect.py`
runs DA-04, DA-06, and DA-13 through DA-15 and DA-17 with a stubbed command runner and exact command
log assertions. `python3 content/skills/bzr-dependency-analysis/tests/test_render.py` runs DA-07
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
