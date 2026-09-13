# RHBZ bug-output fields design

## Problem

RHBZ returns `target_release` and `sub_components` on bug reads, but bzr
currently preserves only unknown names beginning with `cf_`. Consequently an
explicit projection accepts neither field and both REST and XML-RPC discard
their server values before output formatting.

## Scope

Treat exactly `target_release` and `sub_components` as recognized extension
fields beside `cf_*` custom fields. They retain their raw JSON shapes in the
existing dynamic-field map, so a list remains a list and an object remains an
object. The existing dynamic projection and JSON/NDJSON writers then expose
them without a new output representation. Stock servers keep their existing
behavior: an explicitly requested field absent from a response is rendered as
an empty table cell and omitted from JSON/NDJSON, as for `cf_*` fields.

This does not accept arbitrary non-`cf_` fields and does not change the write
field path.

## Success

- `--fields target_release,sub_components` is a recognized projection.
- REST and XML-RPC preserve each field's server JSON shape in JSON and NDJSON.
- Table output renders the existing compact JSON representation for compound
  values.
- Documentation calls out the two RHBZ-only readable extension fields and the
  defined stock-server absence behavior.
- RHBZ server-backed tests perform write-to-read assertions for empty and
  multi-value states.

## Validation

- Focused Rust tests prove deserialization, serialization, and projection for
  both names across REST-shaped and XML-RPC-shaped responses.
- The RHBZ comparison phase reads fields through `bzr`, after its existing
  write operations, for empty and populated values.
- `make lint`, `make test`, and the required RHBZ functional test run prove
  repository and live-server behavior.
