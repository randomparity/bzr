# Issue #158: `bug list` missing field filters

**Date:** 2026-05-06
**Issue:** [#158](https://github.com/randomparity/bzr/issues/158)
**Surfaced by:** `docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md` (Issue C)

## 1. Summary

Add eight new field filters to `bzr bug list`, `bzr query save`, and
`bzr query run`: `--whiteboard`, `--target-milestone`, `--version`,
`--op-sys`, `--platform`, `--resolution`, `--qa-contact`, `--url`. All
eight are repeatable for OR within a field, AND across fields, and
accept `!`-prefix to invert. Six are exact-match (using `notequals`
for negation); two — `--whiteboard` and `--url` — are substring-match
(using `notsubstring` for negation, matching Bugzilla's native
behavior on those fields).

The change extends the existing `FIELD_MAPPINGS`-driven encoding by
one column (`negation_operator`) and reuses the multi-value plumbing
already in place for the current 7 filters.

## 2. Motivation

`bzl-search` exposes all eight filters
(`reference/bzl/bzl-search:111-169`); `bzr bug list` exposes none of
them. Each is a real workflow filter on common Bugzilla installations:
whiteboard tagging conventions, version/milestone scoping, hardware
filtering, resolution-state queries, QA-contact triage views, and URL
substring matching. Without them, testers fall back to `--from-url` or
client-side filtering on the full result set.

This is an umbrella ticket that finishes the bzl-parity gap surfaced
in Issue C of the parity review.

## 3. Scope

In scope:

- Eight new CLI flags on `bzr bug list`, `bzr query save`, and
  `bzr query run` (the last as overrides).
- Eight new `Vec<String>` fields on `SearchParams` and `SavedQuery`.
- One new column on `FieldMapping` (`negation_operator`) and eight
  new rows in `FIELD_MAPPINGS`.
- Refactor of `SearchParams::apply_overrides` from positional params
  to an `Overrides` struct (forced by parameter-count limit).
- REST and XML-RPC encoding (driven entirely by `FIELD_MAPPINGS`).
- Unit, integration, and functional tests.
- `docs/bzr-cli.md` and `CHANGELOG.md` updates.

Out of scope:

- Range filters (`--version-min`, milestone ordering). Power users
  with range needs can use `--from-url` boolean charts.
- CSV-split (`value_delimiter`) semantics on the new flags.
  Repeatable-only matches the existing 7 fields' convention.
- Magic values (`me` resolver) for `--qa-contact`. Can be added later
  if a workflow gap is filed.
- Modifying the existing 7 fields' negation operator. They are all
  exact-match and `notequals` remains correct.

## 4. CLI surface

### 4.1 New flags

```
--whiteboard <text>          substring match on the Status Whiteboard
--target-milestone <name>    exact match
--version <name>             exact match
--op-sys <name>              exact match
--platform <name>            exact match (Bugzilla "Hardware" / API param `platform`)
--resolution <name>          exact match (e.g. FIXED, DUPLICATE; empty for open bugs)
--qa-contact <login>         exact match on QA Contact login
--url <text>                 substring match on the URL field
```

All eight are `Vec<String>` (clap repeatable). Repeating a flag
gives OR semantics within that field for positive values
(e.g. `--whiteboard wip --whiteboard review` matches bugs whose
whiteboard contains `wip` *or* `review`), and AND semantics for
negated values (e.g. `--whiteboard '!wip' --whiteboard '!review'`
matches bugs whose whiteboard contains neither — de Morgan's
complement of the positive case). Different flags AND together, and
all eight AND with the existing filters. Any combination of the
eight may be set.

Help text on each flag names the match style explicitly. Example for
`--whiteboard`:

> Filter by Status Whiteboard substring (repeatable for OR; prefix
> with ! to exclude).

### 4.2 `!`-prefix semantics

For each filter, `!`-prefix flips the operator:

| Flag                  | Positive | Negation operator |
|-----------------------|----------|-------------------|
| `--whiteboard`        | substring | `notsubstring`   |
| `--target-milestone`  | exact     | `notequals`      |
| `--version`           | exact     | `notequals`      |
| `--op-sys`            | exact     | `notequals`      |
| `--platform`          | exact     | `notequals`      |
| `--resolution`        | exact     | `notequals`      |
| `--qa-contact`        | exact     | `notequals`      |
| `--url`               | substring | `notsubstring`   |

Bugzilla's `Bug.search` API uses substring matching natively for
`whiteboard` and `url` on the positive side, so the operator
asymmetry only shows up in the boolean-chart encoding for negation —
the user-facing semantics are symmetric ("contains" vs "does not
contain" for substring fields; "equals" vs "does not equal" for
exact fields).

### 4.3 `--platform` naming

The flag is `--platform`, the `SearchParams`/`SavedQuery` struct
field is `platform`, and the REST API parameter is `platform`. This
matches `bzl-search` and Bugzilla's `Bug.search` documentation.

The `Bug` output struct keeps `rep_platform` (Bugzilla's bug-record
field name), and `bzr bug create`/`clone` keeps `--rep-platform` for
the input side. Search and create are distinct code paths; the
asymmetry is documented but accepted.

URL imports (`--from-url`) recognize the legacy `rep_platform`
buglist.cgi URL parameter and route it into the `platform` struct
field. Modern URLs that use `platform` directly fall through to
`raw_params` (existing behavior).

## 5. Implementation

### 5.1 `FieldMapping` extension

`src/types/bug.rs::FieldMapping` gains one column:

```rust
pub struct FieldMapping {
    pub struct_field: &'static str,    // SearchParams field + REST API param name
    pub url_param: &'static str,       // buglist.cgi URL param (for --from-url)
    pub internal_name: &'static str,   // boolean-chart fN/oN/vN field name
    pub negation_operator: &'static str, // "notequals" or "notsubstring"
}
```

The seven existing rows each gain `negation_operator: "notequals"`
(behavior-preserving). Eight new rows are appended:

```rust
FieldMapping {
    struct_field: "whiteboard",
    url_param: "status_whiteboard",
    internal_name: "status_whiteboard",
    negation_operator: "notsubstring",
},
FieldMapping {
    struct_field: "target_milestone",
    url_param: "target_milestone",
    internal_name: "target_milestone",
    negation_operator: "notequals",
},
FieldMapping {
    struct_field: "version",
    url_param: "version",
    internal_name: "version",
    negation_operator: "notequals",
},
FieldMapping {
    struct_field: "op_sys",
    url_param: "op_sys",
    internal_name: "op_sys",
    negation_operator: "notequals",
},
FieldMapping {
    struct_field: "platform",
    url_param: "rep_platform",
    internal_name: "rep_platform",
    negation_operator: "notequals",
},
FieldMapping {
    struct_field: "resolution",
    url_param: "resolution",
    internal_name: "resolution",
    negation_operator: "notequals",
},
FieldMapping {
    struct_field: "qa_contact",
    url_param: "qa_contact",
    internal_name: "qa_contact",
    negation_operator: "notequals",
},
FieldMapping {
    struct_field: "url",
    url_param: "bug_file_loc",
    internal_name: "bug_file_loc",
    negation_operator: "notsubstring",
},
```

`url_param` for `whiteboard`, `platform`, and `url` uses the legacy
buglist.cgi parameter names because real URL imports in the wild use
those forms. URLs using the modern API parameter names (`whiteboard`,
`platform`, `url`) still work — they fall through to `raw_params` and
are forwarded to the server verbatim.

### 5.2 `SearchParams` changes

Eight new `Vec<String>` fields appended to `SearchParams`:

```rust
pub whiteboard: Vec<String>,
pub target_milestone: Vec<String>,
pub version: Vec<String>,
pub op_sys: Vec<String>,
pub platform: Vec<String>,
pub resolution: Vec<String>,
pub qa_contact: Vec<String>,
pub url: Vec<String>,
```

`SearchParams::get_field()` (the `match_field!` invocation) gains
eight new arms — one per new field — mapping each `struct_field`
literal to the corresponding `Vec<String>`.

`SearchParams::has_filters()` and `has_structured_filters()` each
gain eight `!self.<field>.is_empty()` clauses.

### 5.3 `SavedQuery` changes

Eight new fields with the same names as on `SearchParams`, each
attributed for forwards-compatible TOML deserialization:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub whiteboard: Vec<String>,
// ... seven more
```

`SavedQuery::get_field_mut()` (the second `match_field!` invocation)
gains the eight new arms. `SavedQuery::into_search_params()` and
`to_search_params()` forward all eight. `SavedQuery::has_filters()`
gains the eight emptiness checks so a query with only one of the new
fields is accepted by `bzr query save`.

### 5.4 `apply_overrides` refactor (`Overrides` struct)

The existing 5-positional-param signature is at the project's limit
and cannot grow further. Replace with a single struct argument:

```rust
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Overrides<'a> {
    pub limit: Option<u32>,
    pub fields: Option<&'a str>,
    pub exclude_fields: Option<&'a str>,
    pub creation_time: Option<&'a str>,
    pub last_change_time: Option<&'a str>,
    pub whiteboard: Option<&'a [String]>,
    pub target_milestone: Option<&'a [String]>,
    pub version: Option<&'a [String]>,
    pub op_sys: Option<&'a [String]>,
    pub platform: Option<&'a [String]>,
    pub resolution: Option<&'a [String]>,
    pub qa_contact: Option<&'a [String]>,
    pub url: Option<&'a [String]>,
}

