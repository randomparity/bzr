# Dependency analysis with bzr

The bundled `bzr-dependency-analysis` skill collects a bounded Bugzilla dependency snapshot,
analyzes its structure, and renders local Markdown or Mermaid artifacts. The workflow is read-only:
it uses `bug view`, `bug list`, `bug search`, and `query run`, and does not change Bugzilla.

![Dependency analysis demo](assets/bzr-dependency-analysis-demo.gif)

[Download the asciinema cast](assets/bzr-dependency-analysis-demo.cast) to replay the terminal
session with asciinema.

## Install the skill

Install the bundled payload into a Codex project and resolve every helper from that installed copy:

```sh
PROJECT=$(pwd -P)
bzr skills install --agent codex --project "$PROJECT"
SKILL_ROOT="$PROJECT/.agents/skills/bzr-dependency-analysis"
```

The installer creates `$SKILL_ROOT/scripts/collect.py`, `$SKILL_ROOT/scripts/analyze.py`, and
`$SKILL_ROOT/scripts/render.py`. Keep policy, collection, analysis, and rendered files outside the
installed skill directory so a later installation can replace the managed payload safely.

## Collect, analyze, and render

Choose positive depth and node bounds before retrieval. The optional positive relationship bound
defaults to the node bound and may be raised independently. Node and relationship bounds may not
exceed 9,999; the node ceiling preserves the four-digit `cNNNN` namespace. The collector rejects a
higher bound, duplicate JSON keys, and invalid UTF-8 before invoking `bzr`. If no bounds are
supplied, the skill proposes depth 5, 200 nodes, and 200 relationships. It preflights each server
once with a released read-only command before retrieving resources. `servers` must exactly equal
the server aliases referenced by scopes and the optional restriction, so unused declarations are
rejected before preflight. A minimal policy for one configured server and bug-ID root is:

`max_relationships` applies after each released `bzr` command has returned and its complete JSON
response has been parsed. It bounds selected relationship records retained, staged, and traversed.
A one-direction run may additionally retain the same number of optional reciprocal candidates,
which never drive traversal. Neither limit bounds the upstream Bugzilla/`bzr` response size,
JSON-decoding work, or peak parse memory. Upstream bounded retrieval is tracked separately in
issue #573.

```json
{
  "bounds": {"max_depth": 5, "max_nodes": 200, "max_relationships": 200},
  "bzr": "bzr",
  "direction": "both",
  "resolved_mode": "include-no-traverse",
  "resolved_statuses": ["RESOLVED"],
  "restriction": null,
  "scopes": [{"ids": [1200], "kind": "bug-ids", "server": "primary"}],
  "servers": ["primary"],
  "stale_after_days": 14,
  "unassigned_assignees": {}
}
```

If a server assigns new bugs to a placeholder account, set `unassigned_assignees` to a per-server
map of sorted exact login lists, such as `{"primary":["nobody@bugs.example"]}`. It defaults to an
empty map. Use the `assigned_to` string returned through that same server alias. Exact equality is
required; prefixes and substrings are not inferred as unassigned.

Custom Search URLs may not contain userinfo or the case-insensitive credential query names
`bugzilla_api_key`, `token`, or `api_key`. The collector rejects them with a generic policy error
before creating a child command. Child `bzr` commands otherwise inherit the caller's environment,
including configuration and credential sources, while forcing `RUST_LOG=off` so tracing cannot
corrupt structured failure output.

Set the artifact paths, then run the installed helpers exactly as shown:

```sh
POLICY="$PROJECT/dependency-policy.json"
COLLECTION="$PROJECT/dependency-collection.json"
ANALYSIS="$PROJECT/dependency-analysis.json"
REPORT="$PROJECT/dependency-report.md"
DIAGRAM="$PROJECT/dependency-graph.mmd"

python3 "$SKILL_ROOT/scripts/collect.py" \
  --policy "$POLICY" \
  --output "$COLLECTION"
python3 "$SKILL_ROOT/scripts/analyze.py" \
  --input "$COLLECTION" \
  --output "$ANALYSIS"
python3 "$SKILL_ROOT/scripts/render.py" \
  --input "$ANALYSIS" \
  --format markdown \
  --output "$REPORT"
python3 "$SKILL_ROOT/scripts/render.py" \
  --input "$ANALYSIS" \
  --format mermaid \
  --output "$DIAGRAM"
```

The analyzer rejects a partial collection unless the operator explicitly chooses
`--allow-partial`. Unknown, inaccessible, missing, depth-limited, scope-limited, relationship-capped,
and interrupted nodes remain visible so an incomplete snapshot cannot be mistaken for a complete
graph. A relationship-cap omission count is explicitly a lower bound because unfetched nodes may
contain additional relationships. When the cap closes selected staging, the collector still
fetches every identity already admitted to the current frontier in canonical order, without
staging further selected adjacency, before stopping.

## Interpret the result structurally

Version 1 reports structural roots and leaves, fan-out bottlenecks, stale and unassigned blockers,
strongly connected components, component layers, and the longest dependency chain by edge count.
Both text formats state the traversal direction and exact unassigned-assignee policy used to derive
those findings. For one-direction collection, only selected evidence consumes
`max_relationships`; bounded optional reciprocal evidence cannot preempt selected traversal.
Selected adjacency IDs are deduplicated and sorted numerically before cap admission. For `both`,
the canonical order is `blocks` then `depends_on`, with numeric order inside each field.
Resolved nodes under `include-no-traverse` keep their incoming edge, but their outgoing selected
adjacency consumes no relationship budget and drives no discovery. Reciprocal observations only
corroborate an edge grounded by selected-direction evidence.
Product, milestone, and version restrictions are applied to each fetched node before either
adjacency field is staged. An excluded node stays visible as a scope boundary but contributes no
observation or relationship-budget charge.
These are properties of the collected graph, not a delivery schedule. The format has no duration
model, so it does not support weighted critical-path analysis or delivery-date prediction. Members
of a cyclic component also have no total execution order.

The retained graph and traversal are bounded, and collection can make one `bug view` request per
admitted bug. Matching numeric IDs on different servers remain distinct. Markdown and Mermaid are
the only supported render formats; DOT, HTML, CSV, XLSX, and PDF are not version 1 outputs.

## Regenerate the demo

The recording consumes the stable fixture provisioned by functional phase 18d. It discovers that
fixture with a read-only `bug list`, selects the highest matching root ID on warm reruns, installs
the skill into a temporary project, and records the installed collect/analyze/render pipeline. It
never creates or updates a Bugzilla resource.

```sh
cargo build --release
make functional-test
tools/record-demo.sh dependency-analysis
```

If the marked fixture is absent, the recorder stops and asks for the functional setup instead of
seeding replacement data.
