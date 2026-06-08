# Custom field values in bug output

**Date:** 2026-06-08
**Branch:** `feature/custom-fields-design`
**Source:** user request after confirming `--from-url` works but custom field values
are not returned by `bzr`
**Follows:** `2026-05-27-json-field-trimming-design.md`

## Goal

Allow users to request Bugzilla custom fields with `--fields cf_*` and receive
those per-bug values directly in `bzr` output.

Today `bzr` already passes unknown field tokens through to Bugzilla's
`include_fields`, so a request such as:

```bash
bzr --json bug list --product Kernel --fields id,summary,cf_release
```

can ask the server for `cf_release`. The value is still lost because the REST
response is deserialized into a fixed `Bug` struct and unknown JSON keys are
dropped. The output layer also treats `cf_*` as unknown, so JSON projection and
table rendering cannot display it.

After this change, the same command returns top-level custom field keys:

```json
[
  {
    "id": 123,
    "summary": "panic during boot",
    "cf_release": "9.6"
  }
]
```

## Decisions

1. **Custom fields stay top-level in JSON.** `cf_*` keys appear beside built-in
   bug fields, matching Bugzilla's REST shape and the field names users pass to
   `--fields`.
2. **Only Bugzilla custom fields are captured.** Preserve response keys whose
   names start with `cf_`. Continue dropping unrelated extension keys instead of
   turning `Bug` into an arbitrary raw payload.
3. **No new CLI flag.** Existing `--fields` and `--exclude-fields` are the user
   interface for custom fields. Saved queries and `--from-url` inherit the same
   behavior when the user provides `--fields`, because those values already flow
   through `include_fields`.
4. **`cf_*` is a recognized dynamic field family.** A `cf_*` include token is no
   longer "unknown" for validation or warnings. Unknown non-`cf_*` tokens keep
   the existing warning/error behavior.
5. **`id` remains an internal fetch requirement only.** `force_id_fields` still
   adds `id` to the wire request when necessary, but output projection honors the
   user's field selection literally. `--fields cf_release --json` does not emit
   `id` unless the user requested it.
6. **Custom output order is mode-specific and deterministic.** JSON keeps the
   current built-in-field contract: built-ins appear in struct order, not request
   order, and custom fields appear after built-ins sorted by field name. Table
   output keeps the existing CLI contract: `--fields` controls column order, so
   requested custom columns appear wherever the user placed them.
7. **Default output does not fetch all custom fields.** With no `--fields`, the
   REST client continues to send `BUG_DEFAULT_FIELDS`. Custom field values appear
   when requested, or when a server returns a `cf_*` key despite the requested
   field list.

## Non-goals

- No support for arbitrary non-`cf_*` extension fields.
- No schema discovery or validation that a given `cf_*` field exists on the
  server. Bugzilla remains authoritative: it may return the value, omit it, or
  reject the request.
- No new custom-field filters beyond what Bugzilla already supports through
  structured URL parameters and raw `--from-url` passthrough.
- No change to bug create/update custom field support. This spec is read/output
  only.
- No change to comment, attachment, product, user, or field output.
- No automatic import of Bugzilla UI `columnlist` values from `buglist.cgi`
  URLs. The URL parser currently treats `columnlist` as display metadata and
  ignores it. Users who run `--from-url` must still pass `--fields cf_name` or
  save a query with `query save --from-url ... --fields cf_name` to request
  custom field values.

## Behavior

### JSON output

When a bug response contains a custom field:

```json
{
  "id": 123,
  "summary": "panic during boot",
  "cf_release": "9.6"
}
```

`bzr` preserves `cf_release` in the bug's internal custom-field map.

Projection rules:

- `--fields id,cf_release --json` emits `id` and `cf_release` if present.
- `--fields cf_release --json` emits only `cf_release` if present.
- `--fields cf_missing --json` is valid; if the server omits the key, the
  resulting bug object may be `{}`.
