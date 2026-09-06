# Implementation plan — `bzr field list` name enumeration (issue #718)

**Goal.** Give `bzr field list` a no-argument form that enumerates every bug field name a
`--field` / `--field-json` write will accept, each row marked with why it is accepted.

**Architecture.** `src/cli/field.rs` makes the `List` positional optional. `src/commands/field.rs`
dispatches on its presence: `Some` keeps today's legal-values path untouched; `None` connects,
calls the existing `BugzillaClient::bug_field_names()`, folds the result together with
`BUG_FIELDS` through a new `accepted_bug_fields()` in the module that already owns the
`--field` accept rule (`src/commands/runtime/shared/field_catalogue.rs`), and prints through a
new writer in `src/output/resources/field.rs`. The row type and its schema-key list live in
`src/types/field.rs` beside `FieldValue`.

**Tech stack.** Rust 2021, tokio, clap derive, serde, tabled, wiremock for HTTP mocking, bash
for the functional harness.

## Global Constraints

Transcribed from the spec and `CLAUDE.md`:

- Unit tests live in sibling `<name>_tests.rs` files linked with
  `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`. Inline `mod tests { … }` in `src/` is
  rejected by `make check-test-layout`. Sibling files start with the file-level inner attribute
  the lint needs (`#![expect(clippy::unwrap_used)]`, or the combined form the original used);
  omit it where the tests do not trigger the lint.
- User-facing output goes through `Writers` (`w.out` / `w.err`) and the output helpers. Never
  `println!` / `eprintln!`. Never add `#[expect(clippy::print_stdout)]` or
  `#[expect(clippy::print_stderr)]` in `src/`.
- Clippy pedantic, `-D warnings`. `unwrap_used` denied; `expect_used` and `allow_attributes`
  warned.
- `SCHEMA_VERSION` is live (3.0.0 shipped in v0.9.0). Under ADR 0007 an additive result type is
  a **patch** bump: 3.0.2 → 3.0.3. **The bump is not a one-line change.** Eleven files outside
  `src/` pin the literal and match it exactly; `rg -n '3\.0\.2'` over the tree is the way to
  find them, and all of them must move in the same commit or the functional suite goes red
  (16 failures, one cause). They are: `tests/functional/phases/{08e-bugs-restricted-access,
  18a-json-envelope,18c-skills-install,18d-dependency-analysis}.sh`, `README.md`,
  `docs/bzr-cli.md`, `content/skills/bzr-reference/reference/{commands,json-recipes}.md`,
  `content/skills/bzr-dependency-analysis/scripts/collect.py` (the `BZR_SCHEMA_VERSION`
  constant, compared with `!=` at `:421`), and that skill's
  `tests/fixtures/recording_runner.py` and `tests/test_collect.py`. Touching
  `content/skills/` means `make skills-test` becomes a required guardrail for this change.
- A new `--json` shape needs five coupled updates: the schema file, the **sorted** `SCHEMAS`
  registry, a conformance case, the docs schema list, and the input parser-key drift check
  (not applicable here — output shape). Only the first three are gated by `schema_tests`. The
  **docs schema list is not gated by anything** — no test, `tools/` script, Makefile target or
  CI step reads `docs/bzr-cli.md`'s "Available schemas" paragraph — so skipping it fails
  silently and reaches the published reference. It is checked by hand, in Task 1's acceptance
  criteria.
- Adding to the CLI requires updating the `## Command Tree` in `docs/bzr-cli.md`. No new long
  flag is added, so `ROOT_GLOBALS` in `agent-skills/tests/flag-drift-check.sh` is untouched.
- `flag-drift-check` defaults to `BZR_BIN:-bzr` and resolves the *installed* binary from `PATH`,
  reporting confident drift in both directions against a stale binary. Always run it as
  `BZR_BIN="$PWD/target/debug/bzr"`, or through `make skills-test`, which sets it.
- Guardrails, run **bare** — no `| tail`, no `>/dev/null`, no `|| true`: `make lint`,
  `make test`. Iterate with `make test-one T=<substring>` and `make test-fast`. Never bare
  `cargo test`.
- Functional phase scripts **are** linted in CI, contrary to a widely repeated claim: `make lint`
  includes `check-shell` (`Makefile:113`), which runs `shellcheck -s bash` and `bash -n` over
  `tests/functional/phases/*.sh` (`Makefile:149-150`), and `.github/workflows/ci.yml:270` runs
  `make check-shell` while `ci.yml:47` runs `make check-functional-test-ids` — the latter pins
  the canonical `test_begin "<slug>" "<description>"` single-line shape and the
  lowercase-hyphen slug charset. What is **not** covered is `shfmt`: `Makefile:151-152` applies
  it to `install.sh` and `tools/*.sh` only, so the phase scripts' 4-space indent is not a
  formatting failure and a bare `shfmt` on one will mislead. `test_pass` / `test_fail` /
  `test_skip` are the only counter-moving primitives, and every fixture guard needs an `else`
  that calls one of them.
- `docs/adr/README.md` is **not** edited: this is a dispatched run and the ADR index is not
  CI-coupled for this batch. Report `index row pending`.

Expected implementation size: 450–650 changed lines (M) — derived from the file map below: six
substantive source files totalling ~155 changed lines plus four one-line edits
(`src/types/mod.rs`, `src/output/mod.rs`, `src/commands/schema.rs`,
`src/commands/runtime/shared/mod.rs`), seven sibling test files totalling ~230, one new 25-line
schema, and ~140 lines of docs and functional phase script.

## Deferrals carried from design review

None. (Populated if the design review disposes any finding as `deferred-tracked`.)

## File map

**Created**

| path | responsible for |
|------|-----------------|
| `schemas/field-name.json` | the published `field-name` output contract |

**Modified**

| path | responsible for |
|------|-----------------|
| `src/types/field.rs` | `FieldName`, `FieldNameSource`, `FIELD_NAME_FIELDS` |
| `src/types/field_tests.rs` | their serialization |
| `src/types/mod.rs` | re-exporting `FieldName` / `FieldNameSource` on line 33 |
| `src/commands/runtime/shared/field_catalogue.rs` | `accepted_bug_fields()`; `undeclared()` wording |
| `src/commands/runtime/shared/field_catalogue_tests.rs` | union correctness + the agreement test |
| `src/output/resources/field.rs` | `write_field_names()` |
| `src/output/resources/field_tests.rs` | table and JSON rendering |
| `src/cli/field.rs` | optional positional; `List` **and** `Aliases` doc comments |
| `src/cli/mod.rs` | the `Field` group doc comment (`bzr field --help`) |
| `src/cli/field_tests.rs` | both parse shapes; replaces `parse_field_list_requires_name` |
| `src/cli/mod_tests.rs` | its one `FieldAction::List` destructuring site (line 1339) |
| `src/commands/field.rs` | dispatch on the positional |
| `src/commands/field_tests.rs` | both command paths against wiremock; five `List` sites |
| `src/commands/runtime/shared/mod.rs` | re-export `accepted_bug_fields` |
| `src/commands/schema.rs` | `"field-name"` in the sorted registry |
| `src/commands/schema_tests.rs` | conformance case |
| `src/output/mod.rs` | `SCHEMA_VERSION` bump |
| `docs/bzr-cli.md` | command tree, `field list` section, projection table, schema list, and the stale `--field` discovery text at :1108–1116 |
| `tests/functional/phases/05-fields-classifications.sh` | listing coverage incl. credentialless |
| `tests/functional/phases/08g-bug-arbitrary-fields.sh` | rejection wording + live agreement oracle |

