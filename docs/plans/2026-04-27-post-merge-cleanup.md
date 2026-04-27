# Post-Merge Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Address 6 deferred review items from the `--from-url` PR (#88-#93) in a single cleanup PR.

**Architecture:** All changes build on the `feature/from-url-saved-queries` branch. Task 1 introduces a unified field-mapping table that Tasks 2, 3, and 4 depend on. Tasks 5-7 are independent.

**Tech Stack:** Rust, clap, wiremock, tokio

**Base branch:** `feature/from-url-saved-queries` (must be merged to `main` first, or this work branches from it)

---

## File Map

| File | Action | Tasks |
|------|--------|-------|
| `src/types/bug.rs` | Modify | 1, 2, 3 |
| `src/types/mod.rs` | Modify | 1 |
| `src/client/bug.rs` | Modify | 1, 4 |
| `src/url_parser.rs` | Modify | 1, 7 |
| `src/output/query.rs` | Modify | 5 |
| `src/commands/query.rs` | Modify | 3, 6 |
| `src/commands/bug.rs` | Modify | 3, 7 |
| `src/cli/bug.rs` | Modify | 7 |

---

### Task 1: Unify Field-Mapping Tables (#88)

**Files:**
- Modify: `src/types/bug.rs` (add `FieldMapping`, `FIELD_MAPPINGS`, accessor methods; remove `BOOLEAN_CHART_FIELD_NAMES`)
- Modify: `src/types/mod.rs` (update exports)
- Modify: `src/client/bug.rs` (rewrite `append_multi_value_params` and `append_negated_params` to use `FIELD_MAPPINGS`)
- Modify: `src/url_parser.rs` (rewrite field matching to use `FIELD_MAPPINGS`)

- [ ] **Step 1: Write tests for `FieldMapping` and accessor methods**

Add to the `#[cfg(test)] mod tests` block in `src/types/bug.rs`:

```rust
#[test]
fn field_mappings_covers_all_search_params_vec_fields() {
    // Verify that every FIELD_MAPPINGS entry resolves to a real field
    let params = SearchParams::default();
    for mapping in FIELD_MAPPINGS {
        let field = params.get_field(mapping.struct_field);
        assert!(field.is_empty(), "default field should be empty: {}", mapping.struct_field);
    }
}

#[test]
fn field_mappings_has_expected_count() {
    assert_eq!(FIELD_MAPPINGS.len(), 7);
}

#[test]
fn field_mappings_url_param_lookup() {
    let status = FIELD_MAPPINGS.iter().find(|m| m.url_param == "bug_status");
    assert!(status.is_some());
    assert_eq!(status.unwrap().struct_field, "status");
    assert_eq!(status.unwrap().internal_name, "bug_status");
}

#[test]
fn field_mappings_internal_name_for_creator() {
    let creator = FIELD_MAPPINGS.iter().find(|m| m.struct_field == "creator");
    assert!(creator.is_some());
    assert_eq!(creator.unwrap().internal_name, "reporter");
}

#[test]
fn search_params_get_field_returns_correct_data() {
    let params = SearchParams {
        product: vec!["Firefox".into()],
        status: vec!["NEW".into(), "ASSIGNED".into()],
        ..Default::default()
    };
    assert_eq!(params.get_field("product"), &["Firefox"]);
    assert_eq!(params.get_field("status"), &["NEW", "ASSIGNED"]);
    assert!(params.get_field("creator").is_empty());
}

#[test]
#[should_panic(expected = "unknown field")]
fn search_params_get_field_panics_on_unknown() {
    let params = SearchParams::default();
    params.get_field("nonexistent");
}

#[test]
fn saved_query_get_field_mut_returns_correct_fields() {
    let mut query = SavedQuery::default();
    query.get_field_mut("assigned_to").unwrap().push("dev@example.com".into());
    assert_eq!(query.assignee, vec!["dev@example.com"]);

    query.get_field_mut("status").unwrap().push("NEW".into());
    assert_eq!(query.status, vec!["NEW"]);
}

#[test]
fn saved_query_get_field_mut_returns_none_for_unknown() {
    let mut query = SavedQuery::default();
    assert!(query.get_field_mut("nonexistent").is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib types::bug::tests::field_mappings`
