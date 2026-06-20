# #305 — Consistent / documented JSON envelope (+ NDJSON)

Status: in progress (branch `feat/json-envelope-ndjson-305`)

## Problem

JSON output shapes differ across command families (bare arrays for lists,
`{bugs, failed}` for multi `bug view`, `{resource, action, …}` for mutations,
`comment_id` for `comment tag`, name-keyed maps for templates/queries). These
exceptions are documented in prose but agents still pay for them in per-command
branching, and there is no machine-readable schema to validate against. There is
also no streaming-friendly output for large result sets.

## Acceptance criteria (from the issue)

1. An agent can rely on a published schema per command.
2. NDJSON is available for large/streamed result sets.
3. Current default shapes stay stable (or change only behind an opt-in).

## Decisions

### NDJSON (criterion 2)

Add a third `OutputFormat` variant, `Ndjson`, selectable via `--output ndjson`
and `BZR_OUTPUT=ndjson`. Semantics:

- **Array outputs** (`bug list`/`search`/`my`, `comment list`, `attachment
  list`, `product list`, `user`, `classification list`, `component list`, field
  values, batch `succeeded`/`failed` sets): one compact JSON value per line.
- **Single objects / envelopes**: the value as a single compact line (still
  valid NDJSON — one object, one line).

Implementation altitude: a single `write_json_family(value, format, out)` helper
in `output/formatting.rs` centralizes pretty-vs-NDJSON. `write_formatted` (used
by most list writers via `write_table_or_empty`) routes through it, so the
majority of list commands get NDJSON for free. The ~14 bespoke `match format`
sites change their `OutputFormat::Json =>` arm to
`OutputFormat::Json | OutputFormat::Ndjson => write_json_family(…, format, …)`.
`--json` remains shorthand for `--output json` (pretty), unchanged.

### Published schema (criterion 1)

`Bug` JSON is **dynamic**: its hand-written `Serialize` (`src/types/bug.rs`)
flattens server-specific custom fields to the top level, and `--fields`
selection projects the key set at render time. A derive-based generator
(`schemars`) would describe the Rust struct (a nested `custom_fields` map), not
the real wire shape — so derived schemas would be wrong for the one resource the
issue cares most about.

Therefore: a checked-in `schemas/` directory of authored JSON Schema (draft
2020-12) files, one per output shape, embedded in the binary via `include_str!`
and surfaced by a local (no-network) `bzr schema` command:

- `bzr schema` — list available schema names.
- `bzr schema <name>` — print that schema.

This adds **zero runtime dependencies**, keeps full fidelity for the dynamic bug
object (built-in properties + `additionalProperties: true` for custom fields,
with a description noting `--fields` projection), and the schema files are
themselves agent-consumable artifacts in the repo.

Drift guard: a test (test-only `jsonschema` dev-dependency) validates a
representative serialized sample for each shape against its schema, so a struct
change that breaks a schema fails CI.

### Stable defaults (criterion 3)

No unified envelope is added. Bare arrays stay bare; existing object shapes are
unchanged. NDJSON and `schema` are purely additive. The optional `{data, meta}`
envelope from the issue is intentionally **not** implemented (speculative,
no current consumer) — the schema command + NDJSON cover the stated agent need.

## Out of scope

- `{data, meta}` unified envelope behind a flag (no current consumer).
- Reshaping batch envelopes for NDJSON (kept as documented).