## Task 1 — The row type and its schema

**Interfaces produced** (later tasks rely on exactly these):

```rust
// src/types/field.rs

/// Why a bug field name is accepted by `--field` / `--field-json` (ADR 0062).
///
/// `as_str` is the single definition of the three spellings: serde serializes
/// through it via `into`, and the table writer calls it directly, so the JSON
/// and table output cannot name a source differently. `schemas/field-name.json`
/// pins the same three values and is checked by
/// `field_name_source_enum_is_closed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "&'static str")]
pub enum FieldNameSource {
    Server,
    Bzr,
    Both,
}

impl FieldNameSource {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldNameSource::Server => "server",
            FieldNameSource::Bzr => "bzr",
            FieldNameSource::Both => "both",
        }
    }
}

impl From<FieldNameSource> for &'static str {
    fn from(source: FieldNameSource) -> Self {
        source.as_str()
    }
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct FieldName {
    pub name: String,
    pub source: FieldNameSource,
}

pub const FIELD_NAME_FIELDS: &[&str] = &["name", "source"];
```

`#[serde(into = "&'static str")]` requires the type to be `Clone` (it is `Copy`) and needs the
`From` impl above; that combination is what removes the second copy of the strings rather than
merely pinning it with a test. Neither type derives `Deserialize`: nothing in production or in
the tests parses this shape back, and adding it would reintroduce a second spelling through
`rename_all`.

**Interfaces consumed:** none.

**Where it fits:** every later task refers to `FieldName` / `FieldNameSource`.

### Verification

- Contract: `FieldNameSource` serializes to `"server"` / `"bzr"` / `"both"`.
  `Mode: focused-test` — `src/types/field_tests.rs::field_name_source_serializes_lowercase`.
  Red before the enum exists: the test file does not compile (`cannot find type
  FieldNameSource`). Green: `make test-one T=field_name_source_serializes_lowercase`.
- Contract: `schemas/field-name.json` accepts a serialized `FieldName`.
  `Mode: focused-test` — `src/commands/schema_tests.rs::field_name_conforms`.
  Red before the schema is registered: `schema_for("field-name")` panics with an unknown
  schema. Green: `make test-one T=field_name_conforms`.
- Contract: `SCHEMAS` stays sorted and unique.
  `Mode: focused-test` — `registry_is_sorted_and_unique` in `src/commands/schema_tests.rs`.
  Red if `"field-name"` is appended out of order. Green: `make test-one T=schema`.
- Contract: the docs "Available schemas" list names the new schema.
  `Mode: task-test-not-applicable` — nothing executable reads that paragraph
  (`docs/bzr-cli.md`, the "Available schemas" list); it is prose in the published reference
  with no consumer, and a prose snapshot test is explicitly disallowed. It is an acceptance
  criterion below instead, checked by hand where the edit is made.

### Steps

1. In `src/types/field.rs`, below `FIELD_VALUE_FIELDS`, add `FieldNameSource` with its
   `as_str` and `From` impls, `FieldName`, and `FIELD_NAME_FIELDS`, exactly as written in the
   Interfaces block above.
2. In `src/types/mod.rs`, change line 33 from
   `pub use field::{FieldValue, StatusTransition};` to
   `pub use field::{FieldName, FieldNameSource, FieldValue, StatusTransition};`.
   `FIELD_VALUE_FIELDS` is deliberately **not** re-exported there — `src/commands/field.rs`
   reaches it as `crate::types::field::FIELD_VALUE_FIELDS` — so leave `FIELD_NAME_FIELDS` out
   of this line too and reach it the same way, matching the existing convention.
3. Create `schemas/field-name.json`:

   ```json
   {
     "$schema": "https://json-schema.org/draft/2020-12/schema",
     "$id": "https://github.com/randomparity/bzr/schemas/field-name.json",
     "title": "FieldName",
     "description": "A bug field name that `bzr bug create` / `bzr bug update` accept for `--field` / `--field-json`, as emitted by `bzr field list` with no positional argument.",
     "type": "object",
     "properties": {
       "name": {
         "type": "string",
         "description": "The field name a --field write accepts."
       },
       "source": {
         "type": "string",
         "enum": ["server", "bzr", "both"],
         "description": "Why the name is accepted: `server` = the connected server's field/bug catalogue declares it; `bzr` = bzr models it as a canonical REST bug field; `both` = both."
       }
     },
     "required": ["name", "source"],
     "additionalProperties": false
   }
   ```

4. In `src/commands/schema.rs`, add `"field-name",` to the `SCHEMAS` list **immediately before**
   `"field-value",`. The list is sorted and a test asserts it.
5. In `src/output/mod.rs`, change `pub const SCHEMA_VERSION: &str = "3.0.2";` to `"3.0.3"`.
6. In `src/types/field_tests.rs`, change the first `use` line from
   `use super::{FieldValue, StatusTransition, FIELD_VALUE_FIELDS};` to
   `use super::{FieldName, FieldNameSource, FieldValue, StatusTransition, FIELD_NAME_FIELDS, FIELD_VALUE_FIELDS};`
   and add these two tests. The file already carries `#![expect(clippy::unwrap_used)]`, so
   `unwrap()` is the idiom here — do not introduce `.expect(...)`.

   ```rust
   #[test]
   fn field_name_source_serializes_lowercase() {
       for (source, expected) in [
           (FieldNameSource::Server, "server"),
           (FieldNameSource::Bzr, "bzr"),
           (FieldNameSource::Both, "both"),
       ] {
           let row = FieldName {
               name: "whiteboard".into(),
               source,
           };
           let value = serde_json::to_value(&row).unwrap();
           assert_eq!(value["name"], "whiteboard");
           assert_eq!(value["source"], expected);
       }
   }

   /// Mirrors `field_value_fields_matches_serialized_keys` above: the projection
   /// key list and the serialized object must not drift apart, because
   /// `--fields` validates against the former and projects the latter.
   #[test]
   fn field_name_fields_matches_serialized_keys() {
       let row = FieldName {
           name: "status_whiteboard".into(),
           source: FieldNameSource::Server,
       };
       let value = serde_json::to_value(&row).unwrap();
       let serialized: std::collections::BTreeSet<String> =
           value.as_object().unwrap().keys().cloned().collect();
       let declared: std::collections::BTreeSet<String> = FIELD_NAME_FIELDS
           .iter()
           .map(|s| (*s).to_string())
           .collect();
       assert_eq!(serialized, declared);
   }
   ```
7. In `src/commands/schema_tests.rs`, beside the existing `field_value_conforms` test, add:

   ```rust
   #[test]
   fn field_name_conforms() {
       use crate::types::field::{FieldName, FieldNameSource};
       let row = FieldName {
           name: "status_whiteboard".to_string(),
           source: FieldNameSource::Server,
       };
       assert_conforms("field-name", &to_value(&row));
   }
   ```

   `assert_conforms` validates top-level keys only, so also assert the enum explicitly:

   ```rust
   #[test]
   fn field_name_source_enum_is_closed() {
       let schema = schema_for("field-name");
       let variants = &schema["properties"]["source"]["enum"];
       assert_eq!(variants, &serde_json::json!(["server", "bzr", "both"]));
   }
   ```

8. In `docs/bzr-cli.md`, find the "Available schemas:" paragraph (the one listing `field-value`
   among the read shapes) and insert `field-name` immediately before `field-value` in that
   sentence.
