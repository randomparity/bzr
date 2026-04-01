# Field Name Alias Resolution for `bzr field list` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow `bzr field list status` to work by translating user-friendly field names (e.g. `status`) to the internal Bugzilla API field names (e.g. `bug_status`) that the `/rest/field/bug/` endpoint expects.

**Architecture:** Add a field name alias mapping function in `src/client/field.rs` that translates common short names to their `bug_*` internal equivalents before calling the API. The `/rest/field/bug/{name}` endpoint requires internal names like `bug_status`, while the `/rest/bug/` search endpoint accepts the short `status` form — this inconsistency is the root cause of GitHub issue #41.

**Tech Stack:** Rust, no new dependencies

---

## Background

The Bugzilla REST API has an inconsistency:
- **Bug search endpoint** (`/rest/bug/`): accepts short field names like `status`, `severity`
- **Field endpoint** (`/rest/field/bug/{name}`): requires internal names like `bug_status`, `bug_severity`

When a user runs `bzr field list status`, the CLI passes `status` directly to `/rest/field/bug/status`, and Bugzilla returns error code 51: "There is no fieldBugzilla::Field named 'status'."

The fix adds a small alias table that maps user-friendly names to internal names before calling the API. The CLI help text already suggests names like `status`, `priority`, `severity`, `resolution` — so we must make those work.

## File Structure

- **Modify:** `src/client/field.rs` — add alias resolution function and apply it in `get_field_values()`
- **Modify:** `src/cli/field.rs` — update help text to note that common aliases are supported
- **Modify:** `docs/bzr-cli.md` — document the alias behavior

## Known Bugzilla Field Name Mappings

These are the common fields where the internal name differs from the user-friendly name:

| User-friendly | Internal (API) |
|--------------|----------------|
| `status` | `bug_status` |
| `severity` | `bug_severity` |
| `id` | `bug_id` |
| `type` | `bug_type` |
| `group` | `bug_group` |
| `file_loc` | `bug_file_loc` |

Fields that already match their internal name (no alias needed): `priority`, `resolution`, `product`, `component`, `version`, `assigned_to`, `op_sys`, `rep_platform`, `keywords`, `whiteboard`, `cc`, `blocks`, `depends_on`, `creator`, `creation_time`, `last_change_time`.

---

### Task 1: Add Field Name Alias Resolution with Tests

**Files:**
- Modify: `src/client/field.rs`

- [ ] **Step 1: Write the failing test for alias resolution**

Add a unit test in the `tests` module of `src/client/field.rs` that verifies the alias function maps `"status"` to `"bug_status"`:

```rust
#[test]
fn resolve_field_alias_maps_status() {
    assert_eq!(super::resolve_field_alias("status"), "bug_status");
}

#[test]
fn resolve_field_alias_maps_severity() {
    assert_eq!(super::resolve_field_alias("severity"), "bug_severity");
}

#[test]
fn resolve_field_alias_maps_id() {
    assert_eq!(super::resolve_field_alias("id"), "bug_id");
}

#[test]
fn resolve_field_alias_maps_type() {
    assert_eq!(super::resolve_field_alias("type"), "bug_type");
}

#[test]
fn resolve_field_alias_maps_group() {
    assert_eq!(super::resolve_field_alias("group"), "bug_group");
}

#[test]
fn resolve_field_alias_maps_file_loc() {
    assert_eq!(super::resolve_field_alias("file_loc"), "bug_file_loc");
}

#[test]
fn resolve_field_alias_passes_through_unknown() {
    assert_eq!(super::resolve_field_alias("priority"), "priority");
}

#[test]
fn resolve_field_alias_passes_through_already_prefixed() {
    assert_eq!(super::resolve_field_alias("bug_status"), "bug_status");
}

#[test]
fn resolve_field_alias_is_case_insensitive() {
    assert_eq!(super::resolve_field_alias("Status"), "bug_status");
    assert_eq!(super::resolve_field_alias("SEVERITY"), "bug_severity");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib client::field::tests`
