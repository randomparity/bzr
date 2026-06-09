# Custom Field Values in Bug Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve Bugzilla custom fields named `cf_*` in bug responses and
render them when users request them with `--fields` / `--exclude-fields`.

**Architecture:** Keep the existing fixed `Bug` model for built-in fields, but
add a public `custom_fields: BTreeMap<String, serde_json::Value>` that captures
only flattened response keys beginning with `cf_`. Replace the current
built-in-only field selection helpers with a partition that distinguishes
built-ins, custom fields, and unknown fields. REST deserialization, XML-RPC
fallback, JSON projection, table columns, detail rows, validation, and warnings
all consume that same partition so `cf_*` behaves as a first-class dynamic field
family without accepting arbitrary extension keys.

**Spec:** `docs/superpowers/specs/2026-06-08-custom-fields-output-design.md`

---

## Background

The prior `--fields` work already exists in this branch:

- `src/output/resources/bug.rs` has `ColumnSpec`, a built-in `BugColumn`
  registry, dynamic table rendering, JSON projection, and preflight validation.
- `src/commands/bug/mod.rs` validates field selection before network I/O for
  list-style bug commands, while keeping `bug view` lenient.
- `src/commands/bug/list.rs`, `src/commands/bug/search.rs`,
  `src/commands/bug/my.rs`, and `src/commands/query.rs` already canonicalize
  aliases before sending `include_fields` / `exclude_fields`.
- `src/commands/bug/view.rs` has separate single-ID and multi-ID paths. Watch
  this file carefully: single-ID currently uses canonicalized field values for
  both fetch and render, while multi-ID uses canonicalized values for fetch and
  raw values for render. The custom-field implementation should make this
  intentional and consistent.

Today, unknown include tokens pass through to Bugzilla on the wire, which is
good for `cf_release`, but the returned field is dropped because `Bug` derives
`Deserialize` with only known built-in fields. The output layer then classifies
`cf_release` as unknown, warning or failing before it can render.

---

## File Structure

**Modify:**

- `src/types/bug.rs` - add `BugWire`, manual `Deserialize` and `Serialize` for
  `Bug`, and the `custom_fields` map.
- `src/types/bug_tests.rs` - cover REST deserialization, sparse defaults,
  top-level serialization, ordering, and non-`cf_*` extension dropping.
- `src/xmlrpc/client.rs` - capture XML-RPC `cf_*` members in `value_to_bug` and
  convert XML-RPC values to `serde_json::Value`.
- `src/xmlrpc/client_tests.rs` - cover XML-RPC custom strings, arrays, doubles,
  base64, and ignored non-custom extras.
- `src/client/bug_tests.rs` - lock down the wire-only `id` fetch requirement
  for custom-only field selections.
- `src/output/resources/bug.rs` - replace built-in-only selection helpers with
  a field partition, add `SelectedBugField`, render custom values in table and
  detail output, and update JSON projection/validation/warnings.
- `src/output/resources/bug_tests.rs` - add output behavior tests for custom
  JSON, validation, warnings, table columns, ordering, dedupe, and detail rows.
- `src/commands/bug/list_tests.rs` - command coverage for custom fields in JSON
  and table output.
- `src/commands/bug/my_tests.rs` - command coverage for custom fields through
  the `whoami`-driven personal bug searches.
- `src/commands/bug/search_tests.rs` - `--from-url` custom field behavior and
  `columnlist` non-import behavior.
- `src/commands/bug/view_tests.rs` - single and multi-ID projection plus table
  detail rows for requested custom fields.
- `src/commands/query_tests.rs` - saved query fields containing `cf_*` are
  honored by `query run`.
- `tests/integration.rs` - update the documentation guard that currently treats
  custom field visibility as impossible.
- `src/cli/bug.rs`, `src/cli/query.rs`, `docs/bzr-cli.md` - document that
  `--fields` / `--exclude-fields` accept built-ins and `cf_*` custom fields.
- `CHANGELOG.md` - add an Unreleased entry for user-visible output behavior.

**No new dependencies.** `serde_json`, `base64`, and `BTreeMap` are already
available.

**Commit safety:** Tasks 4-6 change one public behavior surface: whether `cf_*`
field selections are accepted and rendered. If implementing one task per commit,
do not commit a state where validation accepts a custom field that the active
output mode still cannot project or render. Either keep Task 4's partition work
internal until Tasks 5 and 6 land, or squash Tasks 4-6 into one behavior commit.

---

## Task 1: Add failing REST `Bug` model tests

**Files:**

- Modify: `src/types/bug_tests.rs`

- [ ] Add a helper that builds a minimal bug JSON object with `id`, `summary`,
  and one or two `cf_*` keys.
- [ ] Add a test that deserializing a REST bug captures `cf_release` in
  `bug.custom_fields`.
