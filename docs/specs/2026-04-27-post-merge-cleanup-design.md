# Post-Merge Cleanup: from-url Review Items

Addresses issues #88-#93, all deferred from the `--from-url` / saved query PR review.

## Issue #88: Unify Field-Mapping Tables

### Problem

The same 7 Bugzilla field mappings are duplicated in 3 locations with slightly different representations:

1. `url_parser.rs` — URL param name to `SavedQuery` field name (inline match arms)
2. `types/bug.rs` — `BOOLEAN_CHART_FIELD_NAMES` (`SearchParams` field to Bugzilla internal name)
3. `client/bug.rs` — `append_multi_value_params` + `append_negated_params` (inline field arrays)

Adding a new filterable field requires editing all 3, with no compile-time enforcement.

### Design

Define a single canonical table in `types/bug.rs`:

```rust
/// Each entry maps a filterable field across all naming contexts:
/// - `struct_field`: name on `SearchParams` (e.g. "status") — also used as the
///   REST API query parameter since Bugzilla accepts both short and long forms
/// - `url_param`: `buglist.cgi` URL parameter name (e.g. "bug_status")
/// - `internal_name`: Bugzilla internal name for boolean charts (e.g. "bug_status")
///
/// `url_param` and `internal_name` happen to be identical for all current fields.
/// They're kept separate because the URL format and boolean chart format are
/// different API surfaces that could diverge.
pub struct FieldMapping {
    pub struct_field: &'static str,
    pub url_param: &'static str,
    pub internal_name: &'static str,
}

pub const FIELD_MAPPINGS: &[FieldMapping] = &[
    FieldMapping { struct_field: "product",     url_param: "product",      internal_name: "product" },
    FieldMapping { struct_field: "component",   url_param: "component",    internal_name: "component" },
    FieldMapping { struct_field: "status",      url_param: "bug_status",   internal_name: "bug_status" },
    FieldMapping { struct_field: "assigned_to", url_param: "assigned_to",  internal_name: "assigned_to" },
    FieldMapping { struct_field: "creator",     url_param: "reporter",     internal_name: "reporter" },
    FieldMapping { struct_field: "priority",    url_param: "priority",     internal_name: "priority" },
    FieldMapping { struct_field: "severity",    url_param: "bug_severity", internal_name: "bug_severity" },
];
```

**Consumers updated:**

- `client/bug.rs`: `append_multi_value_params` iterates `FIELD_MAPPINGS`, using `struct_field` as the REST param name and `get_field` for the values. `append_negated_params` uses `internal_name` for boolean chart field names.
- `url_parser.rs`: Replace the inline match with a lookup into `FIELD_MAPPINGS` by `url_param`. Map to `SavedQuery` field via `struct_field` and `get_field_mut`.
- `types/bug.rs`: Remove `BOOLEAN_CHART_FIELD_NAMES`; callers use `FIELD_MAPPINGS[i].internal_name` instead.
- `types/mod.rs`: Export `FIELD_MAPPINGS` and `FieldMapping` instead of `BOOLEAN_CHART_FIELD_NAMES`.

**Field accessor helper** — both `SearchParams` and `SavedQuery` need a way to get a `&[String]` / `&mut Vec<String>` by struct_field name. Add methods:

```rust
impl SearchParams {
    /// Get a reference to a multi-value filter field by its struct_field name.
    pub fn get_field(&self, name: &str) -> &[String] { ... }
}

impl SavedQuery {
    /// Get a mutable reference to a multi-value filter field by name.
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Vec<String>> { ... }
}
```

These use a match internally (7 arms) which is a single source of truth for the struct-to-field binding. The match arms map directly to `FIELD_MAPPINGS` entries.

### Note on `assigned_to` vs `assignee`

`SearchParams` uses `assigned_to` (matching the REST API param name), while `SavedQuery` uses `assignee` (a friendlier name for TOML config). The `FIELD_MAPPINGS` table uses `assigned_to` as the `struct_field` name since that's what `SearchParams` uses. `SavedQuery` handles the mapping in its `get_field_mut` method (maps `"assigned_to"` to `self.assignee`).

## Issue #89: Add `into_search_params`

### Problem

`SavedQuery::to_search_params()` clones all 7 `Vec<String>` fields plus `raw_params`. Callers that own the `SavedQuery` discard it after conversion, making the clones unnecessary.

### Design

Add a consuming variant:

```rust
impl SavedQuery {
    pub fn into_search_params(self) -> SearchParams {
        SearchParams {
            product: self.product,
            component: self.component,
            status: self.status,
            assigned_to: self.assignee,
            creator: self.creator,
            priority: self.priority,
            severity: self.severity,
            quicksearch: self.quicksearch,
            limit: self.limit,
            include_fields: self.fields,
            exclude_fields: self.exclude_fields,
            raw_params: self.raw_params,
            ..Default::default()
        }
    }
}
```

Update call sites that own the `SavedQuery`:
- `commands/query.rs::handle_run` — owns the query after loading from config
- `commands/bug.rs::handle_search` (if applicable) — owns the query after URL parsing