9. Run `make test-one T=field_name` bare. Expect the three new tests to pass and no other output
   than the quiet summary.
10. Run `make test-one T=schema` bare. Expect green — this proves the registry order and the
    docs list did not drift.
11. `git add -A && git commit -m "feat(field): publish the field-name output shape"`.

### Acceptance criteria

- `cargo run -- schema field-name` prints the schema.
- `cargo run -- schema` lists `field-name` between `error` and `field-value`.
- `SCHEMA_VERSION` is `3.0.3`.
- **Checked by hand, because nothing gates it:** `docs/bzr-cli.md`'s "Available schemas"
  paragraph now names `field-name`, in the read-shapes list beside `field-value`. Confirm with
  `grep -c 'field-name' docs/bzr-cli.md` before moving on — a skipped edit here is silent.

## Task 2 — `accepted_bug_fields()` and the re-pointed rejection message

**Interfaces produced:**

```rust
// src/commands/runtime/shared/field_catalogue.rs
pub(crate) fn accepted_bug_fields(declared: &[String]) -> Vec<crate::types::FieldName>
```

Returns one row per unique name, sorted ascending by `name`, over the union of `declared` and
every `BUG_FIELDS` canonical name.

**Interfaces consumed:** `FieldName`, `FieldNameSource` (Task 1);
`crate::types::bug::BUG_FIELDS` with `BugField::canonical(self) -> &'static str`, confirmed
present at `src/types/bug/fields.rs:176` and `:63`; the module-private
`is_bzr_known_bug_field(key: &str) -> bool`, already in this file.

**Where it fits:** Task 4's command handler is this function's only production caller. The
function lives here, not in `commands/field.rs`, because this module already owns the rule that
decides what `--field` accepts, and co-locating them is what keeps the listing and the validator
from drifting.

### Verification

- Contract: the union's `source` marking is correct for a server-only name, a bzr-only name, and
  an overlapping name. `Mode: focused-test` —
  `field_catalogue_tests.rs::accepted_bug_fields_marks_each_source`. Red before the function
  exists: compile error, `cannot find function accepted_bug_fields`. Green:
  `make test-one T=accepted_bug_fields_marks_each_source`.
- Contract: everything `accepted_bug_fields` lists, `validate_bug_fields` accepts (acceptance
  criterion 2). `Mode: focused-test` —
  `field_catalogue_tests.rs::everything_listed_is_accepted`. Red if a row is emitted that
  neither source backs: `validate_bug_fields` returns `Err(InputValidation)` and the test fails
  on the unwrapped error. Green: `make test-one T=everything_listed_is_accepted`.
- Contract: the rejection message names `bzr field list`. `Mode: focused-test` —
  `field_catalogue_tests.rs` — extend the existing test that asserts the `undeclared` message
  (search the file for `server capabilities`). Red: it still asserts the old string. Green:
  `make test-one T=field_catalogue`.

### Steps

1. Open `src/commands/runtime/shared/field_catalogue.rs`. Add to the imports at the top:

   ```rust
   use std::collections::BTreeMap;
   use crate::types::{FieldName, FieldNameSource};
   ```

   Line 13 of that file is the single-item `use std::collections::BTreeSet;` — there is no
   braced form to extend — so either widen it to `use std::collections::{BTreeMap, BTreeSet};`
   or add the separate `use std::collections::BTreeMap;` the block above shows. Either is fine;
   pick one and do not leave both.
2. Below `is_bzr_known_bug_field`, add:

   ```rust
   /// Every bug field name `--field` / `--field-json` accepts, given the names the
   /// server's catalogue declares, each marked with why it is accepted.
   ///
   /// This is the listing half of the contract `validate_bug_fields` enforces: the two
   /// read the same two sources — the catalogue and `BUG_FIELDS` via
   /// [`is_bzr_known_bug_field`] — so a name this function emits is a name that function
   /// accepts. Keeping them in one module is what makes that agreement structural rather
   /// than a comment (ADR 0062).
   ///
   /// `BTreeMap` gives sorted, deduplicated output in one pass and collapses a name
   /// present in both sources into a single `Both` row.
   fn accepted_bug_fields_map(declared: &[String]) -> BTreeMap<&str, FieldNameSource> {
       let mut rows: BTreeMap<&str, FieldNameSource> = BTreeMap::new();
       for name in declared {
           rows.insert(name.as_str(), FieldNameSource::Server);
       }
       for field in crate::types::bug::BUG_FIELDS {
           rows.entry(field.canonical())
               .and_modify(|source| *source = FieldNameSource::Both)
               .or_insert(FieldNameSource::Bzr);
       }
       rows
   }

   pub(crate) fn accepted_bug_fields(declared: &[String]) -> Vec<FieldName> {
       accepted_bug_fields_map(declared)
           .into_iter()
           .map(|(name, source)| FieldName {
               name: name.to_string(),
               source,
           })
           .collect()
   }
   ```

   Note the `and_modify` arm: a name already inserted from `declared` is upgraded to `Both`
   rather than overwritten, which is what makes an overlapping name one row and not two.
3. Replace the whole doc comment and body of `undeclared()` with:

   ```rust
   /// Point at the command that answers "what can I set here".
   ///
   /// `bzr field list` with no argument enumerates the whole accepted set — the server's
   /// catalogue names and the REST names bzr models — which is exactly the set this
   /// function guards (ADR 0062). Earlier wording named `bzr server capabilities` and
   /// stopped at "custom fields", because before #718 no command could show the rest.
   fn undeclared(key: &str) -> BzrError {
       BzrError::input_field(
           format!(
               "--field: this server does not declare a field named '{key}'; \
                run `bzr field list` to see every field name this server accepts"
           ),
           "--field",
           Some(key.to_string()),
       )
   }
   ```

4. Open `src/commands/runtime/shared/field_catalogue_tests.rs`. Search it for
   `server capabilities` and update every assertion of the rejection message to expect
   `bzr field list` instead. Do not weaken an exact-string assertion into a substring one.
5. In the same file, add:

   ```rust
   #[test]
   fn accepted_bug_fields_marks_each_source() {
       // `status_whiteboard` is a catalogue-only internal column name; `whiteboard` is a
       // REST name bzr models that the catalogue does not declare; `keywords` is in both.
       let declared = vec![
           "status_whiteboard".to_string(),
           "keywords".to_string(),
           "status_whiteboard".to_string(),
       ];
       let rows = super::accepted_bug_fields(&declared);

       let find = |name: &str| {
           rows.iter()
               .filter(|row| row.name == name)
               .collect::<Vec<_>>()
       };

       let sw = find("status_whiteboard");
       assert_eq!(sw.len(), 1, "a duplicate in `declared` must yield one row");
       assert_eq!(sw[0].source, crate::types::FieldNameSource::Server);

       let wb = find("whiteboard");
       assert_eq!(wb.len(), 1);
       assert_eq!(wb[0].source, crate::types::FieldNameSource::Bzr);

       let kw = find("keywords");
       assert_eq!(kw.len(), 1);
       assert_eq!(kw[0].source, crate::types::FieldNameSource::Both);

       let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
       let mut sorted = names.clone();
       sorted.sort_unstable();
       assert_eq!(names, sorted, "rows must be sorted by name");
   }

   #[test]
   fn accepted_bug_fields_with_empty_catalogue_is_bug_fields_only() {
       let rows = super::accepted_bug_fields(&[]);
       assert_eq!(rows.len(), crate::types::bug::BUG_FIELDS.len());
       assert!(rows
           .iter()
           .all(|row| row.source == crate::types::FieldNameSource::Bzr));
   }
   ```