Expected: compilation errors — `resolve_field_alias` does not exist yet.

- [ ] **Step 3: Implement `resolve_field_alias`**

Add this function above the `impl BugzillaClient` block in `src/client/field.rs`:

```rust
/// Translate user-friendly field names to the internal Bugzilla field names
/// expected by the `/rest/field/bug/{name}` endpoint.
///
/// The bug search endpoint (`/rest/bug/`) accepts short names like `status`,
/// but the field endpoint requires `bug_status`. This function bridges that
/// gap so users can type the intuitive name.
fn resolve_field_alias(name: &str) -> &str {
    match name.to_ascii_lowercase().as_str() {
        "status" => "bug_status",
        "severity" => "bug_severity",
        "id" => "bug_id",
        "type" => "bug_type",
        "group" => "bug_group",
        "file_loc" => "bug_file_loc",
        _ => name,
    }
}
```

Note: The function returns `&str` — for alias matches it returns a static string, for the fallthrough it returns the original `name`. Since `to_ascii_lowercase()` creates a temporary, we need to adjust the approach. The match on the lowercased value determines which static alias to return, but the fallthrough case returns the original `name` unchanged:

```rust
fn resolve_field_alias(name: &str) -> Cow<'_, str> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "status" => Cow::Borrowed("bug_status"),
        "severity" => Cow::Borrowed("bug_severity"),
        "id" => Cow::Borrowed("bug_id"),
        "type" => Cow::Borrowed("bug_type"),
        "group" => Cow::Borrowed("bug_group"),
        "file_loc" => Cow::Borrowed("bug_file_loc"),
        _ => Cow::Borrowed(name),
    }
}
```

Add `use std::borrow::Cow;` at the top of the file.

Update the test assertions to compare with `&str` using `.as_ref()` or `==`:

```rust
#[test]
fn resolve_field_alias_maps_status() {
    assert_eq!(super::resolve_field_alias("status").as_ref(), "bug_status");
}
```