- `--fields summary,cf_release,typo --json` warns about `typo`, not about
  `cf_release`.
- `--exclude-fields cf_release --json` removes `cf_release` from any returned
  bug object.

Single `bug view --json`, multi-ID `bug view --json`, `bug list`, `bug my`,
`bug search`, and `query run` all use the same projection behavior.

### Table output

Requested custom fields render as dynamic columns in list-style table output:

```bash
bzr bug list --product Kernel --fields id,summary,cf_release
```

Expected table columns:

```text
ID  SUMMARY            CF_RELEASE
123 panic during boot  9.6
```

Requested custom fields also render as detail rows in `bug view` table output.
Detail output keeps the existing always-present `Bug #<id>` heading; `--fields`
controls the field rows below that heading.

```text
Bug #123
panic during boot

  Status        NEW
  cf_release    9.6
```

Custom value rendering for table cells and detail rows:

- strings render as their string value
- numbers and booleans render with `to_string()`
- `null` renders as an empty cell
- arrays and objects render as compact JSON

### Validation and warnings

Field selection validation needs to distinguish three categories:

1. built-in fields, resolved through the existing `COLUMNS` registry
2. custom fields, recognized by an exact `cf_` prefix
3. unknown fields, everything else

Rules:

- All-unknown include still exits 7 for list-style commands:
  `--fields typo,other_typo`.
- All-custom include is valid and performs network I/O:
  `--fields cf_release,cf_target`.
- Partial unknown warns once and keeps valid fields:
  `--fields summary,cf_release,typo` warns only for `typo`.
- `bug view` keeps its existing zero-field leniency, but still warns for unknown
  non-`cf_*` include tokens.
- `--fields cf_release --exclude-fields cf_release` exits 7 for list-style JSON
  and table output because the effective selected set is empty.
- With no include list, JSON validation still measures the default universe as
  built-in fields only. `--exclude-fields` for every built-in field exits 7 even
  if custom fields could theoretically appear, because `bzr` did not request any
  custom fields by default.

## Architecture

### Data model

Extend `Bug` with a custom-field map:

```rust
pub struct Bug {
    pub id: u64,
    pub summary: String,
    // existing built-in fields...
    pub rep_platform: Option<String>,
    pub custom_fields: BTreeMap<String, serde_json::Value>,
}
```

The public map contains only keys starting with `cf_`.

Implementation contract:

1. Add a private `BugWire` helper for deserialization. It carries the existing
   built-in fields plus `#[serde(flatten)] extra:
   BTreeMap<String, serde_json::Value>`. Its built-in fields must mirror the
   current `Bug` serde defaults exactly, including sparse responses such as
   `{"id": 42}` and `{"id": 42, "cf_release": "9.6"}`.
2. Implement `Deserialize` for `Bug` by converting `BugWire` into `Bug` and
   filtering `extra` down to keys whose names start with `cf_`.
3. Implement `Serialize` for `Bug` manually. Do not derive `Serialize` for a
   public `custom_fields` field, because that can emit a nested
   `"custom_fields"` key if `#[serde(flatten)]` is missed and would violate the
   output contract. Manual serialization emits built-ins in the current struct
   order, then `custom_fields` entries in `BTreeMap` order.
4. Keep `custom_fields` public for tests and internal output helpers, but never
   emit it as a literal JSON key.

This keeps non-custom extension data out of the public type while preserving
Bugzilla's top-level `cf_*` shape.

### REST client

No change is needed to request construction for the happy path:

- `canonical_field_list` already passes unknown tokens through unchanged.
- `force_id_fields` already ensures deserialization has `id`.
- REST response parsing goes through `Bug`, so custom-field capture belongs in
  the type deserializer rather than in `client/bug.rs`.

The default `BUG_DEFAULT_FIELDS` should not gain custom fields. Users must
request specific `cf_*` names.