6. In the same file, add the agreement test, using the file's existing helpers rather
   than a second mock — `write_config` / `ctx_for` at lines 22–34, `mount_catalogue` / `setup`
   at lines 46–70:

   ```rust
   /// Acceptance criterion 2 of issue #718, executable: every name the listing emits is a
   /// name the validator accepts. The oracle discriminates — a row backed by neither
   /// source makes `validate_bug_fields` return `InputValidation`, and the `expect`
   /// below fails naming the exact key.
   ///
   /// `expected_calls` is 1: the listed set contains catalogue-only names, which are not
   /// `is_bzr_known_bug_field`, and the fresh config carries no cached names, so the
   /// validator probes exactly once.
   #[tokio::test]
   async fn everything_listed_is_accepted() {
       let declared_names = ["status_whiteboard", "short_desc", "keywords", "cf_example"];
       let (server, tmp, client) = setup(&declared_names, 1).await;
       let config_path = write_config(&tmp, &server.uri(), "");

       let declared: Vec<String> = declared_names.iter().map(|n| (*n).to_string()).collect();
       let listed: BTreeSet<String> = super::accepted_bug_fields(&declared)
           .into_iter()
           .map(|row| row.name)
           .collect();
       // The union is strictly wider than either source alone, so this test is
       // exercising the union rather than a degenerate case.
       assert!(listed.len() > crate::types::bug::BUG_FIELDS.len());
       assert!(listed.len() > declared.len());

       validate_bug_fields(&client, &ctx_for(&config_path), &listed)
           .await
           .expect("every listed field name must be accepted by the --field validator");
   }
   ```

   `validate_bug_fields` is already imported at the top of the file via
   `use super::{validate_bug_fields, BugzillaClient};`; `BTreeSet` is already imported too.
   Add `accepted_bug_fields` to that `use super::{…}` line, or call it as
   `super::accepted_bug_fields` as written above — either is consistent with the file.
7. Run `make test-one T=field_catalogue` bare. Expect green.
8. Confirm the oracle bites: temporarily change `or_insert(FieldNameSource::Bzr)` in step 2 to
   also insert a literal row for a name neither source backs — e.g. add
   `rows.insert("cf_not_real_at_all", FieldNameSource::Bzr);` before the return — then run
   `make test-one T=everything_listed_is_accepted` bare and confirm it **fails** naming
   `cf_not_real_at_all`. Revert the line and re-run to confirm green. Record both observations.
9. `git add -A && git commit -m "feat(field): derive the accepted write-field set in one place"`.

### Acceptance criteria

- `accepted_bug_fields(&[])` returns 28 rows, all `Bzr`.
- The rejection message names `bzr field list` and no test still asserts
  `bzr server capabilities` for it.
- The controlled-fault observation in step 8 is recorded (red, then green after revert).

## Task 3 — The output writer

**Interfaces produced:**

```rust
// src/output/resources/field.rs
pub fn write_field_names<W: Write + ?Sized>(
    names: &[crate::types::FieldName],
    format: OutputFormat,
    projection: &FieldProjection,
    table_width: Option<usize>,
    out: &mut W,
)
```

**Interfaces consumed:** `FieldName` (Task 1); the file's existing
`write_formatted_projected` and `write_table_records` imports, both already in scope.

**Where it fits:** Task 4 calls this.

### Verification

- Contract: the table has a `NAME` / `SOURCE` header and one row per entry.
  `Mode: focused-test` — `src/output/resources/field_tests.rs::write_field_names_table`.
  Red before the function exists: compile error. Green:
  `make test-one T=write_field_names_table`.
- Contract: `--fields name` projects to `{name}` only. `Mode: focused-test` —
  `src/output/resources/field_tests.rs::write_field_names_json_projects`. Red before the
  function exists: compile error. Green: `make test-one T=write_field_names_json_projects`.

### Steps

1. In `src/output/resources/field.rs`, add the header constant beside the existing two:

   ```rust
   const FIELD_NAME_HEADERS: &[&str] = &["NAME", "SOURCE"];
   ```

2. Add the writer below `write_field_values`:

   ```rust
   /// Render the bug field names a `--field` write accepts. `source` says why each one is
   /// accepted; see ADR 0062.
   pub fn write_field_names<W: Write + ?Sized>(
       names: &[FieldName],
       format: OutputFormat,
       projection: &FieldProjection,
       table_width: Option<usize>,
       out: &mut W,
   ) {
       write_formatted_projected(names, format, projection, out, |names, out| {
           write_table_records(
               FIELD_NAME_HEADERS,
               names
                   .iter()
                   .map(|row| vec![row.name.clone(), row.source.as_str().to_string()]),
               table_width,
               out,
           );
       });
   }
   ```

   The table cell comes from `FieldNameSource::as_str()` (Task 1), which is the same
   definition serde serializes through, so the writer holds no second copy of the three
   strings. A local `field_name_source_label` match would be a third hand-maintained copy —
   exactly the drift ADR 0062 rejects twice — so do not add one.

3. Extend the file's `use crate::types::{FieldValue, OutputFormat};` line to
   `use crate::types::{FieldName, FieldNameSource, FieldValue, OutputFormat};`.
4. In `src/output/resources/field_tests.rs`, add two tests. Both are mechanical rendering
   checks, so write them in the file's established shape rather than from a transcript here:
   add `write_field_names` to the `use super::{…}` line, `FieldName` and `FieldNameSource` to
   the `use crate::types::{…}` line, and `FIELD_NAME_FIELDS` as
   `use crate::types::field::FIELD_NAME_FIELDS;` — Task 1 step 2 deliberately keeps it out of
   the `crate::types` re-export, matching `FIELD_VALUE_FIELDS`, so that is the only path to it.
   Then add a `capture_names(format, projection, rows)`
   helper alongside the existing `capture_values` (line 6), which already shows the
   `Vec::new()` → writer → `String::from_utf8(buf).unwrap()` pattern and the
   `crate::validation::fields::FieldProjection::none()` path.

   - `write_field_names_table` — over rows
     `[("keywords", Both), ("status_whiteboard", Server)]` at `OutputFormat::Table`, assert the
     output contains `NAME`, `SOURCE`, `status_whiteboard`, `server`, and `both`.
   - `write_field_names_json_projects` — over one `("whiteboard", Bzr)` row at
     `OutputFormat::Json` with
     `FieldProjection::resolve(Some("name"), None, FIELD_NAME_FIELDS).unwrap()`, assert the
     output contains `whiteboard` and does **not** contain `source`. The negative half is the
     assertion that bites: without it the test passes whether or not the projection applied.
   No third test is needed for the `source` spelling: Task 1 makes `as_str()` the single
     definition serde serializes through, so `field_name_source_serializes_lowercase` already
     pins the table cell too.
5. Run `make test-one T=write_field_names` bare. Expect both tests green.
6. `git add -A && git commit -m "feat(field): render the accepted field-name listing"`.

### Acceptance criteria

- Table output carries `NAME` and `SOURCE` headers.
- `--fields name` output contains no `source` key.

## Task 4 — The CLI surface and the command handler

**Interfaces produced:** `FieldAction::List { name: Option<String>, projection: ProjectionArgs }`.