(Apply the same `.as_ref()` pattern to all the alias tests.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib client::field::tests`
Expected: all `resolve_field_alias_*` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/client/field.rs
git commit -m "feat: add field name alias resolution for Bugzilla field endpoint"
```

---

### Task 2: Wire Alias Resolution into `get_field_values`

**Files:**
- Modify: `src/client/field.rs`

- [ ] **Step 1: Write a test that calls `get_field_values("status")` expecting the API call to go to `/rest/field/bug/bug_status`**

Add a new integration-style unit test in `src/client/field.rs` that verifies the alias is applied when calling the API:

```rust
#[tokio::test]
async fn get_field_values_resolves_status_alias() {
    let mock = MockServer::start().await;
    // Mount mock on the INTERNAL name path
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "values": [
                    {"name": "NEW", "sort_key": 100, "is_active": true},
                    {"name": "ASSIGNED", "sort_key": 200, "is_active": true}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let values = client.get_field_values("status").await.unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].name, "NEW");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib client::field::tests::get_field_values_resolves_status_alias`
Expected: FAIL — the client calls `/rest/field/bug/status` (no alias applied yet), which doesn't match the mock mounted at `/rest/field/bug/bug_status`.

- [ ] **Step 3: Apply alias resolution in `get_field_values`**

Modify the `get_field_values` method to call `resolve_field_alias` before building the URL:

```rust
pub async fn get_field_values(&self, field_name: &str) -> Result<Vec<FieldValue>> {
    let resolved = resolve_field_alias(field_name);
    let data: FieldBugResponse =
        self.get_json(&format!("field/bug/{resolved}")).await?;
    let field = data
        .fields
        .into_iter()
        .next()
        .ok_or_else(|| BzrError::NotFound {
            resource: "field",
            id: field_name.to_string(),
        })?;
    Ok(field.values)
}
```

Note: The `NotFound` error still uses the original `field_name` (what the user typed), not the resolved name — this keeps error messages user-friendly.

- [ ] **Step 4: Update the existing `get_field_values_returns_values` test**

The existing test mounts at `/rest/field/bug/status` — but now `get_field_values("status")` will resolve to `bug_status`. Update the mock path:

```rust
#[tokio::test]
async fn get_field_values_returns_values() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))  // <-- changed from /status
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "values": [
                    {"name": "NEW", "sort_key": 100, "is_active": true, "can_change_to": [{"name": "ASSIGNED"}, {"name": "RESOLVED"}]},
                    {"name": "RESOLVED", "sort_key": 500, "is_active": true}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let values = client.get_field_values("status").await.unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].name, "NEW");
    let transitions = values[0].can_change_to.as_ref().unwrap();
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].name, "ASSIGNED");
}
```

- [ ] **Step 5: Run all field tests to verify they pass**

Run: `cargo test --lib client::field::tests`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/client/field.rs
git commit -m "fix: resolve field name aliases in get_field_values

Fixes #41 — 'bzr field list status' now works by translating 'status'
to 'bug_status' before calling the Bugzilla field endpoint."
```

---

### Task 3: Update the Integration Test

**Files:**
- Modify: `src/commands/field.rs`

- [ ] **Step 1: Read the current integration test**

Read `src/commands/field.rs` to see the full test code and the mock path used.

- [ ] **Step 2: Update the integration test mock path**

The integration test in `src/commands/field.rs` mounts a mock at `/rest/field/bug/status`. Since `get_field_values("status")` now resolves to `bug_status`, update the mock:

```rust
Mock::given(method("GET"))
    .and(path("/rest/field/bug/bug_status"))  // <-- changed from /status
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "fields": [{
            "name": "bug_status",  // <-- updated to match internal name
            "values": [
                {"name": "NEW"},
                {"name": "ASSIGNED"},
                {"name": "RESOLVED"}
            ]
        }]
    })))
    .mount(&mock)
    .await;
```

The rest of the test (creating `FieldAction::List { name: "status".to_string() }` and asserting the output) stays the same.

- [ ] **Step 3: Run the integration test**

Run: `cargo test --lib commands::field::tests`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/commands/field.rs
git commit -m "test: update field integration test for alias resolution"
```

---

### Task 4: Update CLI Help Text and Documentation

**Files:**
- Modify: `src/cli/field.rs`
- Modify: `docs/bzr-cli.md`

- [ ] **Step 1: Update the CLI help text**

In `src/cli/field.rs`, update the doc comment on the `name` field:

```rust
#[derive(Subcommand)]
pub enum FieldAction {
    /// List valid values for a bug field
    List {
        /// Field name (e.g. status, priority, severity, resolution).
        /// Common aliases are resolved automatically (status -> bug_status,
        /// severity -> bug_severity, etc.)
        name: String,
    },
}
```

- [ ] **Step 2: Update the CLI reference docs**

In `docs/bzr-cli.md`, find the `bzr field` section (around line 533-543) and add a note about alias support. After the example code block:

```markdown
## `bzr field` -- Field Value Lookup

### `bzr field list`

List valid values for a bug field (e.g. status, priority, severity, resolution). For status fields, shows allowed state transitions.

Common field name aliases are resolved automatically:

| You type | API field name |
|----------|---------------|
| `status` | `bug_status` |
| `severity` | `bug_severity` |
| `id` | `bug_id` |
| `type` | `bug_type` |
| `group` | `bug_group` |
| `file_loc` | `bug_file_loc` |

Fields without aliases (e.g. `priority`, `resolution`) are passed through as-is.

```bash
bzr field list status
bzr field list priority
bzr --json field list severity
```
```

- [ ] **Step 3: Run `cargo clippy -- -D warnings` and `cargo fmt`**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: no warnings, no formatting changes.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/cli/field.rs docs/bzr-cli.md
git commit -m "docs: document field name alias resolution"
```
