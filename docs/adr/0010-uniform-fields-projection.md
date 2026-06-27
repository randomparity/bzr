# 0010 — Uniform `--fields` projection is a generic serde-key filter, not the bug enum

Status: Accepted

## Context

Issue #455 asks for `--fields` / `--exclude-fields` JSON projection on every
list/view verb (`comment list`, `attachment list`, `product list`/`view`,
`component list`/`view`, `user search`, `group list-users`/`view`,
`classification list`/`view`, `field list`), matching the projection that bug
verbs already have.

The bug projection (`src/types/bug/fields.rs`,
`src/commands/runtime/search/fields.rs`) is built around a `BugField` enum: a
hand-maintained registry of 25 variants, each with a canonical name, accepted
aliases (`assignee`→`assigned_to`, `updated`→`last_change_time`), and a table
header, plus table-column reflow and bug-view-specific leniency. Replicating that
for eight more resource types would mean eight more enums and registries.

The issue states the new verbs project by "the `--json` key names already
documented for each verb" — i.e. the serde field names, with no aliases — and
lets the implementer choose whether table output reflows columns or is a no-op.

## Decision

1. **Projection is a generic top-level key filter over `serde_json::Value`,
   keyed off each resource's serde field names — not a per-resource enum.** A
   shared `FieldProjection` (`src/validation/fields.rs`) parses/validates the
   comma lists against a `&[&str]` of known serde keys and trims the serialized
   `Value` in place. Each resource type declares `pub const <TYPE>_FIELDS:
   &[&str]` next to itself, guarded by a serialize-and-compare drift test. No
   aliases, no headers, no enums.

2. **Validation is strict and uniform: unknown token in `--fields` *or*
   `--exclude-fields`, or an empty resulting set, is `InputValidation`
   (exit 7).** This diverges from bug verbs, which warn on partial-unknown
   includes and exempt `bug view` from the zero-field error. The new verbs are
   uniformly strict because the issue's acceptance criterion says "unknown field
   name is an error" and uniformity is the goal. Bug verbs are untouched.

3. **Table output is a no-op with one stderr warning; projection affects only
   `--json` / `--output ndjson`.** Chosen over per-resource column reflow. Table
   columns stay fixed and stable for humans; projection serves the agent/machine
   path. Avoids eight column registries. Validation also runs only in the JSON
   family — in table mode the flags are a true documented no-op.

4. **One projection-aware output seam.** Affected writers gain a
   `projection: &FieldProjection` parameter and use
   `write_formatted_projected` / `write_table_or_empty_projected`, keeping a
   single JSON path per writer (no dead `match` arms) and preserving the
   `schema_version` envelope on `--json`.

## Consequences

- Adding projection to a new verb is mechanical: declare the key list, flatten
  `ProjectionArgs`, resolve in the handler, thread `&projection` to the writer.
- The drift-guard tests make field renames/additions fail loudly until the key
  list is updated, so the registries cannot silently rot.
- Two projection idioms now coexist: bug verbs (alias-aware, lenient, with table
  reflow) and the rest (serde-key-exact, strict, table no-op). This is a
  deliberate, documented asymmetry, not drift — bug verbs predate this work and
  carry a published, alias-friendly contract.
- Projection is top-level only; nested arrays/objects are kept or dropped whole.

## Considered & rejected

- **Generalize the `BugField` enum machinery to all resources.** Rejected:
  eight more registries with aliases/headers nobody asked for, far more code and
  surface than a serde-key filter, and it would couple every resource type to a
  table-column abstraction the no-op table decision makes unnecessary.

- **Reflow table columns to the selected fields.** Rejected: needs a per-resource
  column registry and ordering rules, destabilizes human output, and the issue
  explicitly allows the simpler no-op. Agents use `--json`, where projection is
  real.

- **Validate against the keys present in the serialized payload instead of a
  declared list.** Rejected: an empty array (zero results) exposes no keys, so an
  unknown field would be silently accepted on empty results and rejected on
  non-empty ones — non-deterministic. A static declared list keeps exit 7
  deterministic regardless of result count.

- **Reuse bug's `FieldArgs` clap struct.** Rejected: its help text describes
  bug-specific table-column behavior and aliases that do not apply. A separate
  `ProjectionArgs` keeps the help honest per verb.