**Interfaces consumed:** `accepted_bug_fields` (Task 2), reached as
`super::runtime::shared::accepted_bug_fields`; `write_field_names` (Task 3);
`crate::types::field::FIELD_NAME_FIELDS` (Task 1); the existing
`BugzillaClient::bug_field_names(&self) -> Result<Vec<String>>`, confirmed `pub(crate)` at
`src/client/resources/field.rs:82`; the existing
`crate::commands::runtime::shared::connect_and_configure(ctx)`;
`ProjectionArgs { fields, exclude_fields }` (`src/cli/fields.rs`, both `pub Option<String>`,
derives `Default`).

**Where it fits:** the last source task; everything after it is docs and functional coverage.

### Verification

- Contract: `bzr field list` parses with no positional. `Mode: focused-test` —
  `src/cli/field_tests.rs::parse_field_list_without_a_name_is_the_listing_form`, replacing
  `parse_field_list_requires_name`. Red before the type changes: clap returns
  `MissingRequiredArgument` and `field_action` unwraps a parse error. Green:
  `make test-one T=parse_field_list`.
- Contract: the no-argument form prints the union from one `field/bug` request.
  `Mode: focused-test` — `src/commands/field_tests.rs::field_list_no_argument_lists_names`.
  Red before the handler branch exists: the `None` arm does not compile / the test sees the
  legal-values output. Green: `make test-one T=field_list_no_argument_lists_names`.
- Contract: `field list <name>` is unchanged. `Mode: focused-test` — the file's existing
  `field list` tests, which must pass untouched. Green: `make test-one T=field`.
- Contract: `--fields bogus` on the no-argument form exits 7. `Mode: focused-test` —
  `src/commands/field_tests.rs::field_list_no_argument_rejects_unknown_projection`. Red before
  the branch validates against `FIELD_NAME_FIELDS`: it validates against `FIELD_VALUE_FIELDS`
  and accepts `sort_key`, which is not a key of this shape. Green:
  `make test-one T=field_list_no_argument_rejects_unknown_projection`.

### Steps

1. In `src/cli/field.rs`, replace the `List` variant's doc comment and `name` field:

   ```rust
   /// List the bug field names this server accepts, or the legal values of one field.
   ///
   /// With no argument, prints every bug field name `bzr bug create` and
   /// `bzr bug update` accept for `--field` / `--field-json`, with a `source`
   /// column saying why each is accepted: `server` when the connected
   /// server's field catalogue declares it, `bzr` when bzr models it as a
   /// canonical REST bug field, `both` when both do. Bugzilla's catalogue
   /// reports internal column names for several built-ins
   /// (`status_whiteboard`, `short_desc`, `rep_platform`), while the write
   /// API takes the REST spellings (`whiteboard`, `summary`, `platform`);
   /// both are accepted and both are listed.
   ///
   /// With a field name, prints every value the configured server accepts
   /// for that field. Common aliases (`status`, `severity`, `priority`,
   /// `resolution`, ...) are resolved automatically to their underlying
   /// field names; the canonical names also work. Use this to discover legal
   /// values before passing `--status`, `--priority`, etc. to
   /// `bzr bug create` or `bzr bug update`.
   ///
   /// Examples:
   ///
   ///   bzr field list
   ///   bzr field list --json
   ///   bzr field list status
   ///   bzr field list priority --json
   ///   bzr field list bug_severity
   ///
   /// See bzr-field-aliases(1) for the alias table and
   /// bzr-bug-create(1) / bzr-bug-update(1) for the commands that
   /// consume these values.
   #[command(verbatim_doc_comment)]
   List {
       /// Field name (e.g. status, priority, severity, resolution). Omit it to
       /// list the field names this server accepts instead. Common aliases are
       /// resolved automatically (status -> `bug_status`, severity ->
       /// `bug_severity`, etc.)
       name: Option<String>,
       #[command(flatten)]
       projection: crate::cli::ProjectionArgs,
   },
   ```

1a. Two adjacent doc comments go stale with the same edit; update both.

   - `src/cli/field.rs:22-23`, inside the `Aliases` variant, reads "See bzr-field-list(1) to
     enumerate the legal values of one field." Reword it to cover both forms, and add that
     aliases apply to the **named** form only: `accepted_bug_fields` does no alias resolution,
     whereas `get_field_values` calls `resolve_field_alias` (`src/client/resources/field.rs:48`).
   - `src/cli/mod.rs:488-501`, the `Field` group doc comment — what `bzr field --help` prints —
     opens "Discover valid values for Bugzilla bug fields (status, priority, etc.)" and its
     examples are `bzr field aliases`, `bzr field list status`, `bzr field list priority --json`.
     Reword the opening to name both discovery jobs (which *names* a server accepts, and which
     *values* one field accepts) and add `bzr field list` to the examples.

2. In `src/commands/field.rs`, replace the `FieldAction::List` arm with:

   ```rust
   FieldAction::List { name, projection } => match name {
       Some(name) => {
           let projection = crate::validation::fields::projection_for(
               format,
               projection.fields.as_deref(),
               projection.exclude_fields.as_deref(),
               crate::types::field::FIELD_VALUE_FIELDS,
               w.err,
           )?;
           let client = super::runtime::shared::connect_and_configure(ctx).await?;
           let values = client.get_field_values(name).await?;
           if values.is_empty() && format == OutputFormat::Table {
               let _ = writeln!(w.out, "No values for field '{name}'.");
           } else {
               write_field_values(&values, format, &projection, w.table_width(), w.out);
           }
       }
       None => {
           let projection = crate::validation::fields::projection_for(
               format,
               projection.fields.as_deref(),
               projection.exclude_fields.as_deref(),
               crate::types::field::FIELD_NAME_FIELDS,
               w.err,
           )?;
           let client = super::runtime::shared::connect_and_configure(ctx).await?;
           // Always a fresh probe: `ServerConfig.bug_field_names` is a validator
           // fast path whose staleness is harmless there but would make a
           // listing disagree with the server (ADR 0062).
           let declared = client.bug_field_names().await?;
           let names = super::runtime::shared::accepted_bug_fields(&declared);
           write_field_names(&names, format, &projection, w.table_width(), w.out);
       }
   },
   ```

3. Update the imports. In `src/commands/field.rs`, add `write_field_names` to the
   `use crate::output::resources::field::{…}` line. `field_catalogue` is a **private** `mod` in
   `src/commands/runtime/shared/mod.rs`, re-exported selectively, so widen nothing: add
   `accepted_bug_fields` to the existing line

   ```rust
   pub(crate) use field_catalogue::{connect_and_validate_bug_fields, validate_bug_fields};
   ```

   making it

   ```rust
   pub(crate) use field_catalogue::{
       accepted_bug_fields, connect_and_validate_bug_fields, validate_bug_fields,
   };
   ```

   which is the path step 2's handler already calls.
4. Update every existing construction and destructuring site of `FieldAction::List`, all five
   of them, since `name` is now `Option<String>`:

   - `src/commands/field_tests.rs:12` — the `list_with` helper's `name:` field (the
     `FieldAction::List {` line above it is 11): `name: Some(name.to_string()),`.
   - `src/commands/field_tests.rs` lines 53, 103, 133, 166 — each inline
     `name: "…".to_string(),` becomes `name: Some("…".to_string()),`. (The enclosing
     `let action = FieldAction::List {` lines are 52, 102, 132, 165.)
   - `src/cli/field_tests.rs:41` and `:49` — `assert_eq!(name, "status")` becomes
     `assert_eq!(name.as_deref(), Some("status"))`, and likewise for `"bug_severity"`.
   - `src/cli/mod_tests.rs:1339` — `=> assert_eq!(name, "status")` becomes
     `=> assert_eq!(name.as_deref(), Some("status"))`.