impl SearchParams {
    pub fn apply_overrides(&mut self, o: Overrides<'_>) {
        if let Some(l) = o.limit { self.limit = Some(l); }
        if let Some(f) = o.fields { self.include_fields = Some(f.into()); }
        if let Some(ef) = o.exclude_fields { self.exclude_fields = Some(ef.into()); }
        if let Some(ct) = o.creation_time { self.creation_time = Some(ct.into()); }
        if let Some(lct) = o.last_change_time { self.last_change_time = Some(lct.into()); }
        if let Some(wb) = o.whiteboard { self.whiteboard = wb.to_vec(); }
        // ... seven more multi-value branches
    }
}
```

Override semantics for the eight multi-value fields: `None` keeps the
saved value; `Some(&[…])` (any non-empty slice) replaces it entirely.
The convention at the call site is "empty `Vec` from clap → `None`
override; non-empty `Vec` → `Some(&vec)` override," which matches
user intent: `bzr query run my-q --whiteboard wip` pins the run to
`whiteboard=["wip"]` without needing a clear sentinel.

This is a breaking API change for `apply_overrides`. The single
in-tree caller (`commands/query.rs::handle_run`) is updated as part
of the same change. There are no external callers.

### 5.5 REST encoding (`src/client/bug.rs`)

`append_multi_value_params` is unchanged — adding rows to
`FIELD_MAPPINGS` makes the new fields encode automatically as
`&<struct_field>=<value>` per positive value.

`append_negated_params` replaces its hardcoded `"notequals"` literal
with `mapping.negation_operator`. That is the entire diff to the
negation path.

`append_option_params` is unchanged — the new fields are
`Vec<String>`, not `Option<String>`.

`has_negated_filters` is unchanged.

### 5.6 XML-RPC encoding (`src/xmlrpc/client.rs`)

`add_vec_filters` makes the same one-line change: replace the
hardcoded `"notequals"` literal with `mapping.negation_operator`.
Positive-value array encoding is unchanged.

The `option_fields` table in `search_bugs` is unchanged.

### 5.7 URL parser (`src/url_parser.rs`)

Zero code change. `classify_param` already dispatches via
`FIELD_MAPPINGS::url_param`; new rows pick up automatically.

### 5.8 CLI module (`src/cli/`)

**`src/cli/bug.rs::BugAction::List`** gains eight new `Vec<String>`
fields with `#[arg(long)]`. The doc comment for `BugAction::List`
gains a sentence under the existing filter-flags paragraph noting
the eight new flags and the substring/exact distinction.

