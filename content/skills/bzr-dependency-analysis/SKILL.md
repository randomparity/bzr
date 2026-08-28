---
name: bzr-dependency-analysis
description: Use when collecting and analyzing a bounded, deterministic Bugzilla dependency inventory with bzr.
---

# Collect bounded dependency evidence

Use the bundled collector when the user asks for dependency evidence. It calls
only released read commands (`bug view`, `bug list`, `bug search`, and
`query run`), fetches each admitted bug at most once, and writes one versioned
JSON document atomically.

Before collection, choose positive depth and node bounds and state them to the
user. The current fallback may make one `bug view` request per admitted bug.
It never mutates Bugzilla.

Create a policy JSON file, then run:

```sh
python3 "$SKILL_ROOT/scripts/collect.py" \
  --policy "$POLICY" \
  --output "$COLLECTION"
```

For a reproducible replay, add an exact UTC timestamp:

```sh
python3 "$SKILL_ROOT/scripts/collect.py" \
  --policy "$POLICY" \
  --output "$COLLECTION" \
  --analysis-timestamp 2026-08-28T12:00:00Z
```

The policy has this exact top-level shape:

```json
{
  "bounds": {"max_depth": 5, "max_nodes": 200},
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

`direction` is `depends_on`, `blocks`, or `both`. `resolved_mode` is
`include-traverse` or `include-no-traverse`. Declare every server alias used by
a scope in `servers`.

Supported scope objects are:

- `{"kind":"bug-ids","server":"NAME","ids":[1,2]}`
- `{"kind":"alias","server":"NAME","alias":"ALIAS"}` (at most one per server)
- `{"kind":"saved-query","server":"NAME","name":"QUERY"}`
- `{"kind":"custom-search","server":"NAME","url":"HTTPS_URL","parameter_names":["product"]}`
- `{"kind":"product","server":"NAME","value":"PRODUCT"}`
- `{"kind":"milestone","server":"NAME","value":"MILESTONE"}`
- `{"kind":"version","server":"NAME","value":"VERSION"}`

For a Custom Search URL, `parameter_names` must exactly list the recognized,
allowlisted parameter names present in the URL; values and unknown parameters
are never copied into output provenance.

`restriction` may be null or one saved-query, product, milestone, or version
object using the corresponding scope fields. An oversized saved-query
restriction stops before traversal and produces a valid partial inventory.

The output schema is `bzr-dependency-collection/v1`. Bugzilla API codes
100/101 remain visible as `not_found` nodes and code 102 remains visible as an
`inaccessible` node. Other API, authentication, TLS, HTTP, connection, and
transport failures stop the run, preserve a sanitized partial JSON file, print
one generic failure class, and exit 1. Never copy raw server error text into a
shareable report.

Analyze a complete collection with:

```sh
python3 "$SKILL_ROOT/scripts/analyze.py" \
  --input "$COLLECTION" \
  --output "$ANALYSIS"
```

The analyzer writes canonical `bzr-dependency-analysis/v1` JSON with
server-qualified nodes, canonical scheduling edges, cycle components,
topological layers, the longest dependency chain by edge count, staleness, and
structural project-management findings. It does not accept durations or make
delivery-date claims. A partial collection is rejected unless the operator
explicitly adds `--allow-partial`; partial evidence and boundaries remain
visible in the analysis.