- [ ] Add a sparse-response test for `{"id": 42, "cf_release": "9.6"}` that
  proves missing built-ins keep the same defaults as the current `Bug`
  deserializer.
- [ ] Add a test that unknown non-custom extension keys are dropped.
- [ ] Add a serialization test that `cf_release` is emitted as a top-level key,
  not under `"custom_fields"`.
- [ ] Add an ordering test: built-in keys come first in the existing `Bug`
  struct order, then custom keys in sorted `BTreeMap` order.
- [ ] Update any `Bug` test fixtures in this file to initialize
  `custom_fields: BTreeMap::new()`.
- [ ] Run `cargo test --lib types::bug_tests`.

Expected result: tests fail to compile because `Bug::custom_fields` does not
exist and `Bug` still derives serialization/deserialization.

---

## Task 2: Implement REST custom-field capture

**Files:**

- Modify: `src/types/bug.rs`
- Modify as needed: any test helpers that construct `Bug` directly

- [ ] Add `use std::collections::BTreeMap;` and keep the existing serde imports
  explicit.
- [ ] Add `pub custom_fields: BTreeMap<String, serde_json::Value>` to `Bug`.
- [ ] Stop deriving `Serialize` and `Deserialize` directly for `Bug`.
- [ ] Introduce a private `BugWire` struct that mirrors every current built-in
  `Bug` field, including all existing `#[serde(default)]` behavior.
- [ ] Add `#[serde(flatten)] extra: BTreeMap<String, serde_json::Value>` to
  `BugWire`.
- [ ] Implement `Deserialize` for `Bug` by deserializing `BugWire`, filtering
  `extra` to keys with an exact `cf_` prefix, and copying built-ins into `Bug`.
- [ ] Implement `Serialize` for `Bug` manually with `SerializeMap`, emitting
  built-ins in the current struct order and custom fields afterward in
  `BTreeMap` order.
- [ ] Make sure manual serialization does not emit absent optional fields or
  empty lists differently than the existing derived serialization. If the
  current derived output includes `null` and empty arrays, preserve that.
- [ ] Update all direct `Bug { ... }` literals in tests and production code to
  populate `custom_fields: BTreeMap::new()`.
- [ ] Run `cargo test --lib types::bug_tests`.

Expected result: the new type tests pass.

---

## Task 3: Add XML-RPC custom-field capture

**Files:**

- Modify: `src/xmlrpc/client.rs`
- Modify: `src/xmlrpc/client_tests.rs`

- [ ] Add failing tests that `value_to_bug` captures `cf_release` from an
  XML-RPC struct and ignores a non-custom extension member.
- [ ] Add tests for representative XML-RPC custom values:
  string, integer, boolean, datetime, array, struct, finite double, non-finite
  double, and base64.
- [ ] Add `xmlrpc_value_to_json(value: &Value) -> serde_json::Value`.
- [ ] Convert XML-RPC values as specified:
  string to JSON string, integer to JSON number, boolean to JSON boolean,
  datetime to JSON string, arrays and structs recursively, finite doubles to
  JSON numbers, non-finite doubles to JSON strings, and base64 to a base64
  encoded JSON string using the existing `base64` dependency.
- [ ] Add `custom_fields_from_xmlrpc(m: &BTreeMap<String, Value>)` that keeps
  only keys starting with `cf_`.
- [ ] Populate `Bug { custom_fields, ... }` in `value_to_bug`.
- [ ] Run `cargo test --lib xmlrpc::client_tests`.

Expected result: XML-RPC tests pass, and malformed or unusual custom values do
not fail an otherwise valid bug response.

---

## Task 4: Replace built-in-only field selection with partitions

**Files:**

- Modify: `src/output/resources/bug.rs`
- Modify: `src/output/resources/bug_tests.rs`

- [ ] Add failing tests that:
  - `validate_json_field_selection` accepts all-custom includes.
  - `warn_unknown_fields` is silent for `cf_release`.
  - `warn_unknown_fields` still warns for non-custom typos.
  - `--fields summary,cf_release,typo` warns only for `typo`.
  - the new field partition classifies `cf_release` as custom rather than
  unknown.
- [ ] Add `SelectedBugField<'a>` with `BuiltIn(&'static BugColumn)` and
  `Custom(&'a str)`.
- [ ] Add `FieldPartition<'a>` with `ordered`, `built_ins`, `custom`, and
  `unknown`.
- [ ] Implement a parser that trims blank tokens, deduplicates requested fields
  by effective identity, preserves first occurrence order, resolves aliases to
  built-ins, classifies exact `cf_` prefix tokens as custom, and classifies
  everything else as unknown.
- [ ] Replace `partition_include` callers with `FieldPartition`.
- [ ] Keep `canonical_field_list` behavior mostly unchanged: built-in aliases
  map to canonical field names, and `cf_*` tokens pass through exactly.