**`src/cli/query.rs::QueryAction::Save`** and `QueryAction::Run`
each gain the same eight `Vec<String>` flags. Help text mirrors the
`bug list` flags.

### 5.9 Command wiring

**`src/commands/bug/list.rs`** destructures the eight new flags into
the corresponding `SearchParams` fields. No new validation step
(unlike the date filters in #157, there is no syntactic precheck to
perform).

**`src/commands/query.rs::handle_save`** stores the eight new flags
on the `SavedQuery` struct verbatim.

**`src/commands/query.rs::handle_run`** rewrites its
`apply_overrides` call to construct an `Overrides` struct from the
CLI flags. The empty-`Vec` → `None` translation is local to the call
site:

```rust
fn slice_override(v: &[String]) -> Option<&[String]> {
    (!v.is_empty()).then_some(v)
}

params.apply_overrides(Overrides {
    limit,
    fields: fields.as_deref(),
    exclude_fields: exclude_fields.as_deref(),
    creation_time: created_since.as_deref(),
    last_change_time: changed_since.as_deref(),
    whiteboard: slice_override(&whiteboard),
    target_milestone: slice_override(&target_milestone),
    version: slice_override(&version),
    op_sys: slice_override(&op_sys),
    platform: slice_override(&platform),
    resolution: slice_override(&resolution),
    qa_contact: slice_override(&qa_contact),
    url: slice_override(&url),
});
```