`--from-url` is not a request-construction exception. `columnlist` remains
ignored display metadata. A URL-imported search requests custom fields only when
the user also supplies `--fields` at run time, or when the saved query contains a
stored `fields` value from `query save --from-url ... --fields ...`.

### XML-RPC client

`xmlrpc::client::value_to_bug` manually constructs `Bug`, so it must also fill
`custom_fields`.

Add a small XML-RPC to JSON converter for values under keys starting with
`cf_`:

- string -> JSON string
- integer -> JSON number
- boolean -> JSON boolean
- datetime -> JSON string using the existing formatted value
- array -> JSON array, recursively converted
- struct -> JSON object, recursively converted
- double -> JSON number when finite, otherwise JSON string
- base64 -> base64-encoded JSON string using the existing `base64` dependency

The goal is parity with REST for common Bugzilla custom field values: text,
single-select strings, multi-select arrays, booleans, and numeric values. The
converter must not fail an otherwise valid bug response solely because a custom
field uses XML-RPC base64; lossy-but-readable output is better than dropping the
bug in hybrid fallback.

### Field registry and selection

The current `BugColumn` registry only models built-in fields. Introduce a small
selection type so the output layer can carry both built-in and dynamic fields:

```rust
enum SelectedBugField<'a> {
    BuiltIn(&'static BugColumn),
    Custom(&'a str),
}
```

Parsing helpers should preserve token order and also expose the partitions needed
for validation and warnings:

```rust
struct FieldPartition<'a> {
    ordered: Vec<SelectedBugField<'a>>,
    built_ins: Vec<&'static BugColumn>,
    custom: Vec<&'a str>,
    unknown: Vec<&'a str>,
}
```

Use the partition in all places that currently call `partition_include` or
`resolve_bug_column` for include-list validation, warnings, and rendering.
`ordered` is the source of truth for table column order; `built_ins`, `custom`,
and `unknown` are derived views used by validation, JSON projection, and warning
messages.

`canonical_field_list` can remain mostly unchanged: built-in aliases map to
canonical Bugzilla field names, and `cf_*` tokens pass through unchanged.

For table rendering, preserve the user's include-list order across built-in and
custom fields. `--fields cf_release,id,summary` renders `CF_RELEASE`, `ID`,
`SUMMARY` in that order. Duplicate include tokens are rendered once, keeping the
first occurrence. For JSON projection, keep the existing JSON-order contract:
struct-order built-ins first, sorted custom fields after them.

### JSON projection

`bug_to_json` should continue to project a serialized object, but the serialized
object now includes flattened `cf_*` keys.

Include mode:

1. Resolve built-in aliases to canonical names.
2. Keep exact `cf_*` tokens.
3. Retain only keys in that effective set.

Exclude mode:

1. Resolve built-in aliases to canonical names.
2. Remove exact `cf_*` tokens.
3. Ignore unknown non-`cf_*` excludes, matching current behavior.

No projection layer should warn. Warnings stay in the command preflight gate
where `w.err` is available.

### Table rendering

List-style output should render both built-in columns and requested custom
columns. The custom column header is the uppercased field name, for example
`cf_release` -> `CF_RELEASE`.

Detail output should render requested custom fields as rows after the selected
built-in rows. With no include list, detail output should not print every
captured custom field. With an include list, render only the custom fields named
in that include list, after built-in rows and in include-list order with
duplicates removed. This keeps default `bug view` stable and avoids accidentally
exposing server-specific fields.

## Tests

### Unit tests

`src/types/bug_tests.rs`:

- REST JSON deserialization captures `cf_release`.
- REST JSON deserialization of `{"id": 42, "cf_release": "9.6"}` still defaults
  missing built-in fields the same way `{"id": 42}` does today.
- REST JSON deserialization drops non-`cf_*` unknown extension keys.
- serializing `Bug` emits custom fields as top-level keys, not nested under
  `custom_fields`.