- [ ] Update `canonical_excludes` so it returns both canonical built-in keys and
  exact custom keys, while continuing to ignore unknown non-custom excludes.
- [ ] Update JSON validation so all-custom includes are valid and all-unknown
  non-custom includes still exit 7 for list-style JSON commands.
- [ ] Add validation tests for `--fields cf_release --exclude-fields cf_release`
  in list-style JSON mode. It should exit 7 because the effective selected set
  is empty.
- [ ] Add validation coverage for excluding every default built-in with no
  include list. It should still exit 7 in JSON even though custom fields could
  theoretically exist, because no custom fields were requested.
- [ ] Update warning logic to ignore `cf_*` tokens.
- [ ] Run `cargo test --lib output::resources::bug_tests`.

Expected result: selection and validation tests pass before rendering support is
added.

---

## Task 5: Update JSON projection for custom fields

**Files:**

- Modify: `src/output/resources/bug.rs`
- Modify: `src/output/resources/bug_tests.rs`

- [ ] Add failing tests that:
  - `bug_to_json` includes `cf_release` when selected.
  - `bug_to_json` excludes `cf_release` when excluded.
  - `bug_to_json` with `--fields summary,cf_release` keeps both keys.
  - `bug_to_json` with `--fields cf_release` emits no `id` unless requested.
  - `bug_to_json` with `--fields cf_missing` does not synthesize `null` or an
  empty string when the server omitted the custom key; the projected object may
  be `{}`.
  - full-object JSON orders built-ins before sorted custom fields.
- [ ] Update `bug_to_json` include mode to keep canonical built-in keys plus
  exact custom keys.
- [ ] Update exclude mode to remove canonical built-in keys plus exact custom
  keys.
- [ ] Keep projection warning-free; command preflight owns warnings because it
  has stderr access.
- [ ] Run `cargo test --lib output::resources::bug_tests`.

Expected result: JSON projection tests pass.

---

## Task 6: Render custom fields in table output

**Files:**

- Modify: `src/output/resources/bug.rs`
- Modify: `src/output/resources/bug_tests.rs`

- [ ] Add failing tests that:
  - `validate_table_columns` accepts all-custom includes once custom table
  rendering is implemented.
  - `validate_table_columns` still rejects all-unknown non-custom includes.
  - `write_bugs` renders requested custom columns.
  - mixed built-in/custom include order is preserved.
  - repeated include tokens render once, keeping the first occurrence.
  - string, number, boolean, null, array, and object custom values render as
  specified.
  - a requested custom column whose value is missing on a bug renders as an
  empty cell instead of falling back, warning, or panicking.
  - default table output does not render captured custom fields.
- [ ] Add a `render_custom_value(&serde_json::Value) -> String` helper:
  strings as-is, numbers and booleans via `to_string`, null as empty, arrays
  and objects as compact JSON.
- [ ] Update list-style table rendering to use `SelectedBugField::ordered`
  instead of `Vec<&BugColumn>`.
- [ ] Render custom column headers as uppercased field names, for example
  `cf_release` to `CF_RELEASE`.
- [ ] Preserve the current default table columns when no include list is
  supplied.
- [ ] Ensure `--exclude-fields cf_release` removes a requested custom column.
- [ ] Assert that excluding the only requested custom column is rejected before
  rendering, not rendered as an empty table.
- [ ] Run `cargo test --lib output::resources::bug_tests`.

Expected result: table list custom-field tests pass.

---

## Task 7: Render custom fields in bug detail output

**Files:**

- Modify: `src/output/resources/bug.rs`
- Modify: `src/output/resources/bug_tests.rs`

- [ ] Add failing tests that:
  - `bug view` detail output renders a requested custom row.
  - no-include detail output does not render captured custom fields.
  - custom detail rows follow selected built-in rows and preserve include-list
  order.
  - duplicate custom include tokens render one row.
  - a requested custom detail row whose value is missing renders with an empty
  value instead of being treated as an unknown field.
- [ ] Update `field_selected` or replace it with partition-driven detail logic
  so built-in detail rows keep current behavior.
- [ ] Render requested custom rows after built-in rows only when they appear in
  the include list and are not excluded.
- [ ] Use the same `render_custom_value` helper as list-style table output.
- [ ] Keep the `Bug #<id>` heading always present, even when no detail rows are
  selected.
- [ ] Run `cargo test --lib output::resources::bug_tests`.

Expected result: detail output custom-field tests pass.

---

## Task 8: Add command-level coverage

**Files:**