### 5.10 Output module

`src/output/query.rs::print_query_detail` enumerates each field
explicitly via `print_list_field` (multi-value) or
`print_optional_field`/`print_field` (single-value). It gains eight
new `print_list_field` calls — one per new field — placed adjacent
to the existing seven for visual symmetry. Suggested labels:
`"Whiteboard"`, `"Target Milestone"`, `"Version"`, `"OS"`,
`"Platform"`, `"Resolution"`, `"QA Contact"`, `"URL"`.

`src/output/query.rs::query_summary_line` (the one-liner used by
`bzr query list`) currently shows only a curated subset of fields
(`product`, `status`, search, dates, `limit`, raw-param count). Add
**none** of the eight new fields to that summary — eight more
parts would overwhelm the line, and the detail view is one
`bzr query show` away. If a tester later requests a denser summary,
that is a separate change.

## 6. Testing

### 6.1 Unit tests (sibling `_tests.rs` files)

- **`src/types/bug_tests.rs`**:
  - `SearchParams::has_filters()` and `has_structured_filters()`
    return `true` when only one of the eight new fields is set
    (table-driven over the 8 fields).
  - `SavedQuery::has_filters()` returns `true` when only one new
    field is set.
  - `SavedQuery::into_search_params()` forwards all 8 fields.
  - Round-trip TOML preserves all 8 fields.
  - `Overrides` struct: `apply_overrides` replaces saved
    multi-value fields when `Some(&[…])` and preserves them when
    `None`.
  - `Default` value of `Overrides` is a no-op when applied.

- **`src/client/bug_tests.rs`**:
  - Wiremock asserts the REST request URL carries the expected
    positive params for each new field
    (`&whiteboard=wip`, `&platform=Linux`, etc., table-driven over
    the 8 fields).
  - Negation for an exact-match field: assert
    `f1=resolution&o1=notequals&v1=FIXED`.
  - Negation for a substring field: assert
    `f1=status_whiteboard&o1=notsubstring&v1=wip`.
  - Mixed positive + negative: confirm the boolean-chart index
    increments correctly across mixed fields.

- **`src/xmlrpc/client_tests.rs`**:
  - RPC params map carries each new field as an array of strings
    (positive case, table-driven).
  - Negation case for one exact-match and one substring field
    produces the expected `fN/oN/vN` triples.

- **`src/url_parser_tests.rs`**:
  - `?status_whiteboard=wip` parses into `SavedQuery.whiteboard`.
  - `?bug_file_loc=foo` parses into `SavedQuery.url`.
  - `?rep_platform=Linux` parses into `SavedQuery.platform`.
  - One round-trip: `--from-url` parse → `SavedQuery` → save →
    `into_search_params()` → encoded REST URL has the right
    parameter name (`whiteboard`, `url`, `platform`).

- **`src/commands/query_tests.rs`**:
  - `query save --whiteboard foo` stores the value.
  - `query save` with only a `--whiteboard` filter is accepted
    (regression for `has_filters()`).
  - `query run` overrides replace saved values for each new field.
  - `query run` with empty CLI vec preserves saved value.