Keep borrowing `to_search_params(&self)` for call sites that borrow from config.

## Issue #90: Migrate `output/query.rs` from `println!` to `writeln!`

### Problem

3 `println!` call sites in `output/query.rs` violate the project convention and break `capture_stdout` tests.

### Design

Replace each `println!(...)` with `let _ = writeln!(io::stdout(), ...);` in:
- `print_query_saved` (line 40)
- `print_query_list` (lines 48, 54)

Add `use std::io::{self, Write};` import. Remove any `#[expect(clippy::print_stdout)]` attributes that exist on these functions.

## Issue #91: Guard Against Boolean Chart Index Collision

### Problem

`append_negated_params` generates `fN/oN/vN` starting at index 1. URL-imported `raw_params` may also contain `f1/o1/v1`. If both are present, Bugzilla silently uses whichever appears last.

Currently impossible (no CLI path combines both), but will silently corrupt queries if the surface area grows.

### Design

Add a validation check in `search_bugs_rest` (or `search_bugs`, before dispatching):

```rust
fn has_negated_filters(params: &SearchParams) -> bool {
    FIELD_MAPPINGS.iter().any(|m| {
        let values = params.get_field(m.struct_field);
        values.iter().any(|v| v.starts_with('!'))
    })
}

fn has_raw_boolean_chart_params(params: &SearchParams) -> bool {
    params.raw_params.iter().any(|(k, _)| {
        k.len() >= 2
            && (k.starts_with('f') || k.starts_with('o') || k.starts_with('v'))
            && k[1..].parse::<u32>().is_ok()
    })
}
```

In `search_bugs_rest`, before appending params:

```rust
if has_negated_filters(params) && has_raw_boolean_chart_params(params) {
    return Err(BzrError::InputValidation(
        "cannot combine negated filters (e.g. --status '!CLOSED') with a URL-imported \
         query containing boolean chart parameters; the chart indices would collide. \
         Use either negated filters or the raw URL query, not both.".into()
    ));
}
```

### Tests

- Test that a `SearchParams` with both negated status and raw `f1/o1/v1` returns `InputValidation` error.
- Test that negated-only and raw-only each work without error.

## Issue #92: Auto-Suggest Save Name from URL's `known_name`

### Problem

Bugzilla URLs often contain `known_name=<saved search name>`. This was designed to be extracted but removed as dead code during review.

### Design

**URL parser changes:**
- Move `known_name` from `IGNORED_PARAMS` to active extraction.
- Add `suggested_name: Option<String>` to `ParsedUrl` struct.
- Extract: prefer `known_name` over `query_based_on`. URL-decode the value.

**CLI changes:**
- Make `--save-as` accept an optional name: `--save-as [NAME]` (clap `num_args = 0..=1`, `default_missing_value = ""`).
- When `--save-as` is used without a name (empty string sentinel):
  - If `ParsedUrl::suggested_name` is `Some`, use that as the name.
  - Otherwise, return `InputValidation` error: "no name provided for --save-as and URL has no known_name; specify a name explicitly".
- When `--save-as <name>` is used with an explicit name, ignore `suggested_name`.
- `known_name` values may contain spaces and special characters; validate that the resulting name is non-empty after trimming.

**Output:**
- When auto-using `suggested_name`, print: `Saved query '<name>' (name from URL's known_name)`.

### Tests

- URL with `known_name=my%20search` populates `suggested_name`.
- `--save-as` without a name + URL with `known_name` uses the suggested name.
- `--save-as` without a name + URL without `known_name` returns error.
- `--save-as explicit-name` ignores `known_name`.

## Issue #93: Strengthen Server Override Test

### Problem

`query_run_with_server_override` saves a query and runs it with the same server as both the saved server and the override, so it doesn't test that the override takes precedence.

### Design

Rewrite the test:
1. Save a query with `server: Some("other-server")` in the `SavedQuery`.
2. Run with `server: Some("test")` (the wiremock server).
3. Assert the wiremock mock received exactly 1 request (`.expect(1)`).
4. This proves the override was used instead of `"other-server"`.

The test should manually insert the query into config (with `server: Some("other-server")`) rather than using the save action, to avoid needing a second wiremock server for `"other-server"`.

## Implementation Order

1. **#88** — Unify field mappings (foundational; touches types, client, url_parser)
2. **#89** — Add `into_search_params` (uses the unified table)
3. **#91** — Boolean chart collision guard (uses `FIELD_MAPPINGS` + `get_field`)
4. **#90** — Migrate println to writeln (independent, small)
5. **#93** — Strengthen server override test (independent, test-only)
6. **#92** — Auto-suggest save name (independent, touches url_parser + CLI)

Items 4-6 are independent and can be done in any order or in parallel.

## Testing Strategy

All changes should have unit tests. Issues #91 and #93 are specifically about adding/improving tests. The existing test suite must continue to pass after each change.

Run after each issue:
```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```
