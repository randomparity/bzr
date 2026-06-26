# 0006 — `bug links` uses an isolated relationship fetch

- Status: Accepted
- Date: 2026-06-26
- Issue: #453

## Context

Issue #453 adds `bzr bug links <id>`, returning a bug's relationship graph
(`depends_on`, `blocks`, `dupe_of`, `duplicates`, `regressed_by`, `regressions`)
as a flat list, optionally walked recursively.

bzr's global `Bug` type models only three of the six relationship fields
(`depends_on`, `blocks`, `dupe_of`). The other three are not parsed: `BugWire`
captures unknown keys via `#[serde(flatten)] extra` but keeps only `cf_*`
custom fields, so `duplicates`, `regressed_by`, and `regressions` are dropped on
deserialization. The global `BUG_DEFAULT_FIELDS` include-list (used by
`bug view`/`list`/`my`) does not request them either, and its own comment warns
that some Bugzilla extensions crash the REST serializer when certain fields are
requested. `regressed_by`/`regressions` are BMO extensions absent from stock
Bugzilla; `duplicates` is a reverse-computed field.

So "support all six relations" is not a free addition to the existing read path —
it touches a shared type and a shared, risk-annotated request constant.

A second decision is the `direction` field's meaning: the six relations form
three inverse pairs, and the command must assign each a stable `in`/`out` label.

## Decision

1. **Isolated fetch type, global `Bug` untouched.** `bug links` fetches through a
   dedicated link-node type that deserializes exactly
   `id,summary,status,depends_on,blocks,dupe_of,duplicates,regressed_by,regressions`,
   each relationship field `#[serde(default)]`. The client requests precisely
   this `include_fields` set. The global `Bug` struct, `BugWire`, and
   `BUG_DEFAULT_FIELDS` are not modified, so no other command's request shape or
   parse behavior changes, and the extension-crash risk on the shared default
   list is not widened.

2. **Direction is a fixed per-relation constant.** `out` =
   {`depends_on`, `dupe_of`, `regressed_by`}; `in` =
   {`blocks`, `duplicates`, `regressions`}. `out` edges point from the node to a
   bug it references as a dependency/cause/canonical-duplicate; `in` edges are the
   inverse. Walking only `out` edges yields a root-cause/dependency tree, only
   `in` edges the impact/dependents tree. The mapping is a compile-time constant,
   not derived from server data.

3. **Bounded, batched, cycle-safe BFS.** Traversal is breadth-first with a
   `visited` set (seeded with the root), one batched REST request per level
   (`O(depth)` round-trips), each bug emitted once at its minimal depth with the
   discovering edge's relation/direction. `--depth` is bounded to `1..=10`.

4. **Graceful degradation over hard dependency on BMO fields.** A server that
   omits `duplicates`/`regressed_by`/`regressions` yields empty lists, not an
   error. Related bugs that cannot be fetched are silently skipped.

## Consequences

- The new command carries its own small, purpose-built type; relationship parsing
  for links and for `bug view` evolve independently.
- Adding the three BMO/reverse fields to the *global* `Bug` type later remains
  possible but is explicitly not required by this feature, and is kept off the
  shared default-field path until its serializer-crash risk is assessed.
- The `direction` contract is a committed part of the `--json` shape; changing a
  relation's label later is a breaking change for consumers.
- Round-trips scale with depth, not graph size, satisfying the issue's N+1
  motivation; the depth cap and visited set bound worst-case work on adversarial
  graphs.

## Considered & rejected

- **Add the three missing fields to the global `Bug` type and
  `BUG_DEFAULT_FIELDS`.** Rejected: widens a shared, risk-annotated request
  constant (extension serializer crashes) and changes every `bug view`/`list`
  response for a single command's needs. Revisit only if multiple commands need
  the fields.
- **Support only the three already-modeled relations.** Rejected: the issue
  explicitly requires all six, and the reverse/BMO fields are the higher-value
  half for impact analysis. The isolated-fetch design makes all six cheap and
  low-risk.
- **One REST request per related bug during traversal.** Rejected: reproduces the
  exact N+1 round-trip cost the feature exists to remove. Batching per BFS level
  keeps requests at `O(depth)`.
- **Follow the graph with no depth cap.** Rejected: an adversarial or large graph
  could fan out unboundedly. A `1..=10` cap plus the visited set bounds work while
  covering realistic triage depths.
- **`direction` derived from whether a field is stored vs. reverse-computed.**
  Rejected: less useful for traversal than the inverse-pair convention, and
  ambiguous for `regressed_by`'s passive phrasing. A fixed per-field constant is
  unambiguous and falsifiable.