- **`src/cli/mod_tests.rs`**:
  - clap parses each of the eight new flags as `Vec<String>` on
    `bug list`, `query save`, and `query run`. Table-driven across
    the 24 cases.

- **`src/output/query_tests.rs`**:
  - `print_query_detail` emits a labeled row for each of the eight
    new fields when set (table-driven over the 8 fields).
  - `print_query_detail` omits the row when the field is empty
    (existing `print_list_field` behavior; one regression test
    confirms it).
  - `query_summary_line` is unchanged when only the new fields are
    set (regression test that proves we did not silently widen the
    summary view).

### 6.2 Integration test (`tests/integration.rs`)

One end-to-end wiremock run of:

```sh
bzr bug list --product P --whiteboard wip --resolution '!FIXED'
```

Asserts the outgoing REST request URL carries `product=P`,
`whiteboard=wip`, and `f1=resolution&o1=notequals&v1=FIXED`. The
mixed positive + negation across two new fields exercises the full
pipeline.

### 6.3 Functional tests (`tests/functional/run-tests.sh`, Phase 8)

Two added blocks against the real Bugzilla container:

```sh
# Setup: create two bugs in product P, set whiteboard on bug A
#        to "needs-review", leave bug B's whiteboard empty.

# Action 1: bzr bug list --product P --whiteboard "needs-review" --json
# Assert:   jq output contains bug A; bug B is absent.

# Action 2: bzr bug list --product P --whiteboard '!needs-review' --json
# Assert:   jq output contains bug B; bug A is absent.
```

Plus one structural test of an exact-match field — `--resolution
FIXED` and `--resolution '!FIXED'` against a fixture pair — to
validate the `notequals` path round-trips through the real server.

Total added shell: ~50 lines.

## 7. Migration & compatibility

- `SavedQuery` gains 8 `#[serde(default, skip_serializing_if =
  "Vec::is_empty")]` fields. Existing TOML configs deserialize
  unchanged; no migration step required.
- `SearchParams` is `#[non_exhaustive]`; new fields do not break
  external callers.
- `FieldMapping` gains a public field. External users who construct
  `FieldMapping` literals would break; there are none in tree, and
  `FIELD_MAPPINGS` is the only documented consumer.
- `SearchParams::apply_overrides` signature change is a breaking
  in-tree-only API change. The single in-tree caller is updated in
  the same change.

## 8. Documentation

`docs/bzr-cli.md` gains a "Field filters" subsection under
`bug list` documenting the eight new flags, their match style, and
negation behavior. The `query save` and `query run` sections gain a
one-line cross-reference: *"All `bzr bug list` filter flags are
also accepted; see [bug list](#bug-list) for syntax and
semantics."*

`CHANGELOG.md` gains entries under `## [0.4.0-dev]`:

```
### Added
- `bzr bug list`, `bzr query save`, and `bzr query run` accept eight
  new field filters: `--whiteboard`, `--target-milestone`,
  `--version`, `--op-sys`, `--platform`, `--resolution`,
  `--qa-contact`, and `--url`. All eight are repeatable for OR
  within a field, AND across fields, and accept `!`-prefix to
  invert (substring fields use `notsubstring`, exact-match fields
  use `notequals`). Closes #158.

### Changed
- `SearchParams::apply_overrides` now takes a single `Overrides`
  struct instead of five positional parameters. Internal API; no
  caller-visible effect outside the crate.
```

## 9. Out of scope (revisitable)

- Range filters such as `--version-min`, milestone ordering, or
  numeric milestone comparisons. Bugzilla's `Bug.search` doesn't
  expose these as structured params; users with range needs can
  reach for `--from-url` boolean charts.
- CSV-split `value_delimiter` semantics on the new flags.
  Repeatable-only matches the existing 7 fields' convention.
- Magic `me` resolver for `--qa-contact`. Can be added later if a
  workflow gap is filed.
- Switching the existing 7 fields from `notequals` to a different
  operator. They are exact-match and `notequals` is correct.
- A future `--whiteboard-regex` / `--url-regex` (server-side regex
  is supported by Bugzilla but is a different shape and not
  pre-empted by this design).
