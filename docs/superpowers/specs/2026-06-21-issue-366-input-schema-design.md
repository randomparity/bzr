# Issue 366: Input Schemas for `--from-json`

## Context

`bzr schema` publishes checked-in JSON Schemas for JSON output shapes, but the
structured input accepted by `bug create --from-json` and `bug update
--from-json` has no equivalent contract. Agents can submit these payloads, but
cannot validate key names or value kinds before invoking `bzr`.

## Decision

Add two schema names to the existing file-backed registry:

- `bug-create-input`
- `bug-update-input`

Each schema describes the whole payload accepted by `--from-json`: either a
top-level object or a top-level array of objects. The object schema is kept in a
`$defs` entry so drift tests can compare its `properties` with the serde
`deny_unknown_fields` parser for the corresponding input struct.

These schemas describe the JSON payload shape, not every command-level
precondition after CLI overlay:

- `bug-create-input` does not mark `product`, `component`, or `summary` as
  required because those may be supplied by explicit CLI flags.
- `bug-update-input` does not require `id` on the object form because
  positional IDs may provide the target. The array-item definition requires
  `id`, matching the array path.

Unknown keys remain disallowed with `additionalProperties: false`, matching the
parser. Custom-field writes stay out of scope until the custom-field input
surface is decided.

## Drift Guard

Add parser-local unit tests beside `bug create` and `bug update` that:

- derive the accepted key set from serde's `deny_unknown_fields` error;
- compare that key set to the schema object's `properties`;
- parse each schema property's example value through the real JSON input struct.

This keeps the published key set and representative value types aligned with
the parser without adding a runtime schema-generation dependency.