- serialized JSON orders built-ins before sorted custom fields.
- default `Bug` test helpers initialize `custom_fields` empty.

`src/xmlrpc/client_tests.rs`:

- XML-RPC bug parsing captures `cf_release`.
- XML-RPC arrays for multi-select custom fields become JSON arrays.
- XML-RPC doubles and base64 custom fields serialize without failing the whole
  bug parse.
- non-`cf_*` XML-RPC extras are ignored.

`src/output/resources/bug_tests.rs`:

- `bug_to_json` includes selected custom fields.
- `bug_to_json` excludes selected custom fields.
- `bug_to_json` with `--fields summary,cf_release` keeps both.
- all-custom `validate_json_field_selection` succeeds.
- all-unknown non-custom include still fails for list-style output.
- partial unknown warns only for non-custom fields.
- table output renders requested custom columns.
- table output preserves mixed built-in/custom include order.
- table output deduplicates repeated include tokens by first occurrence.
- detail output renders requested custom rows.
- detail output with no include list does not render captured custom fields.

### Command tests

`src/commands/bug/list_tests.rs`:

- `bug list --fields id,cf_release --json` sends `include_fields=id,cf_release`
  on the wire and emits `cf_release`.
- `bug list --fields cf_release --json` does not emit `id`.
- table mode renders a `CF_RELEASE` column.

`src/commands/bug/search_tests.rs`:

- `bug search --from-url ... --fields id,cf_release --json` emits the custom
  field.
- `bug search --from-url` does not infer fields from URL `columnlist`; users must
  pass `--fields` if they want custom values returned.

`src/commands/query_tests.rs`:

- saved query fields containing `cf_release` are honored by `query run`.

`src/commands/bug/view_tests.rs`:

- single `bug view --fields cf_release --json` emits only `cf_release`.
- multi-ID `bug view --fields id,cf_release --json` projects each bug inside
  the `bugs` array.
- table detail output includes the custom row when requested.

### Integration/docs tests

Update the documentation guard in `tests/integration.rs` that currently says
custom `cf_*` fields cannot become visible via `--json`. Replace it with a
guard that prevents misleading payload wording while allowing accurate custom
field documentation.

Add or update docs in `docs/bzr-cli.md` and `src/cli/bug.rs`:

- `--fields` can request built-in fields and Bugzilla custom fields named
  `cf_*`.
- custom fields are returned only when requested or when the server returns them.
- unknown non-custom fields still warn or fail as before.

Add a `CHANGELOG.md` entry when the implementation lands, since this changes
user-visible output behavior.

## Implementation order

1. Add failing tests for REST `Bug` deserialization and top-level serialization.
2. Add `custom_fields` to `Bug` with filtered `cf_*` capture.
3. Add XML-RPC custom-field capture.
4. Teach field selection helpers that `cf_*` is a dynamic field family.
5. Update JSON projection to retain/remove custom fields.
6. Add table list/detail rendering for requested custom fields.
7. Update command/integration tests.
8. Update CLI help, `docs/bzr-cli.md`, and `CHANGELOG.md`.
9. Run focused tests:

```bash
cargo test types::bug_tests
cargo test xmlrpc::client_tests
cargo test output::resources::bug_tests
cargo test commands::bug::list_tests
cargo test commands::bug::search_tests
cargo test commands::bug::view_tests
cargo test commands::query_tests
cargo test --test integration custom
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Resolved tradeoffs

1. `bug view` with no `--fields` does not show captured custom fields even if
   the server returns them anyway. Stable default output is more important than
   opportunistically exposing server-specific fields.
2. Custom table headers use uppercase field names, for example `CF_RELEASE`,
   matching the existing table header style.
3. Any exact `cf_` prefix is treated as a custom field token. `bzr` does not
   validate whether `cf_` itself or any other `cf_*` name is a real Bugzilla
   field; Bugzilla remains authoritative.