5. **Replace** the existing `parse_field_list_requires_name` test in `src/cli/field_tests.rs`.
   It currently asserts the contract this change deliberately removes:

   ```rust
   #[test]
   fn parse_field_list_requires_name() {
       assert_eq!(
           parse_error_kind(&["bzr", "field", "list"]),
           ErrorKind::MissingRequiredArgument
       );
   }
   ```

   becomes

   ```rust
   /// The no-argument form is the field-name listing (issue #718), so the missing
   /// positional that used to be a usage error is now a valid invocation.
   #[test]
   fn parse_field_list_without_a_name_is_the_listing_form() {
       match field_action(&["bzr", "field", "list"]) {
           FieldAction::List { name, .. } => assert_eq!(name, None),
           FieldAction::Aliases => panic!("expected List"),
       }
   }
   ```

   Name the second variant explicitly, as the file's other two-variant matches do — clippy runs
   with `-D warnings` and a wildcard arm on a two-variant enum is flagged.
6. In `src/commands/field_tests.rs`, add two tests for the no-argument form, using the file's
   `setup_isolated_env()` / `CapturedIo` / `json_envelope_data` idiom exactly as
   `field_list_returns_values` (line 33) does:

   ```rust
   async fn mount_catalogue_names(mock: &wiremock::MockServer) {
       Mock::given(method("GET"))
           .and(path("/rest/field/bug"))
           .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
               "fields": [{"name": "status_whiteboard"}, {"name": "keywords"}]
           })))
           .mount(mock)
           .await;
   }

   #[tokio::test]
   async fn field_list_no_argument_lists_names() {
       let (mock, _tmp, config_path) = setup_isolated_env().await;
       mount_catalogue_names(&mock).await;

       let action = FieldAction::List {
           name: None,
           projection: ProjectionArgs::default(),
       };
       let mut io = crate::test_helpers::CapturedIo::new();
       let result = super::execute(
           &action,
           &crate::commands::runtime::invocation::CommandContext::new(
               None,
               OutputFormat::Json,
               None,
           )
           .with_config_path_override(Some(config_path.clone())),
           &mut io.writers(),
       )
       .await;
       assert!(result.is_ok(), "no-argument field list: {result:?}");
       let parsed = crate::test_helpers::json_envelope_data(&io.out_str().to_string());
       let rows = parsed.as_array().unwrap();
       let source_of = |name: &str| {
           rows.iter()
               .find(|row| row["name"] == name)
               .map(|row| row["source"].as_str().unwrap().to_string())
       };
       // catalogue-only, bzr-only, and overlapping, in one assertion set.
       assert_eq!(source_of("status_whiteboard").as_deref(), Some("server"));
       assert_eq!(source_of("whiteboard").as_deref(), Some("bzr"));
       assert_eq!(source_of("keywords").as_deref(), Some("both"));
   }

   /// `sort_key` is a valid key of the *named* form and an invalid key of this one, so
   /// this fails if the handler validates against `FIELD_VALUE_FIELDS` by mistake. A
   /// nonsense token would be rejected either way and prove nothing.
   #[tokio::test]
   async fn field_list_no_argument_rejects_unknown_projection() {
       let (mock, _tmp, config_path) = setup_isolated_env().await;
       mount_catalogue_names(&mock).await;

       let action = FieldAction::List {
           name: None,
           projection: ProjectionArgs {
               fields: Some("sort_key".to_string()),
               ..ProjectionArgs::default()
           },
       };
       let mut io = crate::test_helpers::CapturedIo::new();
       let result = super::execute(
           &action,
           &crate::commands::runtime::invocation::CommandContext::new(
               None,
               OutputFormat::Json,
               None,
           )
           .with_config_path_override(Some(config_path.clone())),
           &mut io.writers(),
       )
       .await;
       let err = result.expect_err("sort_key is not a FieldName key");
       assert_eq!(err.exit_code(), 7);
   }
   ```

   `ProjectionArgs` (`src/cli/fields.rs`) derives `Default` and both fields — `fields` and
   `exclude_fields`, each `Option<String>` — are `pub`, so the struct-update syntax above
   compiles as written. `BzrError::exit_code()` returns `EXIT_CODE_INPUT`, which is `7`
   (`src/error.rs:149`).
7. Run `make test-one T=field` bare. Expect every new test green and every pre-existing
   `field list` test still green.
8. Run `make lint` bare. Expect exit 0.
9. `git add -A && git commit -m "feat(field): add a no-argument form to bzr field list"`.

### Acceptance criteria

- `cargo run -- field list` against a server prints a `NAME` / `SOURCE` table.
- `cargo run -- field list status` output is byte-identical to before the change.
- `make lint` and `make test` are green.

## Task 5 — Documentation

**Interfaces consumed:** the CLI surface from Task 4.

### Verification

- Contract: the tree edit introduces no *flag* drift.
  `Mode: focused-test` — `agent-skills/tests/flag-drift-check.sh` with `BZR_BIN` pointed at the
  freshly built debug binary. Be honest about what this proves: the script compares **long
  flags only** (its own header, lines 3–17, and its `binary_flags` extractor at 71–79 — a
  positional never enters the comparison), so it is blind to the `<FIELD_NAME>` →
  `[<FIELD_NAME>]` change by design and its result is identical before and after step 2. It is
  run to prove the edit broke nothing, not to prove the edit happened. Green:
  `BZR_BIN=<abs path to target/debug/bzr> sh agent-skills/tests/flag-drift-check.sh` exits 0.
- Contract: the `--field` documentation no longer asserts that no command can enumerate the
  accepted set. `Mode: focused-test` — the grep pair in step 7 below. Red before step 6:
  `docs/bzr-cli.md` still contains `missing enumeration command`, so the first grep exits 0
  and the check fails. Green: step 7 exits 0. This is the only check in Task 5 that can fail
  on a docs defect, and it exists because the flag-drift check above structurally cannot.
- Contract: the projection table lists the keys each form accepts.
  `Mode: task-test-not-applicable` — the projection table is prose in `docs/bzr-cli.md` with no
  executable consumer; the drift check compares flags, not table cells, and inventing a prose
  snapshot test is explicitly disallowed.

### Steps

1. `cargo build` bare, so `target/debug/bzr` matches HEAD before any drift check runs.
2. In `docs/bzr-cli.md`, in the `## Command Tree`, change

   ```
   │   └── list <FIELD_NAME> [--fields <F>] [--exclude-fields <F>]
   ```

   to

   ```
   │   └── list [<FIELD_NAME>] [--fields <F>] [--exclude-fields <F>]
   ```

3. In the "Valid field names per verb" table, change the `field list` row from

   ```
   | `field list` | `name`, `sort_key`, `is_active`, `can_change_to` |
   ```

   to

   ```
   | `field list <name>` | `name`, `sort_key`, `is_active`, `can_change_to` |
   | `field list` (no argument) | `name`, `source` |
   ```

