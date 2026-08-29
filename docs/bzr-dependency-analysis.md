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
once with a released read-only command before retrieving resources. A minimal policy for one
configured server and bug-ID root is:

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
  "stale_after_days": 14
}
```

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
contain additional relationships.

## Interpret the result structurally

Version 1 reports structural roots and leaves, fan-out bottlenecks, stale and unassigned blockers,
strongly connected components, component layers, and the longest dependency chain by edge count.
These are properties of the collected graph, not a delivery schedule. The format has no duration
model, so it does not support weighted critical-path analysis or delivery-date prediction. Members
of a cyclic component also have no total execution order.

Collection is bounded but can make one `bug view` request per admitted bug. Matching numeric IDs on
different servers remain distinct. Markdown and Mermaid are the only supported render formats;
DOT, HTML, CSV, XLSX, and PDF are not version 1 outputs.

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
