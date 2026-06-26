# `bzr bug links` — relationship-graph command (issue #453)

Status: Draft
Issue: #453
Related ADR: [0006](../../adr/0006-bug-links-isolated-fetch.md)

## Problem

To build a bug's dependency graph today, an agent must `bug view <id> --json`,
extract the relationship fields, then issue one `bug view` per related bug. That
is N+1 round trips for a common triage task, re-implemented ad hoc with no
cycle bound and no breadth limit.

## Goal

A single read-only command that returns a bug's related bugs as a flat list of
records — optionally walking the graph with a bounded, cycle-safe BFS — so an
agent can build the graph in one pipeline:

```
bzr bug links 12345 --recursive --depth 2 --output ndjson | jq -c '{id, relation, status, summary}'
```

## Surface

```
bzr bug links <id>                          # one hop, all relations
bzr bug links <id> --recursive --depth 2    # bounded BFS
bzr bug links <id> --relation depends_on    # filter to one relation
bzr bug links <id> --json
bzr --output ndjson bug links <id>
```

`<id>` is a single numeric bug id (aliases are out of scope; relationship fields
are numeric ids).

### Flags

| Flag | Type | Meaning |
|------|------|---------|
| `--recursive` | bool | Enable BFS beyond the direct neighbors. |
| `--depth N` | u32 | Maximum hop distance from the root. clap `requires("recursive")`. Default `1`. Range `1..=10` enforced by clap's value parser; `0` or `>10` is a clap usage error. |
| `--relation <type>` | enum | Restrict traversal **and** output to one relationship type. One of the six relation names below, parsed via `FromStr`; an unknown value is a clap usage error. |

These three are validated at **parse time by clap** (range, `requires`, and
`FromStr`), so an invalid value is a clap usage error (exit code `2`, clap's
default), matching how the existing `--order`/`SortDirection` flag is handled —
not bzr's runtime `InputValidation` (exit `7`). `requires("recursive")` fires
only when `--depth` is given explicitly; the defaulted value does not force
`--recursive`.

### Bounds

Depth alone does not bound work: a single bug can have hundreds of related
bugs, so a level's frontier can be wide. Two additional bounds keep traversal
and request sizes finite (the issue calls out the lack of a *breadth* bound as
the core gap):

- **Per-request id chunking.** Each level fetches its frontier in chunks of at
  most `LINKS_ID_CHUNK = 100` ids per `GET`, so no single URL packs an unbounded
  `id=` list past server/proxy URL-length limits. Round-trips per level are
  `ceil(level_width / 100)`.
- **Total-node cap.** Traversal visits at most `LINKS_MAX_NODES = 1000` distinct
  related bugs. On reaching the cap, traversal stops, the records collected so
  far are emitted normally, and a one-line notice is written to **stderr**
  (`stopped at LINKS_MAX_NODES (1000) related bugs; results may be incomplete`)
  so a truncated graph is never silently mistaken for a complete one. stdout
  output stays valid (`[]`/ndjson/table unaffected in shape).

Without `--recursive`, depth is fixed at `1` (direct neighbors only). `--depth`
given without `--recursive` is a clap usage error (`requires`), naming
`--recursive` in the message.

This command is read-only: `--dry-run` is not applicable, it takes no
confirmation, and it works against public servers without an API key
(anonymous capability).

## Relationship model

Six relationship types, three inverse pairs. bzr's global `Bug` type today only
models `depends_on`, `blocks`, `dupe_of`; the other three are dropped at
deserialization. Per ADR-0006 this command uses an **isolated fetch type** that
requests exactly the relationship fields, so the global `Bug` type and
`BUG_DEFAULT_FIELDS` are untouched and no other command's request shape changes.

| relation | wire field | shape | direction | meaning (for node N) |
|----------|-----------|-------|-----------|----------------------|
| `depends_on` | `depends_on` | array | `out` | N depends on the related bug |
| `blocks` | `blocks` | array | `in` | the related bug depends on N |
| `dupe_of` | `dupe_of` | scalar | `out` | N is a duplicate of the related bug |
| `duplicates` | `duplicates` | array | `in` | the related bug is a duplicate of N |
| `regressed_by` | `regressed_by` | array | `out` | N was regressed by the related bug |
| `regressions` | `regressions` | array | `in` | the related bug is a regression caused by N |

`regressed_by`/`regressions`/`duplicates` are BMO extensions / reverse-computed
fields. Servers that do not return them simply yield empty arrays — the records
are absent, never an error.

**Direction rationale.** `out` edges point from the node to a bug it references
as a cause/parent (its dependency, its canonical duplicate, the bug that
regressed it). `in` edges are the inverse — bugs that point back at the node.
Walking only `out` edges recursively yields a root-cause/dependency tree;
walking only `in` edges yields the impact/dependents tree. The mapping is a
fixed per-field constant, not derived at runtime.

## Output

One record per related bug:

```json
{"id": 12346, "relation": "depends_on", "direction": "out", "depth": 1, "summary": "...", "status": "NEW"}
```

- `id` (u64), `relation` (string, one of the six), `direction` (`"in"`/`"out"`),
  `depth` (u32, ≥1), `summary` (string|null), `status` (string|null).
- `--json`: a pretty-printed JSON array (`[]` when empty).
- `--output ndjson`: one compact object per line (nothing when empty).
- table (default): columns `ID`, `RELATION`, `DIR`, `DEPTH`, `STATUS`,
  `SUMMARY`; when empty, prints `No related bugs for #<id>.` to stdout.