- Modify: `src/commands/bug/list_tests.rs`
- Modify: `src/commands/bug/my_tests.rs`
- Modify: `src/commands/bug/search_tests.rs`
- Modify: `src/commands/bug/view_tests.rs`
- Modify: `src/commands/query_tests.rs`
- Modify: `src/client/bug_tests.rs`
- Modify if needed: `src/commands/bug/view.rs`

- [ ] Add `bug list --fields id,cf_release --json` test coverage proving the
  REST request sends `include_fields=id,cf_release` and output emits
  `cf_release`.
- [ ] Add `bug list --fields cf_release --json` coverage proving `id` is added
  to the REST request as an internal fetch requirement but does not appear in
  output projection.
- [ ] Add client-level `force_id_fields` coverage for custom-only includes:
  `Some("cf_release")` becomes `id,cf_release`, while `id` remains removable
  from the output projection layer.
- [ ] Add client-level coverage that default bug searches still use the existing
  `BUG_DEFAULT_FIELDS` request and do not fetch or discover all server custom
  fields when `--fields` is absent.
- [ ] Add table-mode `bug list --fields id,cf_release` coverage proving a
  `CF_RELEASE` column renders.
- [ ] Add `bug my --fields id,cf_release --json` coverage proving every
  generated personal search carries the custom field request and projected
  output includes `cf_release`.
- [ ] Add `bug search --from-url ... --fields id,cf_release --json` coverage
  proving custom fields survive URL-imported search execution.
- [ ] Add `bug search --from-url` coverage proving URL `columnlist` still does
  not infer custom fields without explicit `--fields`.
- [ ] Add `query run` coverage for a saved query whose stored `fields` includes
  `cf_release`.
- [ ] Add single-ID `bug view --fields cf_release --json` coverage proving only
  the custom key emits.
- [ ] Add multi-ID `bug view --fields id,cf_release --json` coverage proving
  each object in the `bugs` array is projected.
- [ ] Add table detail coverage proving `bug view --fields cf_release` renders
  a custom row.
- [ ] Fix any single-ID vs multi-ID `bug view` spec drift uncovered by these
  tests. Fetch parameters should use canonicalized built-in aliases; render
  selection should still understand aliases and exact `cf_*` tokens.
- [ ] Run:

```bash
cargo test --lib commands::bug::list_tests
cargo test --lib commands::bug::my_tests
cargo test --lib commands::bug::search_tests
cargo test --lib commands::bug::view_tests
cargo test --lib commands::query_tests
```

Expected result: command paths pass and warnings fire only for unknown
non-custom field tokens.

---

## Task 9: Update integration guard, docs, and changelog

**Files:**

- Modify: `tests/integration.rs`
- Modify: `src/cli/bug.rs`
- Modify: `src/cli/query.rs`
- Modify: `docs/bzr-cli.md`
- Modify: `CHANGELOG.md`

- [ ] Replace the integration/documentation guard that currently says custom
  `cf_*` fields cannot become visible via `--json`.
- [ ] Add or update CLI help for bug/list/search/view/my and query save/run:
  `--fields` can request built-in bug fields and Bugzilla custom fields named
  `cf_*`.
- [ ] Document that custom fields are returned only when requested, or when the
  server returns them despite the requested field list.
- [ ] Document that unknown non-custom fields still warn or fail as before.
- [ ] Add an `[Unreleased]` changelog bullet under `### Changed` or `### Added`
  describing custom `cf_*` output support.
- [ ] Run `cargo test --test integration custom` or the nearest focused test
  names for the updated guard.

Expected result: user-facing docs match implemented behavior without suggesting
schema discovery or arbitrary extension-field support.

---

## Task 10: Final verification

- [ ] Re-read the diff for unnecessary abstraction, repeated logic, unclear
  names, and accidental broadening to non-`cf_*` extension fields.
- [ ] Run focused tests:

```bash
cargo test --lib types::bug_tests
cargo test --lib client::bug_tests
cargo test --lib xmlrpc::client_tests
cargo test --lib output::resources::bug_tests
cargo test --lib commands::bug::list_tests
cargo test --lib commands::bug::my_tests
cargo test --lib commands::bug::search_tests
cargo test --lib commands::bug::view_tests
cargo test --lib commands::query_tests
cargo test --test integration custom
```

- [ ] Run format and lint:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] If the focused integration filter finds no tests by name, run the exact
  updated integration test names directly or run `cargo test --test integration`
  before finishing.
- [ ] Review `docs/bzr-cli.md` examples for consistency with gh-style JSON
  projection: `id` appears only when requested.

---

## Out of Scope

- Arbitrary non-`cf_*` extension-field preservation.
- Server-side validation that a given custom field exists.
- New custom-field filters beyond current Bugzilla query passthrough support.
- Bug create/update support for custom fields.
- Changes to comment, attachment, product, user, or field output.
- Importing Bugzilla UI `columnlist` display metadata from `buglist.cgi` URLs.