4. Rewrite the `### bzr field list` section. Keep the existing alias table and its paragraph;
   add above them a description of the no-argument form:

   - what it lists (every bug field name `--field` / `--field-json` accepts);
   - the three `source` values and what each means;
   - the internal-vs-REST name asymmetry, naming the concrete pairs
     `status_whiteboard`/`whiteboard`, `short_desc`/`summary`, `rep_platform`/`platform`,
     `bug_file_loc`/`url`, `blocked`/`blocks`, and stating plainly that bzr does **not** pair
     them in the output — both spellings are listed, both are accepted, and the `source` column
     is the relationship it does record;
   - the three caveats from the spec, in full: (i) a field the server *removes* after its
     names were cached is still accepted on a cache-hit write while this listing, which always
     probes, no longer shows it — the one accepted-but-unlisted case, recorded as a residual by
     ADR 0053; (ii) a field *added* since the listing is a cache miss and re-probes, so it is
     never wrongly rejected; (iii) listed means bzr will not refuse the key, not that Bugzilla
     will honour it — which covers the read-only catalogue names *and* the read-only entries of
     `BUG_FIELDS`, a `--fields` read-projection list, so `id`, `creator`, `creation_time`, and
     `last_change_time` are listed as `source: bzr` and no write can set them;
   - examples:

     ```bash
     bzr field list
     bzr --json field list
     bzr --json field list | jq -r '.data[] | select(.name | startswith("cf_")) | .name'
     ```

5. Confirm the `field-name` entry added to the "Available schemas" paragraph in Task 1 is
   present and in the right place.
5a. **Retitle the `field` group section.** `docs/bzr-cli.md:1554` reads
   ``## `bzr field` -- Field Value Lookup``, and the table of contents links it at `:15` as
   `- [field](#bzr-field----field-value-lookup)`. After this change the section documents name
   enumeration as well as value lookup, so the title names the thing it is not. Change the
   heading to ``## `bzr field` -- Field Name and Value Lookup`` and the TOC entry to
   `- [field](#bzr-field----field-name-and-value-lookup)`. Then confirm no other link breaks:
   `rg -n 'field-value-lookup' docs/ agent-skills/ content` must return nothing — at the time
   of writing `docs/bzr-cli.md:15` is the only reference, so these two edits are the whole of
   it.