The root bug itself is never emitted — only related bugs.

### Root not found vs. root with no links

The empty-result path (`[]` / no ndjson lines / `No related bugs for #<id>.`) is
reached **only when the root bug fetched successfully** but yielded no in-scope
edges. If the **root** id cannot be fetched (nonexistent, or no read
permission), the command fails with the same `NotFound` error and exit code as
`bug view <id>` — it does **not** degrade to an empty "no related bugs" result.
Silent skipping (traversal point 6) applies to *related* bugs discovered during
the walk, never to the root.

## Traversal semantics

Bounded, cycle-safe BFS, each level batched into id-chunked requests:

1. A `visited` set seeded with the root id. The root is fetched (level 0) but
   not emitted.
2. For each level `k` (1..=max_depth), batch-fetch every newly-discovered,
   not-yet-fetched neighbor id, chunked into requests of ≤`LINKS_ID_CHUNK` ids
   (see Bounds). Each fetched node yields its `summary`/`status` (used to emit
   its record at depth `k`) and its adjacency (the level `k+1` frontier).
3. A bug id is emitted **once**, at the first (minimal) depth it is discovered.
   The recorded `relation`/`direction` is the edge that first discovered it.
   Re-encountering an already-visited id (a cycle or shared node) adds nothing.
4. Discovery order is server-independent: each level's frontier is processed in
   **ascending bug-id order** (the batch response array order is *not* relied
   upon — Bugzilla does not guarantee it matches the requested id order). For a
   single bug, its own relations are expanded in the fixed relation order of the
   table above. The combination — ascending parent id, then fixed relation
   order, then ascending neighbor id — makes first-discovery (and thus each
   node's recorded relation/direction) deterministic across runs and servers.
5. With `--relation R`, only edges of relation `R` are traversed and emitted.
6. Related bugs that cannot be fetched (permissions, deleted) are omitted from
   the batch response and therefore from the output. They are skipped silently;
   they do not abort the command. (This applies only to *related* bugs — the
   root is not skipped; see "Root not found vs. root with no links".)

Per-level round-trips are `ceil(level_width / LINKS_ID_CHUNK)` (REST/Hybrid),
bounded overall by `LINKS_MAX_NODES` — replacing the issue's per-related-bug
N+1 with a request count that scales with graph size / chunk size, not with
node count one-at-a-time. (The XML-RPC fallback below is the exception: it is
`O(nodes)`, since XML-RPC `get_bug` fetches one id per call.)

### Worked example (cycle)

Bugs: `1 depends_on 2`, `2 depends_on 3`, `3 depends_on 1` (a cycle).
`bzr bug links 1 --recursive --depth 5`:

- visited starts `{1}`. Fetch 1 → frontier `{2}` (relation `depends_on`).
- depth 1: fetch 2 → emit `{id:2, relation:depends_on, direction:out, depth:1}`;
  frontier `{3}`. visited `{1,2}`.
- depth 2: fetch 3 → emit `{id:3, …, depth:2}`; 3's neighbor `1` is already
  visited → not re-emitted. visited `{1,2,3}`.
- frontier empty → stop early at depth 2 even though `--depth 5`. Two records.

## Client boundary

A new fetch entry point returns isolated link nodes for a set of ids:

- REST / Hybrid: `GET bug?id=…&include_fields=id,summary,status,depends_on,blocks,dupe_of,duplicates,regressed_by,regressions`, with the id set split across requests of ≤`LINKS_ID_CHUNK` ids each.
- XML-RPC: per-id `get_bug`, mapping the resulting `Bug` to a link node
  (populates `depends_on`/`blocks`/`dupe_of`; the BMO-only fields stay empty,
  which XML-RPC servers do not provide anyway).

A node deserializes each relationship field with `#[serde(default)]`, so a
server that omits any field yields an empty list / `None`.

## Out of scope

- Alias input for `<id>` (relationship fields are numeric).
- Emitting the root bug as a record.
- Per-relation depth limits or weighted traversal.
- Surfacing inaccessible related-bug ids (silently skipped).

## Acceptance criteria (from the issue)

- `bzr bug links <id>` returns a non-empty result for a bug with ≥1 relationship.
- `--json` / `--output ndjson` match the documented record shape.
- `--recursive --depth N` performs a bounded BFS with a `depth` field; cycles are
  visited once.
- `--relation <type>` filters to the named relationship.
- `commands.yml` manifest and `docs/bzr-cli.md` updated; `make skills-test` drift
  check passes.
- Wiremock test covering one-hop and two-hop recursion with a cycle.
- Functional phase script exercising the command against a real container,
  including the credentialless path.

### Additional criteria (from spec review)

- A nonexistent / unreadable **root** id fails with the `NotFound` exit code
  (matching `bug view`), not an empty "no related bugs" result. Wiremock case:
  root 404.
- A frontier wider than `LINKS_ID_CHUNK` is fetched over multiple chunked
  requests; output is identical regardless of chunk boundaries.
- Discovery is deterministic when a batch response returns nodes out of
  requested-id order. Wiremock case: response array reordered vs. request.
- Hitting `LINKS_MAX_NODES` stops traversal, emits the collected records, and
  writes the truncation notice to stderr (stdout stays valid).