Expected: compilation errors — `FIELD_MAPPINGS`, `get_field`, `get_field_mut` don't exist yet.

- [ ] **Step 3: Implement `FieldMapping`, `FIELD_MAPPINGS`, and accessor methods**

In `src/types/bug.rs`, replace `BOOLEAN_CHART_FIELD_NAMES` (lines 142-150 on feature branch) with:

```rust
/// Maps a filterable field across all naming contexts.
pub struct FieldMapping {
    /// Name on `SearchParams` / `SavedQuery` (e.g. "status").
    /// Also used as the REST API query parameter.
    pub struct_field: &'static str,
    /// `buglist.cgi` URL parameter name (e.g. "bug_status").
    pub url_param: &'static str,
    /// Bugzilla internal name for boolean charts (e.g. "bug_status").
    pub internal_name: &'static str,
}

/// Canonical field-mapping table for the 7 multi-value filter fields.
///
/// Single source of truth consumed by:
/// - `client/bug.rs` (`append_multi_value_params`, `append_negated_params`)
/// - `url_parser.rs` (URL param to struct field mapping)
/// - Any code needing boolean chart internal names
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

Add to `impl SearchParams` (after `apply_overrides`):

```rust
/// Get a reference to a multi-value filter field by its struct_field name.
///
/// # Panics
/// Panics if `name` is not a recognized field in `FIELD_MAPPINGS`.
pub fn get_field(&self, name: &str) -> &[String] {
    match name {
        "product" => &self.product,
        "component" => &self.component,
        "status" => &self.status,
        "assigned_to" => &self.assigned_to,
        "creator" => &self.creator,
        "priority" => &self.priority,
        "severity" => &self.severity,
        _ => panic!("unknown field: {name}"),
    }
}
```

Add to `impl SavedQuery` (after `has_filters`):

```rust
/// Get a mutable reference to a multi-value filter field by struct_field name.
///
/// Uses `FIELD_MAPPINGS` struct_field names. Note that `assigned_to` maps to
/// `self.assignee` (the `SavedQuery` uses a friendlier name for TOML config).
///
/// Returns `None` for unrecognized field names.
pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Vec<String>> {
    match name {
        "product" => Some(&mut self.product),
        "component" => Some(&mut self.component),
        "status" => Some(&mut self.status),
        "assigned_to" => Some(&mut self.assignee),
        "creator" => Some(&mut self.creator),
        "priority" => Some(&mut self.priority),
        "severity" => Some(&mut self.severity),
        _ => None,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib types::bug::tests::field_mappings && cargo test --lib types::bug::tests::search_params_get_field && cargo test --lib types::bug::tests::saved_query_get_field`
Expected: all pass.

- [ ] **Step 5: Update `src/types/mod.rs` exports**

Replace the `BOOLEAN_CHART_FIELD_NAMES` export with `FIELD_MAPPINGS` and `FieldMapping`:

In the `pub use bug::{...}` block (line 11-13 on feature branch), change:
```rust
    BOOLEAN_CHART_FIELD_NAMES,
```
to:
```rust
    FIELD_MAPPINGS, FieldMapping,
```

- [ ] **Step 6: Rewrite `append_multi_value_params` in `src/client/bug.rs`**

Replace the current function (lines 34-54) with:

```rust
fn append_multi_value_params(
    mut builder: reqwest::RequestBuilder,
    params: &SearchParams,
) -> reqwest::RequestBuilder {
    for mapping in FIELD_MAPPINGS {
        let (positive, _) = partition_filters(params.get_field(mapping.struct_field));
        for v in positive {
            builder = builder.query(&[(mapping.struct_field, v)]);
        }
    }
    builder
}
```

- [ ] **Step 7: Rewrite `append_negated_params` in `src/client/bug.rs`**

Replace the current function (lines 64-93) with:

```rust
fn append_negated_params(
    mut builder: reqwest::RequestBuilder,
    params: &SearchParams,
) -> reqwest::RequestBuilder {
    let mut idx = 1u32;
    for mapping in FIELD_MAPPINGS {
        let (_, negated) = partition_filters(params.get_field(mapping.struct_field));
        for v in negated {
            let f_key = format!("f{idx}");
            let o_key = format!("o{idx}");
            let v_key = format!("v{idx}");
            builder = builder.query(&[
                (&f_key, mapping.internal_name),
                (&o_key, "notequals"),
                (&v_key, v),
            ]);
            idx += 1;
        }
    }
    builder
}
```

Update the import at the top of `src/client/bug.rs` (line 6-7): replace `BOOLEAN_CHART_FIELD_NAMES` with `FIELD_MAPPINGS`:

```rust
    partition_filters, ApiMode, Bug, CreateBugParams, HistoryEntry, SearchParams, UpdateBugParams,
    FIELD_MAPPINGS,
```

- [ ] **Step 8: Rewrite URL parser field matching in `src/url_parser.rs`**

Replace the inline match block (lines ~100-113 on feature branch) with a `FIELD_MAPPINGS` lookup:

```rust
        // Recognized vec fields — map Bugzilla URL param names to SavedQuery fields
        if let Some(mapping) = FIELD_MAPPINGS.iter().find(|m| m.url_param == key) {
            if let Some(target) = query.get_field_mut(mapping.struct_field) {
                target.push(value.to_string());
                continue;
            }
        }
```

Add the import at the top of `src/url_parser.rs`:

```rust
use crate::types::FIELD_MAPPINGS;
```

- [ ] **Step 9: Run full test suite**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 10: Commit**

```bash
git add src/types/bug.rs src/types/mod.rs src/client/bug.rs src/url_parser.rs
git commit -m "refactor: unify field-mapping tables into FIELD_MAPPINGS (#88)"
```

---

### Task 2: Add `into_search_params` (#89)

**Files:**
- Modify: `src/types/bug.rs` (add `into_search_params`)
- Modify: `src/commands/query.rs` (use `into_search_params` in `handle_run`)
- Modify: `src/commands/bug.rs` (use `into_search_params` in `handle_search`)

- [ ] **Step 1: Write test for `into_search_params`**

Add to the `#[cfg(test)] mod tests` block in `src/types/bug.rs`:

```rust
#[test]
fn into_search_params_moves_fields() {
    let query = SavedQuery {
        kind: QueryKind::List,
        product: vec!["Firefox".into()],
        component: vec!["General".into()],
        status: vec!["NEW".into()],
        assignee: vec!["dev@example.com".into()],
        creator: vec!["reporter@example.com".into()],
        priority: vec!["P1".into()],
        severity: vec!["critical".into()],
        quicksearch: Some("crash".into()),
        limit: Some(25),
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
        raw_params: vec![("f1".into(), "qa_contact".into())],
        ..Default::default()
    };
    let params = query.into_search_params();
    assert_eq!(params.product, vec!["Firefox"]);
    assert_eq!(params.component, vec!["General"]);
    assert_eq!(params.status, vec!["NEW"]);
    assert_eq!(params.assigned_to, vec!["dev@example.com"]);
    assert_eq!(params.creator, vec!["reporter@example.com"]);
    assert_eq!(params.priority, vec!["P1"]);
    assert_eq!(params.severity, vec!["critical"]);
    assert_eq!(params.quicksearch, Some("crash".into()));
    assert_eq!(params.limit, Some(25));
    assert_eq!(params.include_fields, Some("id,summary".into()));
    assert_eq!(params.exclude_fields, Some("comments".into()));
    assert_eq!(params.raw_params, vec![("f1".into(), "qa_contact".into())]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib types::bug::tests::into_search_params_moves_fields`
Expected: compilation error — `into_search_params` not defined.

- [ ] **Step 3: Implement `into_search_params`**

Add to `impl SavedQuery` in `src/types/bug.rs`, right after `to_search_params`:

```rust
/// Consuming variant of `to_search_params` — moves fields instead of cloning.
/// Use when the `SavedQuery` is owned and not needed after conversion.
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib types::bug::tests::into_search_params_moves_fields`
Expected: PASS.

- [ ] **Step 5: Update `handle_search` in `src/commands/bug.rs`**

`handle_run` is handled in Task 3 (requires restructuring to extract server first).

For `handle_search` in `src/commands/bug.rs`, the `parsed.query` is owned. Change (around line 142):

```rust
        let mut params = parsed.query.to_search_params();
```

to:

```rust
        let save_query = if save_as.is_some() {
            Some(parsed.query.clone())
        } else {
            None
        };
        let mut params = parsed.query.into_search_params();
```

And update the `save_info` construction (around line 149) to use `save_query`:

```rust
        let save_info = save_as
            .as_ref()
            .zip(save_query)
            .map(|(name, query)| (name.clone(), query));
```

- [ ] **Step 6: Run full test suite**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/types/bug.rs src/commands/bug.rs
git commit -m "perf: add into_search_params to avoid cloning (#89)"
```

---

### Task 3: Use `into_search_params` in `handle_run` (#89 continued)

**Files:**
- Modify: `src/commands/query.rs` (restructure `handle_run` to use `into_search_params`)

- [ ] **Step 1: Restructure `handle_run` to extract server then consume the query**

The query is borrowed from `config.queries`. Since `into_search_params` consumes `self`, extract the server string before consuming. Replace the body of `handle_run` (from the `Config::load()` call onward) with:

```rust
    let config = Config::load()?;
    let saved = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;

    // Extract server before consuming the query
    let saved_server = saved.server.clone();
    let effective_server = server
        .or(server_override.as_deref())
        .or(saved_server.as_deref());

    let mut params = saved.clone().into_search_params();
    params.apply_overrides(*limit, fields.as_deref(), exclude_fields.as_deref());

    let client = super::shared::connect_and_configure(effective_server, api).await?;
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass. The existing `query_run_executes_saved_query` and other run tests validate correctness.

- [ ] **Step 3: Commit**

```bash
git add src/commands/query.rs
git commit -m "perf: use into_search_params in handle_run (#89)"
```

---

### Task 4: Guard Against Boolean Chart Index Collision (#91)

**Files:**
- Modify: `src/client/bug.rs` (add validation in `search_bugs_rest`)

- [ ] **Step 1: Write tests for the collision guard**

Add to the `#[cfg(test)] mod tests` block in `src/client/bug.rs`:

```rust
#[test]
fn has_negated_filters_detects_negation() {
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        ..Default::default()
    };
    assert!(super::has_negated_filters(&params));
}

#[test]
fn has_negated_filters_false_for_positive_only() {
    let params = SearchParams {
        status: vec!["NEW".into()],
        ..Default::default()
    };
    assert!(!super::has_negated_filters(&params));
}

#[test]
fn has_raw_boolean_chart_params_detects_f1() {
    let params = SearchParams {
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "equals".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        ..Default::default()
    };
    assert!(super::has_raw_boolean_chart_params(&params));
}

#[test]
fn has_raw_boolean_chart_params_false_for_non_chart() {
    let params = SearchParams {
        raw_params: vec![("product".into(), "Firefox".into())],
        ..Default::default()
    };
    assert!(!super::has_raw_boolean_chart_params(&params));
}

#[test]
fn has_raw_boolean_chart_params_false_for_empty() {
    let params = SearchParams::default();
    assert!(!super::has_raw_boolean_chart_params(&params));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib client::bug::tests::has_negated`
Expected: compilation error — functions don't exist.

- [ ] **Step 3: Implement the detection helpers**

Add above `search_bugs_rest` in `src/client/bug.rs`:

```rust
/// Returns true if any multi-value filter field contains negated values (prefixed with `!`).
fn has_negated_filters(params: &SearchParams) -> bool {
    FIELD_MAPPINGS.iter().any(|m| {
        params
            .get_field(m.struct_field)
            .iter()
            .any(|v| v.starts_with('!'))
    })
}

/// Returns true if `raw_params` contains boolean chart parameters (`fN`, `oN`, `vN`
/// where N is a positive integer).
fn has_raw_boolean_chart_params(params: &SearchParams) -> bool {
    params.raw_params.iter().any(|(k, _)| {
        k.len() >= 2
            && k.as_bytes()[0].is_ascii_lowercase()
            && matches!(k.as_bytes()[0], b'f' | b'o' | b'v')
            && k[1..].parse::<u32>().is_ok()
    })
}
```

- [ ] **Step 4: Run helper tests to verify they pass**

Run: `cargo test --lib client::bug::tests::has_negated && cargo test --lib client::bug::tests::has_raw_boolean`
Expected: all pass.

- [ ] **Step 5: Add the validation to `search_bugs_rest`**

In `search_bugs_rest`, replace the comment block (around lines 203-207 on feature branch):

```rust
        // Note: raw_params and append_negated_params both use fN/oN/vN indices.
        // Index collision cannot occur because URL-parsed queries store boolean
        // chart params in raw_params (not as negated structured filters), and
        // there is no CLI path that combines negated filters with raw params.
        req_builder = append_raw_params(req_builder, &params.raw_params);
```

with:

```rust
        if has_negated_filters(params) && has_raw_boolean_chart_params(params) {
            return Err(crate::error::BzrError::InputValidation(
                "cannot combine negated filters (e.g. --status '!CLOSED') with a \
                 URL-imported query containing boolean chart parameters; the chart \
                 indices would collide"
                    .into(),
            ));
        }
        req_builder = append_raw_params(req_builder, &params.raw_params);
```

- [ ] **Step 6: Write integration test for the collision error**

Add to the tests in `src/client/bug.rs`:

```rust
#[tokio::test]
async fn search_bugs_rejects_negated_plus_raw_boolean_chart() {
    let mock = wiremock::MockServer::start().await;
    let client = crate::client::BugzillaClient::new(
        &mock.uri(),
        None,
        crate::types::ApiMode::Rest,
    );
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "equals".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        ..Default::default()
    };
    let result = client.search_bugs(&params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("cannot combine negated filters"),
        "unexpected error: {err}"
    );
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/client/bug.rs
git commit -m "fix: guard against boolean chart index collision (#91)"
```

---

### Task 5: Migrate `output/query.rs` from `println!` to `writeln!` (#90)

**Files:**
- Modify: `src/output/query.rs`

- [ ] **Step 1: Replace `println!` with `writeln!` in `print_query_saved`**

Add import at the top of `src/output/query.rs`:

```rust
use std::io::{self, Write as _};
```

In `print_query_saved`, replace:

```rust
        OutputFormat::Table => {
            println!("{}", query_saved_message(name, verb));
        }
```

with:

```rust
        OutputFormat::Table => {
            let _ = writeln!(io::stdout(), "{}", query_saved_message(name, verb));
        }
```

- [ ] **Step 2: Replace `println!` in `print_query_list`**

In `print_query_list`, replace:

```rust
        if queries.is_empty() {
            println!("No saved queries configured.");
            return;
        }
```

with:

```rust
        if queries.is_empty() {
            let _ = writeln!(io::stdout(), "No saved queries configured.");
            return;
        }
```

And replace:

```rust
            println!("{}", query_summary_line(name, &queries[name]));
```

with:

```rust
            let _ = writeln!(io::stdout(), "{}", query_summary_line(name, &queries[name]));
```

- [ ] **Step 3: Remove any `#[expect(clippy::print_stdout)]` attributes**

Check if these functions have `#[expect(clippy::print_stdout)]` annotations and remove them if present.

- [ ] **Step 4: Run tests and clippy**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass. The existing `capture_stdout` tests should continue to work (and may actually capture output better now).

- [ ] **Step 5: Commit**

```bash
git add src/output/query.rs
git commit -m "fix: migrate output/query.rs from println to writeln (#90)"
```

---

### Task 6: Strengthen Server Override Test (#93)

**Files:**
- Modify: `src/commands/query.rs` (rewrite `query_run_with_server_override` test)

- [ ] **Step 1: Rewrite the test**

Replace the `query_run_with_server_override` test (around line 609 on feature branch) with:

```rust
    #[tokio::test]
    async fn query_run_with_server_override() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // Save a query that records a different server than the mock
        let save_action = save_action("server-test");
        let (result, _) =
            capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());

        // Patch the saved query to have a different server
        let mut config = Config::load().unwrap();
        let query = config.queries.get_mut("server-test").unwrap();
        query.server = Some("other-server".into());
        config.save().unwrap();

        // Mount a mock that expects exactly 1 request
        let mock_guard = Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})),
            )
            .expect(1)
            .mount_as_scoped(&mock)
            .await;

        // Run with --server override pointing to the mock server ("test")
        let run_action = QueryAction::Run {
            name: "server-test".into(),
            limit: None,
            fields: None,
            exclude_fields: None,
            server: Some("test".into()),
        };
        let (result, _) = capture_stdout(super::execute(
            &run_action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(
            result.is_ok(),
            "query run with server override failed: {result:?}"
        );

        // Drop the scoped mock to trigger the expect(1) assertion
        drop(mock_guard);
    }
```

Key changes from the original:
1. The saved query now has `server: Some("other-server")` — a server that doesn't exist as a mock.
2. The `--server` override uses `"test"` which points to the wiremock server.
3. The global server param is `None` (not `Some("test")`), so only the action-level override can route to the mock.
4. `.expect(1)` verifies the mock was actually hit.

- [ ] **Step 2: Run the test**

Run: `cargo test --lib commands::query::tests::query_run_with_server_override`
Expected: PASS. The override should route to "test" (the mock), not "other-server".

- [ ] **Step 3: Commit**

```bash
git add src/commands/query.rs
git commit -m "test: strengthen server override test to verify precedence (#93)"
```

---

### Task 7: Auto-Suggest Save Name from URL's `known_name` (#92)

**Files:**
- Modify: `src/url_parser.rs` (extract `known_name`, add `suggested_name` to `ParsedUrl`)
- Modify: `src/cli/bug.rs` (make `--save-as` optional argument)
- Modify: `src/commands/bug.rs` (use `suggested_name` when `--save-as` has no explicit name)

- [ ] **Step 1: Write tests for `suggested_name` extraction**

Add to the `#[cfg(test)] mod tests` block in `src/url_parser.rs`:

```rust
#[test]
fn parses_known_name_into_suggested_name() {
    let config = test_config();
    let result = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&known_name=my%20saved%20search",
        &config,
    )
    .unwrap();
    assert_eq!(
        result.suggested_name,
        Some("my saved search".into())
    );
}

#[test]
fn prefers_known_name_over_query_based_on() {
    let config = test_config();
    let result = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&known_name=preferred&query_based_on=ancestor",
        &config,
    )
    .unwrap();
    assert_eq!(
        result.suggested_name,
        Some("preferred".into())
    );
}

#[test]
fn falls_back_to_query_based_on() {
    let config = test_config();
    let result = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&query_based_on=ancestor%20query",
        &config,
    )
    .unwrap();
    assert_eq!(
        result.suggested_name,
        Some("ancestor query".into())
    );
}

#[test]
fn no_suggested_name_when_absent() {
    let config = test_config();
    let result = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox",
        &config,
    )
    .unwrap();
    assert!(result.suggested_name.is_none());
}

#[test]
fn empty_known_name_ignored() {
    let config = test_config();
    let result = parse_bugzilla_url(
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&known_name=",
        &config,
    )
    .unwrap();
    assert!(result.suggested_name.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib url_parser::tests::parses_known_name`
Expected: failure — `suggested_name` field doesn't exist on `ParsedUrl`.

- [ ] **Step 3: Implement `suggested_name` extraction**

In `src/url_parser.rs`, update `IGNORED_PARAMS` to remove `known_name` and `query_based_on`:

```rust
const IGNORED_PARAMS: &[&str] = &["columnlist", "list_id", "query_format"];
```

Update `ParsedUrl` struct:

```rust
#[derive(Debug)]
pub struct ParsedUrl {
    pub query: SavedQuery,
    /// Suggested name extracted from URL's `known_name` or `query_based_on` param.
    pub suggested_name: Option<String>,
}
```

In `parse_bugzilla_url`, add tracking variables before the `for` loop:

```rust
    let mut known_name: Option<String> = None;
    let mut query_based_on: Option<String> = None;
```

In the `for` loop, add handling before the credential check:

```rust
        if key == "known_name" {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                known_name = Some(trimmed.to_string());
            }
            continue;
        }

        if key == "query_based_on" {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                query_based_on = Some(trimmed.to_string());
            }
            continue;
        }
```

Update the return to include `suggested_name`:

```rust
    Ok(ParsedUrl {
        query,
        suggested_name: known_name.or(query_based_on),
    })
```

- [ ] **Step 4: Run url_parser tests to verify they pass**

Run: `cargo test --lib url_parser::tests`
Expected: all pass.

- [ ] **Step 5: Make `--save-as` accept an optional name**

In `src/cli/bug.rs`, change the `save_as` field (around line 64-65):

```rust
        #[arg(long, requires = "from_url")]
        save_as: Option<String>,
```

to:

```rust
        /// Save this URL query for future reuse. Optionally provide a name;
        /// if omitted, uses the URL's known_name parameter.
        #[arg(long, requires = "from_url", num_args = 0..=1, default_missing_value = "")]
        save_as: Option<String>,
```

- [ ] **Step 6: Update `handle_search` to use `suggested_name`**

In `src/commands/bug.rs`, update the `save_info` construction (around line 149) to handle the empty-string sentinel:

Replace:

```rust
        let save_info = save_as
            .as_ref()
            .map(|name| (name.clone(), parsed.query.clone()));
```

(Or the version from Task 2 if already modified.)

With:

```rust
        let save_info = if let Some(raw_name) = save_as {
            let name = if raw_name.is_empty() {
                parsed.suggested_name.ok_or_else(|| {
                    crate::error::BzrError::InputValidation(
                        "no name provided for --save-as and URL has no known_name; \
                         specify a name explicitly: --save-as <name>"
                            .into(),
                    )
                })?
            } else {
                raw_name.clone()
            };
            Some((name, parsed.query.clone()))
        } else {
            None
        };
```

Note: if Task 2 changed this code to use `into_search_params`, adapt accordingly — clone the query for `save_info` before consuming it.

- [ ] **Step 7: Write integration tests**

Add to the tests in `src/commands/bug.rs`:

```rust
#[tokio::test]
async fn handle_search_from_url_auto_names_from_known_name() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})),
        )
        .mount(&mock)
        .await;

    let server_url = mock.uri();
    let url = format!(
        "{server_url}/buglist.cgi?product=TestProduct&known_name=my%20saved%20search"
    );
    // --save-as with no name (empty string sentinel)
    let action = BugAction::Search {
        query: None,
        from_url: Some(url),
        save_as: Some(String::new()),
        limit: None,
        fields: None,
        exclude_fields: None,
    };
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "auto-name from known_name failed: {result:?}");

    let config = Config::load().unwrap();
    assert!(
        config.queries.contains_key("my saved search"),
        "query should be saved as 'my saved search'"
    );
}

#[tokio::test]
async fn handle_search_save_as_no_name_no_known_name_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::Search {
        query: None,
        from_url: Some("https://bugzilla.example.com/buglist.cgi?product=Firefox".into()),
        save_as: Some(String::new()),
        limit: None,
        fields: None,
        exclude_fields: None,
    };
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("no name provided for --save-as"),
        "unexpected error: {err}"
    );
}
```

- [ ] **Step 8: Run full test suite**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add src/url_parser.rs src/cli/bug.rs src/commands/bug.rs
git commit -m "feat: auto-suggest save name from URL's known_name (#92)"
```

---

### Task 8: Final Verification

- [ ] **Step 1: Run full test suite and lints**

```bash
cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check
```

Expected: all pass, no warnings, no formatting issues.

- [ ] **Step 2: Verify all issues are addressed**

Confirm each issue:
- #88: `BOOLEAN_CHART_FIELD_NAMES` removed, `FIELD_MAPPINGS` used everywhere
- #89: `into_search_params` exists and is used in owning call sites
- #90: No `println!` in `output/query.rs`
- #91: `search_bugs_rest` rejects negated + raw boolean chart collision
- #92: `--save-as` without a name auto-fills from `known_name`
- #93: Server override test verifies a different saved server is overridden

- [ ] **Step 3: Update CLI docs if needed**

If `docs/bzr-cli.md` documents `--save-as`, update it to mention the optional name behavior.