6. **Rewrite the stale `--field` discovery paragraph.** `docs/bzr-cli.md` lines 1108–1116
   currently read:

   ```
   ... `bzr server capabilities` lists the custom fields a server
   declares.

   The set bzr accepts is wider than the set it can currently list:
   `server capabilities` shows only custom (`is_custom`) fields, so the non-custom
   catalogue names and the `BUG_FIELDS` REST names are accepted without appearing
   in any listing. `bzr field list <name>` enumerates one named field's legal
   *values* and needs the name up front, so it does not close that gap either.
   Issue #718 tracks the missing enumeration command.
   ```

   Every clause of the second paragraph becomes false when this change ships, and its last
   sentence points at the issue this change closes. Line 1109 is also the advice the rejection
   message is being re-pointed away from in Task 2 step 3, so leaving it makes the document
   disagree with the binary's own error text.

   Replace both with a single paragraph that: points at `bzr field list` (no argument) as the
   command that enumerates the accepted set; states the three caveats from the spec — a field
   the server *removes* after its names were cached is still accepted on a cache-hit write
   while the listing no longer shows it (ADR 0053's recorded residual); a field *added* since
   the listing is a cache miss and re-probes, so it is never wrongly rejected; and listed means
   bzr will not refuse the key, not that Bugzilla will honour it, which covers both the
   read-only catalogue names and the read-only `BUG_FIELDS` entries (`id`, `creator`,
   `creation_time`, `last_change_time`) that a write cannot set. Keep the existing paragraph
   below it about the cache, which is still accurate.
7. Run this grep pair bare from the worktree root; both must hold:

   ```sh
   grep -c 'missing enumeration command' docs/bzr-cli.md   # must print 0 (exit 1)
   grep -cF 'list [<FIELD_NAME>]' docs/bzr-cli.md          # must print 1
   ```

   The first goes red on exactly the defect step 6 fixes. Note `grep -c` exits 1 on zero
   matches, so read the printed count, not just the exit status.
8. Run `cd agent-skills && BZR_BIN="$(git rev-parse --show-toplevel)/target/debug/bzr" sh tests/flag-drift-check.sh`
   bare from the worktree. Expect exit 0 and no drift lines. A run with `BZR_BIN` unset is
   evidence of nothing — it resolves the installed binary from `PATH`.
9. `git add -A && git commit -m "docs(field): document the field list name enumeration"`.

### Acceptance criteria

- `flag-drift-check.sh` exits 0 against the freshly built binary.
- The `field list` section describes both forms and states the non-pairing explicitly.
- `docs/bzr-cli.md` contains no reference to issue #718 as open work, and its `--field`
  section points at `bzr field list` rather than `bzr server capabilities`.

## Task 6 — Functional coverage

**Interfaces consumed:** the CLI surface from Task 4; the harness helpers `test_begin`,
`test_pass`, `test_fail`, `run_bzr` (which prepends `--json`), `run_bzr_raw`, `assert_success`,
`assert_exit_code`, `assert_json`, `assert_stderr_contains`, all defined in
`tests/functional/lib.sh`.

**Where it fits:** the acceptance criteria require coverage against a real container, including
the credentialless path.

### Verification

- Contract: the listing enumerates both sources against a real server.
  `Mode: focused-test` — the new `field-list-no-argument-*` blocks in
  `tests/functional/phases/05-fields-classifications.sh`. Red observation is produced
  deliberately in step 9 below. Green: `make functional-test` reports them passing.
- Contract: a name read out of the listing is accepted by `--field` against a real server.
  `Mode: focused-test` — the new `field-list-agrees-with-field-validator` block in
  `tests/functional/phases/08g-bug-arbitrary-fields.sh`. Green: `make functional-test`.
- Contract: the rejection message names a command that works.
  `Mode: focused-test` — the amended `undeclared-field-advice-names-a-command-that-works`
  block in the same file.

### Steps

1. In `tests/functional/phases/05-fields-classifications.sh`, after the `field-aliases` block
   and before the classification blocks, add (4-space indent throughout):

   ```bash
   # `field list` with no argument enumerates the whole accepted --field set: the
   # server's catalogue names and the REST names bzr models (ADR 0062, issue #718).
   # Asserting BOTH sources appear is what discriminates the union from either half
   # alone — a catalogue-only regression drops every `bzr` row, and a BUG_FIELDS-only
   # regression drops every `server` row.
   test_begin "field-list-no-argument-lists-both-sources" "field list (no argument) lists both sources"
   run_bzr field list
   if assert_success &&
       assert_json 'any(.[]; .source == "server")' true &&
       assert_json 'any(.[]; .source == "bzr")' true; then test_pass; fi

   # The concrete asymmetry the issue is about: Bugzilla's catalogue reports
   # `status_whiteboard`, the write API takes `whiteboard`, and both are accepted.
   # Naming the pair makes this fail on the real regression rather than on an
   # abstraction of it.
   test_begin "field-list-no-argument-marks-internal-and-rest-names" "field list marks internal and REST spellings"
   run_bzr field list
   if assert_success &&
       assert_json 'map(select(.name == "status_whiteboard")) | .[0].source' "server" &&
       assert_json 'map(select(.name == "whiteboard")) | .[0].source' "bzr"; then test_pass; fi

   test_begin "field-list-no-argument-fields-projects-keys" "field list (no argument) --fields projects keys"
   run_bzr field list --fields name
   if assert_success && assert_json '.[0] | keys == ["name"]' true; then test_pass; fi

   test_begin "field-list-no-argument-fields-unknown-exits-7" "field list (no argument) --fields unknown exits 7"
   run_bzr field list --fields sort_key
   if assert_exit_code 7; then test_pass; fi

   # The catalogue is anonymously readable, so the listing must work with no
   # credential. `sort_key` above and this block together prove the no-argument form
   # validates against its own key set rather than FieldValue's.
   test_begin "credentialless-field-list-no-argument" "credentialless field list (no argument)"
   run_bzr_raw --json --server public field list
   if assert_success &&
       assert_json 'any(.[]; .source == "server")' true &&
       assert_json 'any(.[]; .source == "bzr")' true; then test_pass; fi
   ```

   Note `--fields sort_key`: it is a valid key of the *named* form and an invalid key of this
   one, so the exit-7 assertion fails if the handler validates against the wrong key set. A
   nonsense token like `bogus_xyz` would pass either way and prove nothing.
2. In `tests/functional/phases/08g-bug-arbitrary-fields.sh`, in the
   `bug-update-undeclared-field-exits-7` block, change
   `assert_stderr_contains "bzr server capabilities"` to
   `assert_stderr_contains "bzr field list"`.
3. Replace the `undeclared-field-advice-names-a-command-that-works` block — comment and all —
   with:

   ```bash
   # The rejection above is the one message a user is guaranteed to read, because they
   # only see it when they are already stuck. Advice that fails when followed is a
   # defect, so run the command it names and require it to work. Since #718 that
   # command is `bzr field list` with no argument, which enumerates the whole accepted
   # set rather than the custom-field subset.
   test_begin "undeclared-field-advice-names-a-command-that-works" "the undeclared-field message names a command that works"
   run_bzr field list
   if assert_success && assert_json 'length > 0' true; then test_pass; fi
   ```

4. **At the very end of the phase**, immediately before the `rm -r "$_AF_DIR"` line
   (currently 08g:157), add the live agreement oracle. Placement matters: the phase's last
   existing block (08g:145-155) asserts `.whiteboard` is `""` on `$AFID`, so an oracle that
   writes ahead of it would be relied upon to be ignored by Bugzilla — the design's own caveat
   3 says bzr cannot prove what the server honours, so do not build a passing assertion on it.

   ```bash
   # Acceptance criterion 2 against a real server: anything the listing shows is
   # accepted. The name is read OUT of the listing rather than hard-coded in the
   # comparison, so the assertion bites in both directions — a listing that stopped
   # emitting server-only names yields an empty name and fails at the guard, and a
   # listing that emitted a name the validator rejects exits 7 here.
   test_begin "field-list-agrees-with-field-validator" "a server-only name from field list is accepted by --field"
   run_bzr field list
   _AF_SERVER_NAME=""
   if assert_success; then
       _AF_SERVER_NAME=$(jq -r 'map(select(.source == "server" and .name == "short_desc")) | .[0].name // empty' "$BZR_STDOUT")
   fi
   if [[ -z "$_AF_SERVER_NAME" ]]; then
       test_fail "field list did not report short_desc as a server-declared name"
   elif [[ -z "$AFID" ]]; then
       test_fail "no fixture bug: the --field create above did not succeed"
   else
       run_bzr bug update "$AFID" --field "${_AF_SERVER_NAME}=oracle"
       if assert_success; then test_pass; fi
   fi
   ```

   `short_desc` is pinned deliberately, on three grounds. An arbitrary `.[0]` could land on a
   read-only catalogue field (`bug_id`, a timestamp) that Bugzilla refuses on its own, which
   would redden the block for a reason unrelated to bzr's validator. `short_desc` is proven
   declared by these containers by a currently-passing test in this suite
   (`05-fields-classifications.sh:36`), not by an ADR alone. And it is not a `BUG_FIELDS`
   canonical — `summary` is — so `source: server` is the grounded expectation. If Bugzilla
   does honour it, the only effect is on the fixture bug's summary, which nothing asserts.

4a. Add `_AF_SERVER_NAME` to the phase's `unset` line (currently 08g:158), which today reads
   `unset _AF _AF_DIR _AF_INITIAL _AF_UPDATED _AF_JSON AFID`. Phase files are sourced into one
   shell by the runner, so a variable left set leaks into every later phase.

5. Run `make lint` bare. Expect exit 0.
6. Run `make test` bare. Expect exit 0.
7. Run `cargo build` bare so the functional harness runs HEAD.
8. Run `make functional-test` bare. It takes roughly 10 minutes on a warm Docker/podman host;
   background it rather than polling. Expect exit 0.
9. **Confirm the new assertions bite.** The Makefile exposes no per-phase target —
   `functional-test`, `functional-test-bz50/52/53`, `functional-test-all` and
   `functional-test-keyring` are all whole-suite runs differing only in Bugzilla version — so
   budget **two further full runs at roughly 10 minutes each** (the fault run, then the revert
   run) and background both per the repository's guardrail-runtime rule. After the green run,
   make one controlled fault: in `src/commands/runtime/shared/field_catalogue.rs`, change
   `accepted_bug_fields_map` to skip the `declared` loop entirely (return only the `BUG_FIELDS`
   half). Rebuild and re-run `make functional-test`. Expect exactly
   **four** blocks to fail — `field-list-no-argument-lists-both-sources`,
   `field-list-no-argument-marks-internal-and-rest-names`,
   `credentialless-field-list-no-argument` (every row becomes `bzr`, so its
   `any(.[]; .source == "server")` assertion is false too), and
   `field-list-agrees-with-field-validator` — and exactly **three** to stay green:
   `field-list-no-argument-fields-projects-keys`,
   `field-list-no-argument-fields-unknown-exits-7`, and
   `undeclared-field-advice-names-a-command-that-works` (whose `length > 0` is still satisfied
   by the 28 `BUG_FIELDS` rows). Record the full seven-block partition, not a subset: a
   partial expectation cannot tell a correct fault from a fault that also broke something
   else. Revert the fault, rebuild, re-run, and
   confirm green. Record both observations — the red list and the green re-run — in the PR body
   and the completion report.
10. **Confirm the counters moved per phase.** Read the per-phase summary lines from the green
    run and check that phase 5's and phase 8g's totals rose by the number of blocks added
    (5 and 1 respectively, with one block amended in place). A green aggregate is not evidence
    that a new block ran.
11. `git add -A && git commit -m "test(field): cover the field list name enumeration"`.

### Acceptance criteria

- `make functional-test` is green with the new blocks counted, verified per phase.
- The controlled-fault run turned the three named blocks red and the revert turned them green,
  and both observations are recorded.
- Every new fixture guard has an `else` branch calling `test_fail`; no new block can silently
  evaporate.

## Post-task guardrails

Run bare, in order, before the PR:

1. `make lint`
2. `make test`
3. `cargo build` then `make functional-test`
4. `cd agent-skills && BZR_BIN="$(git rev-parse --show-toplevel)/target/debug/bzr" sh tests/flag-drift-check.sh`

`make skills-test` is **not** required: nothing under `agent-skills/` is modified. Run
`flag-drift-check.sh` directly instead, with `BZR_BIN` set.

## Rollback

Every task is a single commit on `feat/field-list-names-718` with no migration, no persisted
state change, and no external write. Reverting the branch restores the prior behaviour exactly;
the only durable artifact is the `SCHEMA_VERSION` string, which reverts with it.
