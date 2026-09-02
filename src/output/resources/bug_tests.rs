#![expect(clippy::unwrap_used)]

use super::*;
use crate::commands::runtime::search::fields::{
    validate_json_field_selection, validate_table_columns, warn_unknown_fields,
};
use crate::types::bug::{
    BugAdjacencyBug, BugAdjacencyError, BugAdjacencyRequest, BugAdjacencyResult, BugLink,
    ColumnSpec, LinkRelation, BUG_FIELDS,
};
use crate::types::{Bug, FieldChange, Flag, HistoryEntry};

fn review_flag(status: &str, requestee: Option<&str>) -> Flag {
    Flag {
        name: Some("review".into()),
        status: Some(status.into()),
        setter: Some("alice@example.com".into()),
        requestee: requestee.map(Into::into),
    }
}

fn make_bug(id: u64, summary: &str, status: &str) -> Bug {
    Bug {
        id,
        summary: Some(summary.into()),
        status: Some(status.into()),
        resolution: None,
        dupe_of: None,
        deadline: None,
        product: Some("TestProduct".into()),
        component: Some(vec!["TestComponent".into()]),
        version: Some(vec!["1.0".into()]),
        assigned_to: Some("dev@example.com".into()),
        priority: Some("P1".into()),
        severity: Some("major".into()),
        creation_time: Some("2025-01-15T10:00:00Z".into()),
        last_change_time: Some("2025-01-16T12:00:00Z".into()),
        creator: Some("reporter@example.com".into()),
        url: None,
        whiteboard: None,
        keywords: vec!["regression".into()],
        blocks: vec![200, 201],
        depends_on: vec![100],
        cc: vec!["watcher@example.com".into()],
        op_sys: None,
        platform: None,
        target_milestone: None,
        groups: vec![],
        estimated_time: None,
        remaining_time: None,
        flags: Vec::new(),
        custom_fields: std::collections::BTreeMap::new(),
    }
}

fn make_bug_with_custom(id: u64, summary: &str, status: &str) -> Bug {
    let mut bug = make_bug(id, summary, status);
    bug.custom_fields
        .insert("cf_release".into(), serde_json::json!("9.6"));
    bug.custom_fields
        .insert("cf_score".into(), serde_json::json!(12));
    bug
}

fn make_history_entry() -> HistoryEntry {
    HistoryEntry {
        who: "editor@example.com".into(),
        when: "2025-04-01T12:00:00Z".into(),
        changes: vec![
            FieldChange {
                field_name: "status".into(),
                removed: Some("NEW".into()),
                added: Some("ASSIGNED".into()),
                attachment_id: None,
            },
            FieldChange {
                field_name: "flagtypes.name".into(),
                removed: Some(String::new()),
                added: Some("review?".into()),
                attachment_id: Some(99),
            },
        ],
    }
}

fn capture_bugs(format: OutputFormat, bugs: &[Bug]) -> String {
    capture_bugs_spec(format, bugs, ColumnSpec::default()).0
}

/// Returns (stdout, stderr) so column-selection tests can assert warnings.
fn capture_bugs_spec(format: OutputFormat, bugs: &[Bug], spec: ColumnSpec<'_>) -> (String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    write_bugs(bugs, spec, format, &mut out, &mut err);
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn capture_bug_detail(format: OutputFormat, bug: &Bug) -> String {
    let mut buf = Vec::new();
    write_bug_detail(bug, ColumnSpec::default(), format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_detail_spec(bug: &Bug, spec: ColumnSpec<'_>) -> String {
    let mut out = Vec::new();
    write_bug_detail(bug, spec, OutputFormat::Table, &mut out);
    String::from_utf8(out).unwrap()
}

fn capture_history_table(history: &[HistoryEntry]) -> String {
    let mut buf = Vec::new();
    write_history_table(history, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_history_json(records: &[HistoryRecord], format: OutputFormat) -> String {
    let mut buf = Vec::new();
    write_history_json(records, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn sample_adjacency() -> BugAdjacencyResult {
    BugAdjacencyResult {
        requests: vec![
            BugAdjacencyRequest::Success {
                requested: "00123".into(),
                bug_id: 123,
            },
            BugAdjacencyRequest::Success {
                requested: "release-alias".into(),
                bug_id: 123,
            },
            BugAdjacencyRequest::Success {
                requested: "release-alias".into(),
                bug_id: 123,
            },
            BugAdjacencyRequest::Failure {
                requested: "missing-alias".into(),
                error: BugAdjacencyError::NotFoundAlias,
            },
            BugAdjacencyRequest::Failure {
                requested: "999999".into(),
                error: BugAdjacencyError::NotFoundId,
            },
            BugAdjacencyRequest::Failure {
                requested: "restricted".into(),
                error: BugAdjacencyError::Inaccessible,
            },
        ],
        bugs: vec![
            BugAdjacencyBug {
                id: 42,
                summary: None,
                status: None,
                resolution: None,
                product: None,
                version: None,
                assigned_to: None,
                last_change_time: None,
                target_milestone: None,
                blocks: vec![],
                depends_on: vec![],
            },
            BugAdjacencyBug {
                id: 123,
                summary: Some("Example".into()),
                status: Some("NEW".into()),
                resolution: None,
                product: Some("Example Product".into()),
                version: Some(vec!["unspecified".into()]),
                assigned_to: Some("owner@example.invalid".into()),
                last_change_time: Some("2026-08-29T00:00:00Z".into()),
                target_milestone: Some("---".into()),
                blocks: vec![300, 200, 200],
                depends_on: vec![20, 10, 20],
            },
        ],
    }
}

fn capture_bug_adjacency(format: OutputFormat, mut result: BugAdjacencyResult) -> String {
    let mut buf = Vec::new();
    write_bug_adjacency(&mut result, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn expected_adjacency_payload() -> serde_json::Value {
    serde_json::json!({
        "requests": [
            {"requested": "00123", "bug_id": 123},
            {"requested": "release-alias", "bug_id": 123},
            {"requested": "release-alias", "bug_id": 123},
            {"requested": "missing-alias", "error": {"type": "not_found", "api_code": 100}},
            {"requested": "999999", "error": {"type": "not_found", "api_code": 101}},
            {"requested": "restricted", "error": {"type": "inaccessible", "api_code": 102}}
        ],
        "bugs": [
            {
                "id": 42,
                "summary": null,
                "status": null,
                "resolution": null,
                "product": null,
                "version": null,
                "assigned_to": null,
                "last_change_time": null,
                "target_milestone": null,
                "blocks": [],
                "depends_on": []
            },
            {
                "id": 123,
                "summary": "Example",
                "status": "NEW",
                "resolution": null,
                "product": "Example Product",
                "version": ["unspecified"],
                "assigned_to": "owner@example.invalid",
                "last_change_time": "2026-08-29T00:00:00Z",
                "target_milestone": "---",
                "blocks": [200, 300],
                "depends_on": [10, 20]
            }
        ]
    })
}

#[test]
fn write_bug_adjacency_json_has_the_closed_result_shape() {
    let output = capture_bug_adjacency(OutputFormat::Json, sample_adjacency());
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version": crate::output::SCHEMA_VERSION,
            "data": expected_adjacency_payload(),
        })
    );
}

#[test]
fn write_bug_adjacency_json_uses_schema_version_2_1_0() {
    let output = capture_bug_adjacency(OutputFormat::Json, sample_adjacency());
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["schema_version"], "2.1.0");
}

#[test]
fn write_bug_adjacency_ndjson_is_one_compact_result_record() {
    let result = sample_adjacency();
    let output = capture_bug_adjacency(OutputFormat::Ndjson, result);
    assert_eq!(output.lines().count(), 1);
    assert!(!output.contains("  \""));
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value, expected_adjacency_payload());
}

#[test]
fn write_bug_adjacency_table_has_request_and_canonical_bug_sections() {
    let output = capture_bug_adjacency(OutputFormat::Table, sample_adjacency());
    assert!(output.contains("Requests"), "request section:\n{output}");
    assert!(output.contains("Canonical bugs"), "bug section:\n{output}");
    assert!(output.contains("00123"), "request order:\n{output}");
    assert!(output.contains("release-alias"), "alias request:\n{output}");
    assert_eq!(
        output.matches("release-alias").count(),
        2,
        "duplicates:\n{output}"
    );
    assert!(
        output.contains("NOT FOUND (100)"),
        "not found alias:\n{output}"
    );
    assert!(output.contains("NOT FOUND (101)"), "not found:\n{output}");
    assert!(
        output.contains("INACCESSIBLE (102)"),
        "inaccessible:\n{output}"
    );
    assert!(output.contains("200, 300"), "blocks:\n{output}");
    assert!(output.contains("10, 20"), "depends_on:\n{output}");
    assert!(
        output.find("00123").unwrap() < output.find("release-alias").unwrap()
            && output.rfind("release-alias").unwrap() < output.find("missing-alias").unwrap()
            && output.find("missing-alias").unwrap() < output.find("Canonical bugs").unwrap()
            && output.find("Canonical bugs").unwrap() < output.find("42").unwrap()
            && output.find("42").unwrap() < output.rfind("123").unwrap(),
        "section and row ordering:\n{output}"
    );
}

#[test]
fn write_bug_adjacency_normalizes_edge_arrays_in_place() {
    let mut result = sample_adjacency();
    let mut output = Vec::new();

    write_bug_adjacency(&mut result, OutputFormat::Json, &mut output);

    assert_eq!(result.bugs[1].blocks, vec![200, 300]);
    assert_eq!(result.bugs[1].depends_on, vec![10, 20]);
}

fn sample_record(field: &str, comment_id: Option<u64>) -> HistoryRecord {
    HistoryRecord {
        when: "2025-04-01T12:00:00Z".into(),
        who: "editor@example.com".into(),
        field: field.into(),
        old_value: Some("NEW".into()),
        new_value: Some("ASSIGNED".into()),
        comment_id,
    }
}

// ── write_bugs ───────────────────────────────────────────────────

#[test]
fn write_bugs_json_empty_list() {
    let bugs: Vec<Bug> = vec![];
    let json = serde_json::to_string_pretty(&bugs).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_bugs_json_one_bug() {
    let bugs = vec![make_bug(42, "Login broken", "NEW")];
    let json = serde_json::to_string_pretty(&bugs).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["id"], 42);
    assert_eq!(parsed[0]["summary"], "Login broken");
}

#[test]
fn write_bugs_table_empty_says_no_bugs_found() {
    let output = capture_bugs(OutputFormat::Table, &[]);
    assert!(output.contains("No bugs found."));
}

#[test]
fn write_bugs_ndjson_emits_one_object_per_line() {
    let bugs = vec![
        make_bug(1, "first", "NEW"),
        make_bug(2, "second", "ASSIGNED"),
    ];
    let output = capture_bugs(OutputFormat::Ndjson, &bugs);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(first["id"], 1);
    assert_eq!(second["id"], 2);
    // Compact: no pretty-print indentation on any line.
    assert!(!output.contains("  \""));
}

#[test]
fn write_bugs_ndjson_empty_emits_nothing() {
    let output = capture_bugs(OutputFormat::Ndjson, &[]);
    assert!(output.is_empty());
}

#[test]
fn write_bug_detail_ndjson_emits_single_line() {
    let bug = make_bug(99, "detail", "NEW");
    let output = capture_bug_detail(OutputFormat::Ndjson, &bug);
    assert_eq!(output.lines().count(), 1);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["id"], 99);
}

#[test]
fn write_bugs_json_empty_renders_empty_array() {
    let output = capture_bugs(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn write_bugs_table_renders_columns_and_truncates() {
    let bugs = vec![make_bug(42, "Login broken", "NEW")];
    let output = capture_bugs(OutputFormat::Table, &bugs);
    assert!(output.contains("ID"));
    assert!(output.contains("STATUS"));
    assert!(output.contains("PRIORITY"));
    assert!(output.contains("ASSIGNEE"));
    assert!(output.contains("SUMMARY"));
    assert!(output.contains("42"));
    assert!(output.contains("NEW"));
    assert!(output.contains("P1"));
    assert!(output.contains("dev"));
    assert!(output.contains("Login broken"));
}

#[test]
fn write_bugs_json_via_write() {
    let bugs = vec![make_bug(99, "Crash on startup", "ASSIGNED")];
    let output = capture_bugs(OutputFormat::Json, &bugs);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 99);
    assert_eq!(parsed[0]["summary"], "Crash on startup");
    assert_eq!(parsed[0]["status"], "ASSIGNED");
}

#[test]
fn write_bugs_fields_selects_only_requested_columns() {
    let bugs = vec![make_bug(217_559, "kernel panic on boot", "ASSIGNED")];
    let spec = ColumnSpec {
        include: Some("id,priority"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(out.contains("ID"), "ID column present:\n{out}");
    assert!(out.contains("PRIORITY"), "PRIORITY column present:\n{out}");
    assert!(!out.contains("STATUS"), "STATUS must be absent:\n{out}");
    assert!(!out.contains("ASSIGNEE"), "ASSIGNEE must be absent:\n{out}");
    assert!(!out.contains("SUMMARY"), "SUMMARY must be absent:\n{out}");
    assert!(err.is_empty(), "no warning for known fields: {err:?}");
}

#[test]
fn write_bugs_fields_adds_non_default_columns_populated() {
    let bugs = vec![make_bug(218_034, "LPM crash", "WORKING")];
    let spec = ColumnSpec {
        include: Some("id,priority,severity,status,product,summary"),
        exclude: None,
    };
    let (out, _err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    for header in ["ID", "PRIORITY", "SEVERITY", "STATUS", "PRODUCT", "SUMMARY"] {
        assert!(out.contains(header), "{header} column present:\n{out}");
    }
    // make_bug populates severity=major and product=TestProduct.
    assert!(out.contains("major"), "severity value rendered:\n{out}");
    assert!(
        out.contains("TestProduct"),
        "product value rendered:\n{out}"
    );
}

#[test]
fn write_bugs_no_fields_keeps_default_columns() {
    let bugs = vec![make_bug(1, "summary text", "NEW")];
    let out = capture_bugs(OutputFormat::Table, &bugs);
    for header in ["ID", "STATUS", "PRIORITY", "ASSIGNEE", "SUMMARY"] {
        assert!(out.contains(header), "default header {header}:\n{out}");
    }
}

#[test]
fn write_bugs_exclude_fields_drops_default_column() {
    let bugs = vec![make_bug(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: None,
        exclude: Some("summary"),
    };
    let (out, _err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(out.contains("ID"), "ID retained:\n{out}");
    assert!(!out.contains("SUMMARY"), "SUMMARY excluded:\n{out}");
}

#[test]
fn write_bugs_unknown_field_warns_and_falls_back() {
    let bugs = vec![make_bug(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: Some("not_a_field"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(
        err.contains("not_a_field"),
        "warns about unknown field: {err:?}"
    );
    // All requested columns were unknown -> fall back to the default set.
    assert!(
        out.contains("ID") && out.contains("SUMMARY"),
        "fallback default columns:\n{out}"
    );
}

#[test]
fn write_bugs_json_trims_to_selected_fields() {
    let bugs = vec![make_bug(42, "x", "NEW")];
    let spec = ColumnSpec {
        include: Some("id"),
        exclude: None,
    };
    let (out, _err) = capture_bugs_spec(OutputFormat::Json, &bugs, spec);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    let keys: Vec<&str> = parsed[0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, vec!["id"], "JSON array element trimmed to id:\n{out}");
}

#[test]
fn write_bugs_assignee_alias_still_selects_column() {
    let bugs = vec![make_bug(1, "s", "NEW")];
    let spec = ColumnSpec {
        include: Some("id,assignee"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(out.contains("ASSIGNEE"), "alias resolves column:\n{out}");
    assert!(err.is_empty(), "no warning: {err:?}");
}

#[test]
fn write_bugs_table_renders_requested_custom_column() {
    let bugs = vec![make_bug_with_custom(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: Some("id,cf_release"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);

    assert!(err.is_empty(), "custom fields are known: {err:?}");
    assert!(out.contains("CF_RELEASE"), "custom header rendered:\n{out}");
    assert!(out.contains("9.6"), "custom value rendered:\n{out}");
}

#[test]
fn write_bugs_table_preserves_mixed_custom_order() {
    let bugs = vec![make_bug_with_custom(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: Some("cf_release,id,summary"),
        exclude: None,
    };
    let (out, _err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    let header = out
        .lines()
        .find(|line| line.contains("CF_RELEASE"))
        .unwrap();

    let custom_pos = header.find("CF_RELEASE").unwrap();
    let id_pos = header.find("ID").unwrap();
    let summary_pos = header.find("SUMMARY").unwrap();
    assert!(
        custom_pos < id_pos && id_pos < summary_pos,
        "header order follows --fields:\n{out}"
    );
}

#[test]
fn write_bugs_table_deduplicates_repeated_custom_fields() {
    let bugs = vec![make_bug_with_custom(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: Some("id,cf_release,cf_release,summary"),
        exclude: None,
    };
    let (out, _err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    let header = out
        .lines()
        .find(|line| line.contains("CF_RELEASE"))
        .unwrap();

    assert_eq!(header.matches("CF_RELEASE").count(), 1, "{out}");
}

// ── canonical_field_list ─────────────────────────────────────────

#[test]
fn canonical_field_list_translates_aliases() {
    let got = canonical_field_list(Some("assignee,updated,created,reporter,platform"));
    assert_eq!(
        got.as_deref(),
        Some("assigned_to,last_change_time,creation_time,creator,platform")
    );
}

#[test]
fn canonical_field_list_passes_through_unknown_and_canonical() {
    let got = canonical_field_list(Some("id,cf_custom,summary"));
    assert_eq!(got.as_deref(), Some("id,cf_custom,summary"));
}

#[test]
fn canonical_field_list_handles_empty_and_blanks() {
    assert_eq!(canonical_field_list(None), None);
    assert_eq!(canonical_field_list(Some("")), None);
    assert_eq!(canonical_field_list(Some(",, ,")), None);
}

// ── write_bug_detail ─────────────────────────────────────────────

#[test]
fn write_bug_detail_json_contains_all_fields() {
    let bug = make_bug(42, "Detail test", "ASSIGNED");
    let json = serde_json::to_string_pretty(&bug).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["summary"], "Detail test");
    assert_eq!(parsed["status"], "ASSIGNED");
    assert_eq!(parsed["product"], "TestProduct");
    assert_eq!(parsed["component"], serde_json::json!(["TestComponent"]));
    assert_eq!(parsed["version"], serde_json::json!(["1.0"]));
    assert_eq!(parsed["assigned_to"], "dev@example.com");
    assert_eq!(parsed["priority"], "P1");
    assert_eq!(parsed["severity"], "major");
    assert_eq!(parsed["creator"], "reporter@example.com");
    assert_eq!(parsed["keywords"][0], "regression");
    assert_eq!(parsed["blocks"][0], 200);
    assert_eq!(parsed["depends_on"][0], 100);
}

#[test]
fn write_bug_detail_json_with_resolution() {
    let mut bug = make_bug(42, "Fixed bug", "RESOLVED");
    bug.resolution = Some("FIXED".into());
    let json = serde_json::to_string_pretty(&bug).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["resolution"], "FIXED");
}

#[test]
fn write_bug_detail_table_renders_all_fields() {
    let mut bug = make_bug(42, "Detail test", "ASSIGNED");
    bug.resolution = Some("FIXED".into());
    let output = capture_bug_detail(OutputFormat::Table, &bug);
    assert!(output.contains("Bug"));
    assert!(output.contains("#42"));
    assert!(output.contains("Detail test"));
    assert!(output.contains("Status"));
    assert!(output.contains("Resolution"));
    assert!(output.contains("FIXED"));
    assert!(output.contains("Product"));
    assert!(output.contains("TestProduct"));
    assert!(output.contains("Component"));
    assert!(output.contains("TestComponent"));
    assert!(output.contains("Assignee"));
    assert!(output.contains("dev@example.com"));
    assert!(output.contains("Priority"));
    assert!(output.contains("P1"));
    assert!(output.contains("Severity"));
    assert!(output.contains("major"));
    assert!(output.contains("Creator"));
    assert!(output.contains("reporter@example.com"));
    assert!(output.contains("Keywords"));
    assert!(output.contains("regression"));
    assert!(output.contains("Blocks"));
    assert!(output.contains("200, 201"));
    assert!(output.contains("Depends on"));
    assert!(output.contains("100"));
}

#[test]
fn write_bug_detail_table_shows_flags_and_milestone() {
    let mut bug = make_bug(7, "has flags", "NEW");
    bug.target_milestone = Some("9.0".into());
    bug.flags = vec![
        review_flag("+", None),
        review_flag("?", Some("bob@example.com")),
    ];
    let mut out = Vec::new();

    write_bug_detail(&bug, ColumnSpec::default(), OutputFormat::Table, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains("Target Milestone"), "got: {output}");
    assert!(output.contains("9.0"));
    assert!(output.contains("Flags"));
    assert!(output.contains("review+"), "got: {output}");
    assert!(output.contains("review?(bob@example.com)"), "got: {output}");
}

#[test]
fn write_bug_detail_table_suppresses_unset_milestone_and_empty_flags() {
    let mut bug = make_bug(7, "no milestone", "NEW");
    bug.target_milestone = Some("---".into());
    // flags left empty
    let mut out = Vec::new();

    write_bug_detail(&bug, ColumnSpec::default(), OutputFormat::Table, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(
        !output.contains("Target Milestone"),
        "should suppress ---: {output}"
    );
    assert!(
        !output.contains("Flags"),
        "should suppress empty flags: {output}"
    );
}

#[test]
fn bug_to_json_keeps_flags_when_selected() {
    let mut bug = make_bug(7, "s", "NEW");
    bug.flags = vec![review_flag("+", None)];
    let spec = ColumnSpec::new(Some("id,flags"), None);

    let value = bug_to_json(&bug, spec);
    let map = value.as_object().unwrap();

    assert!(map.contains_key("id"));
    assert!(map.contains_key("flags"));
    assert_eq!(map["flags"][0]["name"], "review");
    // Trimmed to the selection: summary and status are gone.
    assert!(!map.contains_key("summary"));
    assert!(!map.contains_key("status"));
}

#[test]
fn bug_to_json_projects_groups_and_time_tracking_fields() {
    let mut bug = make_bug(7, "s", "NEW");
    bug.groups = vec!["functest-grp".into()];
    bug.estimated_time = Some(8.0);
    bug.remaining_time = Some(5.0);
    let spec = ColumnSpec::new(Some("groups,estimated_time,remaining_time"), None);

    let value = bug_to_json(&bug, spec);

    assert_eq!(value["groups"], serde_json::json!(["functest-grp"]));
    assert_eq!(value["estimated_time"], 8.0);
    assert_eq!(value["remaining_time"], 5.0);
    assert_eq!(value.as_object().unwrap().len(), 3);
}

#[test]
fn write_bug_detail_table_shows_dupe_of() {
    let bug = crate::types::Bug {
        id: 42,
        summary: Some("duplicate source".into()),
        status: Some("RESOLVED".into()),
        resolution: Some("DUPLICATE".into()),
        dupe_of: Some(99),
        deadline: None,
        product: None,
        component: None,
        version: None,
        assigned_to: None,
        priority: None,
        severity: None,
        creation_time: None,
        last_change_time: None,
        creator: None,
        url: None,
        whiteboard: None,
        keywords: vec![],
        blocks: vec![],
        depends_on: vec![],
        cc: vec![],
        op_sys: None,
        platform: None,
        target_milestone: None,
        groups: vec![],
        estimated_time: None,
        remaining_time: None,
        flags: Vec::new(),
        custom_fields: std::collections::BTreeMap::new(),
    };
    let mut out = Vec::new();

    super::write_bug_detail(
        &bug,
        ColumnSpec::default(),
        crate::types::OutputFormat::Table,
        &mut out,
    );

    let output = String::from_utf8(out).unwrap();
    assert!(output.contains("Duplicate of"));
    assert!(output.contains("99"));
}

#[test]
fn write_bug_detail_table_handles_minimal_bug() {
    let bug = Bug {
        id: 1,
        summary: Some("Unicode summary — déjà vu".into()),
        status: Some("NEW".into()),
        resolution: None,
        dupe_of: None,
        deadline: None,
        product: None,
        component: None,
        version: None,
        assigned_to: None,
        priority: None,
        severity: None,
        creation_time: None,
        last_change_time: None,
        creator: None,
        url: None,
        whiteboard: None,
        keywords: vec![],
        blocks: vec![],
        depends_on: vec![],
        cc: vec![],
        op_sys: None,
        platform: None,
        target_milestone: None,
        groups: vec![],
        estimated_time: None,
        remaining_time: None,
        flags: Vec::new(),
        custom_fields: std::collections::BTreeMap::new(),
    };
    let output = capture_bug_detail(OutputFormat::Table, &bug);
    assert!(output.contains("Unicode summary — déjà vu"));
    assert!(output.contains('-'));
    assert!(!output.contains("Keywords"));
    assert!(!output.contains("Blocks"));
    assert!(!output.contains("Depends on"));
}

#[test]
fn write_bug_detail_json_via_write() {
    let bug = make_bug(7, "Json bug", "NEW");
    let output = capture_bug_detail(OutputFormat::Json, &bug);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 7);
    assert_eq!(parsed["summary"], "Json bug");
}

#[test]
fn detail_default_shows_all_present_fields() {
    let bug = make_bug(7, "boom", "ASSIGNED");
    let out = capture_detail_spec(&bug, ColumnSpec::default());
    assert!(out.contains("Status"), "status row present:\n{out}");
    assert!(out.contains("Priority"), "priority row present:\n{out}");
    assert!(out.contains("Product"), "product row present:\n{out}");
}

#[test]
fn detail_include_limits_rows() {
    let bug = make_bug(7, "boom", "ASSIGNED");
    let spec = ColumnSpec {
        include: Some("id,priority"),
        exclude: None,
    };
    let out = capture_detail_spec(&bug, spec);
    assert!(out.contains("Priority"), "priority row present:\n{out}");
    assert!(
        !out.contains("Status"),
        "status row hidden when not requested:\n{out}"
    );
    assert!(!out.contains("Product"), "product row hidden:\n{out}");
}

#[test]
fn detail_exclude_drops_row() {
    let bug = make_bug(7, "boom", "ASSIGNED");
    let spec = ColumnSpec {
        include: None,
        exclude: Some("priority"),
    };
    let out = capture_detail_spec(&bug, spec);
    assert!(out.contains("Status"), "status retained:\n{out}");
    assert!(!out.contains("Priority"), "priority excluded:\n{out}");
}

#[test]
fn detail_include_renders_requested_custom_row() {
    let bug = make_bug_with_custom(7, "boom", "ASSIGNED");
    let spec = ColumnSpec {
        include: Some("status,cf_release"),
        exclude: None,
    };
    let out = capture_detail_spec(&bug, spec);

    assert!(out.contains("Status"), "built-in row retained:\n{out}");
    assert!(
        out.contains("cf_release"),
        "custom row label rendered:\n{out}"
    );
    assert!(out.contains("9.6"), "custom row value rendered:\n{out}");
}

#[test]
fn detail_default_does_not_render_captured_custom_fields() {
    let bug = make_bug_with_custom(7, "boom", "ASSIGNED");
    let out = capture_detail_spec(&bug, ColumnSpec::default());

    assert!(
        !out.contains("cf_release"),
        "custom label hidden by default:\n{out}"
    );
    assert!(
        !out.contains("9.6"),
        "custom value hidden by default:\n{out}"
    );
}

// ── write_json (pretty) ──────────────────────────────────────────

#[test]
fn write_json_produces_valid_json_for_bug() {
    let bug = make_bug(1, "Test bug", "NEW");
    let json = serde_json::to_string_pretty(&bug).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["summary"], "Test bug");
    assert_eq!(parsed["status"], "NEW");
}

#[test]
fn write_json_produces_valid_json_for_vec() {
    let bugs = vec![make_bug(1, "A", "NEW"), make_bug(2, "B", "RESOLVED")];
    let json = serde_json::to_string_pretty(&bugs).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

// ── write_history ────────────────────────────────────────────────

#[test]
fn write_history_json_emits_flattened_records() {
    let records = vec![
        sample_record("status", Some(7)),
        sample_record("priority", None),
    ];
    let output = capture_history_json(&records, OutputFormat::Json);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["who"], "editor@example.com");
    assert_eq!(arr[0]["when"], "2025-04-01T12:00:00Z");
    assert_eq!(arr[0]["field"], "status");
    assert_eq!(arr[0]["old_value"], "NEW");
    assert_eq!(arr[0]["new_value"], "ASSIGNED");
    assert_eq!(arr[0]["comment_id"], 7);
    // Null comment_id is present (not omitted) so the shape is stable.
    assert!(arr[1]["comment_id"].is_null());
    // The grouped wire shape must NOT leak into the flattened output.
    assert!(arr[0].get("changes").is_none());
    assert!(arr[0].get("field_name").is_none());
}

#[test]
fn write_history_json_preserves_unknown_delta_values() {
    let record = HistoryRecord {
        when: "2025-04-01T12:00:00Z".into(),
        who: "editor@example.com".into(),
        field: "status".into(),
        old_value: None,
        new_value: None,
        comment_id: None,
    };
    let output = capture_history_json(&[record], OutputFormat::Json);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);

    assert!(parsed[0]["old_value"].is_null());
    assert!(parsed[0]["new_value"].is_null());
}

#[test]
fn write_history_json_empty_is_empty_array() {
    let output = capture_history_json(&[], OutputFormat::Json);
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed, serde_json::json!([]));
}

#[test]
fn write_history_ndjson_one_record_per_line() {
    let records = vec![
        sample_record("status", Some(7)),
        sample_record("priority", None),
    ];
    let output = capture_history_json(&records, OutputFormat::Ndjson);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    for line in lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.is_object());
        assert!(v.get("field").is_some());
    }
}

#[test]
fn write_history_table_renders_changes() {
    let history = vec![make_history_entry()];
    let output = capture_history_table(&history);
    assert!(output.contains("Change"));
    assert!(output.contains("editor@example.com"));
    assert!(output.contains("2025-04-01T12:00:00Z"));
    assert!(output.contains("status"));
    assert!(output.contains("NEW"));
    assert!(output.contains("ASSIGNED"));
    assert!(output.contains("flagtypes.name"));
    assert!(output.contains("[attachment #99]"));
    assert!(output.contains("review?"));
    assert!(output.contains('─'));
}

#[test]
fn write_history_table_empty_renders_nothing() {
    let output = capture_history_table(&[]);
    assert!(!output.contains("Change"));
    assert!(!output.contains('─'));
}

// ── multi_bug_view ───────────────────────────────────────────────

fn no_color() {
    colored::control::set_override(false);
}

fn sample_bug(id: u64, summary: &str) -> Bug {
    Bug {
        id,
        summary: Some(summary.into()),
        status: Some("NEW".into()),
        resolution: None,
        dupe_of: None,
        deadline: None,
        product: None,
        component: None,
        version: None,
        assigned_to: None,
        priority: None,
        severity: None,
        creation_time: None,
        last_change_time: None,
        creator: None,
        url: None,
        whiteboard: None,
        keywords: vec![],
        blocks: vec![],
        depends_on: vec![],
        cc: vec![],
        op_sys: None,
        platform: None,
        target_milestone: None,
        groups: vec![],
        estimated_time: None,
        remaining_time: None,
        flags: Vec::new(),
        custom_fields: std::collections::BTreeMap::new(),
    }
}

#[test]
fn multi_bug_view_renders_success_blocks_with_dividers() {
    no_color();
    let rows = vec![
        MultiBugRow::Ok(Box::new(sample_bug(1, "first"))),
        MultiBugRow::Ok(Box::new(sample_bug(2, "second"))),
        MultiBugRow::Ok(Box::new(sample_bug(3, "third"))),
    ];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, ColumnSpec::default(), &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Bug #1"));
    assert!(out.contains("Bug #2"));
    assert!(out.contains("Bug #3"));
    let divider = "─".repeat(60);
    assert_eq!(out.matches(&divider).count(), 2);
}

#[test]
fn multi_bug_view_renders_failure_block_with_unavailable_marker() {
    no_color();
    let rows = vec![MultiBugRow::Failed {
        id: "999".into(),
        error: "bug not found: 999".into(),
    }];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, ColumnSpec::default(), &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Bug #999"));
    assert!(out.contains("UNAVAILABLE"));
    assert!(out.contains("Error: bug not found: 999"));
}

#[test]
fn multi_bug_view_single_row_emits_no_divider() {
    no_color();
    let rows = vec![MultiBugRow::Ok(Box::new(sample_bug(7, "only")))];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, ColumnSpec::default(), &mut buf);
    let out = String::from_utf8(buf).unwrap();
    let divider = "─".repeat(60);
    assert_eq!(out.matches(&divider).count(), 0);
}

#[test]
fn multi_bug_view_interleaves_success_and_failure_in_order() {
    no_color();
    let rows = vec![
        MultiBugRow::Ok(Box::new(sample_bug(10, "alpha"))),
        MultiBugRow::Failed {
            id: "11".into(),
            error: "denied".into(),
        },
        MultiBugRow::Ok(Box::new(sample_bug(12, "gamma"))),
    ];
    let mut buf = Vec::new();
    write_multi_bug_view(&rows, ColumnSpec::default(), &mut buf);
    let out = String::from_utf8(buf).unwrap();
    let pos_alpha = out.find("Bug #10").unwrap();
    let pos_unavail = out.find("Bug #11").unwrap();
    let pos_gamma = out.find("Bug #12").unwrap();
    assert!(pos_alpha < pos_unavail && pos_unavail < pos_gamma);
}

// ── validate_table_columns (F2/F3) ───────────────────────────────

#[test]
fn validate_table_columns_ok_for_default_spec() {
    assert!(validate_table_columns(ColumnSpec::default()).is_ok());
}

#[test]
fn validate_table_columns_ok_for_partial_unknown_include() {
    let spec = ColumnSpec {
        include: Some("id,not_a_field"),
        exclude: None,
    };
    assert!(validate_table_columns(spec).is_ok());
}

#[test]
fn validate_table_columns_ok_for_all_custom_include() {
    let spec = ColumnSpec {
        include: Some("cf_custom"),
        exclude: None,
    };
    assert!(validate_table_columns(spec).is_ok());
}

#[test]
fn validate_table_columns_errors_for_all_unknown_include() {
    let spec = ColumnSpec {
        include: Some("not_a_field"),
        exclude: None,
    };
    let err = validate_table_columns(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        err.to_string().contains("not_a_field"),
        "names the offending field: {err}"
    );
}

#[test]
fn validate_table_columns_errors_when_exclude_removes_all_defaults() {
    let spec = ColumnSpec {
        include: None,
        exclude: Some("id,status,priority,assignee,summary"),
    };
    let err = validate_table_columns(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn validate_table_columns_errors_when_exclude_removes_sole_include() {
    let spec = ColumnSpec {
        include: Some("id"),
        exclude: Some("id"),
    };
    let err = validate_table_columns(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn validate_table_columns_errors_when_exclude_removes_sole_custom_include() {
    let spec = ColumnSpec {
        include: Some("cf_release"),
        exclude: Some("cf_release"),
    };
    let err = validate_table_columns(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn validate_table_columns_ok_for_all_blank_include() {
    let spec = ColumnSpec {
        include: Some(",,"),
        exclude: None,
    };
    assert!(validate_table_columns(spec).is_ok());
}

// ── bug_to_json / bugs_to_json projection (#206) ─────────────────

/// The serde key sequence of `Bug`, in struct-declaration order. Locks the
/// `preserve_order` decision (Finding 4) and is the reference for the registry
/// drift guard (Finding 3).
const BUG_STRUCT_KEY_ORDER: [&str; 27] = [
    "id",
    "summary",
    "status",
    "resolution",
    "dupe_of",
    "deadline",
    "product",
    "component",
    "version",
    "assigned_to",
    "priority",
    "severity",
    "creation_time",
    "last_change_time",
    "creator",
    "url",
    "whiteboard",
    "keywords",
    "blocks",
    "depends_on",
    "cc",
    "op_sys",
    "platform",
    "rep_platform",
    "target_milestone",
    "groups",
    "flags",
];

fn keys_of(value: &serde_json::Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

#[test]
fn bug_to_json_include_keeps_only_named_keys() {
    let bug = make_bug(1, "s", "NEW");
    let spec = ColumnSpec {
        include: Some("summary,status"),
        exclude: None,
    };
    assert_eq!(keys_of(&bug_to_json(&bug, spec)), vec!["summary", "status"]);
}

#[test]
fn bug_to_json_include_alias_resolves_to_canonical_key() {
    let bug = make_bug(1, "s", "NEW");
    let spec = ColumnSpec {
        include: Some("assignee"),
        exclude: None,
    };
    let v = bug_to_json(&bug, spec);
    assert_eq!(keys_of(&v), vec!["assigned_to"]);
    assert_eq!(v["assigned_to"], "dev@example.com");
}

#[test]
fn bug_to_json_exclude_id_drops_id() {
    let bug = make_bug(1, "s", "NEW");
    let spec = ColumnSpec {
        include: None,
        exclude: Some("id"),
    };
    let v = bug_to_json(&bug, spec);
    let map = v.as_object().unwrap();
    assert!(!map.contains_key("id"), "id dropped");
    assert!(map.contains_key("summary"), "other keys retained");
    assert_eq!(map.len(), BUG_STRUCT_KEY_ORDER.len() - 1);
}

#[test]
fn bug_to_json_exclude_subset_drops_only_those() {
    let bug = make_bug(1, "s", "NEW");
    let spec = ColumnSpec {
        include: None,
        exclude: Some("cc,keywords"),
    };
    let v = bug_to_json(&bug, spec);
    let map = v.as_object().unwrap();
    assert!(!map.contains_key("cc"));
    assert!(!map.contains_key("keywords"));
    assert!(map.contains_key("id"));
    assert_eq!(map.len(), BUG_STRUCT_KEY_ORDER.len() - 2);
}

#[test]
fn bug_to_json_no_selection_is_full_object() {
    let bug = make_bug(1, "s", "NEW");
    for spec in [
        ColumnSpec::default(),
        ColumnSpec {
            include: Some(""),
            exclude: None,
        },
        ColumnSpec {
            include: Some(",,"),
            exclude: None,
        },
    ] {
        let v = bug_to_json(&bug, spec);
        assert_eq!(
            v.as_object().unwrap().len(),
            BUG_STRUCT_KEY_ORDER.len(),
            "full object for {spec:?}"
        );
    }
}

#[test]
fn bug_to_json_partial_unknown_keeps_known_only() {
    let bug = make_bug(1, "s", "NEW");
    let spec = ColumnSpec {
        include: Some("summary,not_a_field"),
        exclude: None,
    };
    let v = bug_to_json(&bug, spec);
    let map = v.as_object().unwrap();
    assert!(map.contains_key("summary"));
    assert!(!map.contains_key("not_a_field"));
    assert_eq!(map.len(), 1);
}

#[test]
fn bug_to_json_include_keeps_selected_custom_field() {
    let bug = make_bug_with_custom(1, "s", "NEW");
    let spec = ColumnSpec {
        include: Some("cf_release"),
        exclude: None,
    };
    let v = bug_to_json(&bug, spec);

    assert_eq!(keys_of(&v), vec!["cf_release"]);
    assert_eq!(v["cf_release"], "9.6");
}

#[test]
fn bug_to_json_include_keeps_builtin_and_custom_fields() {
    let bug = make_bug_with_custom(1, "s", "NEW");
    let spec = ColumnSpec {
        include: Some("summary,cf_release"),
        exclude: None,
    };
    let v = bug_to_json(&bug, spec);

    assert_eq!(keys_of(&v), vec!["summary", "cf_release"]);
    assert_eq!(v["summary"], "s");
    assert_eq!(v["cf_release"], "9.6");
}

#[test]
fn bug_to_json_exclude_drops_selected_custom_field() {
    let bug = make_bug_with_custom(1, "s", "NEW");
    let spec = ColumnSpec {
        include: None,
        exclude: Some("cf_release"),
    };
    let v = bug_to_json(&bug, spec);
    let map = v.as_object().unwrap();

    assert!(!map.contains_key("cf_release"));
    assert!(map.contains_key("cf_score"));
}

#[test]
fn bug_to_json_full_object_preserves_struct_field_order() {
    let bug = make_bug(1, "s", "NEW");
    let v = bug_to_json(&bug, ColumnSpec::default());
    assert_eq!(keys_of(&v), BUG_STRUCT_KEY_ORDER.to_vec());
}

#[test]
fn bug_to_json_projection_preserves_struct_order_not_request_order() {
    // Include given out of struct order; the projected object must still be in
    // struct-declaration order (Finding 4: real ordering lock, not vacuous).
    let bug = make_bug(1, "s", "NEW");
    let spec = ColumnSpec {
        include: Some("status,id,summary"),
        exclude: None,
    };
    assert_eq!(
        keys_of(&bug_to_json(&bug, spec)),
        vec!["id", "summary", "status"]
    );
}

#[test]
fn bug_to_json_full_object_places_custom_fields_after_built_ins() {
    let bug = make_bug_with_custom(1, "s", "NEW");
    let keys = keys_of(&bug_to_json(&bug, ColumnSpec::default()));

    assert_eq!(&keys[..BUG_STRUCT_KEY_ORDER.len()], BUG_STRUCT_KEY_ORDER);
    assert_eq!(
        &keys[BUG_STRUCT_KEY_ORDER.len()..],
        ["cf_release", "cf_score"]
    );
}

#[test]
fn bugs_to_json_projects_every_element() {
    let bugs = vec![make_bug(1, "a", "NEW"), make_bug(2, "b", "RESOLVED")];
    let spec = ColumnSpec {
        include: Some("id"),
        exclude: None,
    };
    let arr = bugs_to_json(&bugs, spec);
    assert_eq!(arr.len(), 2);
    for v in &arr {
        assert_eq!(keys_of(v), vec!["id"]);
    }
    assert_eq!(arr[0]["id"], 1);
    assert_eq!(arr[1]["id"], 2);
}

// ── Registry drift guard (Finding 3) ─────────────────────────────

#[test]
fn columns_registry_is_one_to_one_with_bug_serde_keys() {
    let mut bug = make_bug(1, "s", "NEW");
    bug.estimated_time = Some(8.0);
    bug.remaining_time = Some(5.0);
    let value = serde_json::to_value(&bug).unwrap();
    let mut serde_keys: std::collections::HashSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    assert!(
        serde_keys.remove("rep_platform"),
        "the published compatibility alias must remain during the 2.1.x transition"
    );
    let registry_keys: std::collections::HashSet<String> = BUG_FIELDS
        .iter()
        .map(|field| field.canonical().to_string())
        .collect();
    assert_eq!(
        serde_keys, registry_keys,
        "BUG_FIELDS canonical names must be 1:1 with Bug's non-alias serde keys"
    );
    assert_eq!(
        registry_keys.len(),
        BUG_FIELDS.len(),
        "no duplicate canonical names in BUG_FIELDS"
    );
}

// ── validate_json_field_selection (Finding 1) ────────────────────

#[test]
fn validate_json_default_spec_ok() {
    assert!(validate_json_field_selection(ColumnSpec::default()).is_ok());
}

#[test]
fn validate_json_all_unknown_include_errs() {
    let spec = ColumnSpec {
        include: Some("not_a_field,also_not_a_field"),
        exclude: None,
    };
    let err = validate_json_field_selection(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn validate_json_all_custom_include_ok() {
    let spec = ColumnSpec {
        include: Some("cf_x,cf_y"),
        exclude: None,
    };
    assert!(validate_json_field_selection(spec).is_ok());
}

#[test]
fn validate_json_exclude_every_key_errs() {
    let all = BUG_FIELDS
        .iter()
        .map(|field| field.canonical())
        .collect::<Vec<_>>()
        .join(",");
    let spec = ColumnSpec {
        include: None,
        exclude: Some(all.as_str()),
    };
    let err = validate_json_field_selection(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn validate_json_errors_when_exclude_removes_sole_custom_include() {
    let spec = ColumnSpec {
        include: Some("cf_release"),
        exclude: Some("cf_release"),
    };
    let err = validate_json_field_selection(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn validate_json_exclude_table_defaults_ok() {
    // Excluding the five default *table* columns must NOT exit 7 under --json:
    // 18 other fields remain. This is the regression the table validator would
    // get wrong if reused verbatim (Finding 1).
    let spec = ColumnSpec {
        include: None,
        exclude: Some("id,status,priority,assignee,summary"),
    };
    assert!(validate_json_field_selection(spec).is_ok());
}

#[test]
fn validate_json_partial_unknown_include_ok() {
    let spec = ColumnSpec {
        include: Some("summary,not_a_field"),
        exclude: None,
    };
    assert!(validate_json_field_selection(spec).is_ok());
}

#[test]
fn validate_json_blank_include_ok() {
    for blank in ["", ",,"] {
        let spec = ColumnSpec {
            include: Some(blank),
            exclude: None,
        };
        assert!(
            validate_json_field_selection(spec).is_ok(),
            "blank include {blank:?} is no selection"
        );
    }
}

// ── warn_unknown_fields ──────────────────────────────────────────

fn capture_unknown_warning(spec: ColumnSpec<'_>) -> String {
    let mut err = Vec::new();
    warn_unknown_fields(spec, &mut err);
    String::from_utf8(err).unwrap()
}

#[test]
fn warn_unknown_fields_warns_for_unknown_include_token() {
    let w = capture_unknown_warning(ColumnSpec {
        include: Some("summary,not_a_field"),
        exclude: None,
    });
    assert!(
        w.contains("ignoring unknown field(s): not_a_field"),
        "{w:?}"
    );
}

#[test]
fn warn_unknown_fields_silent_for_all_known() {
    let w = capture_unknown_warning(ColumnSpec {
        include: Some("summary,status,cf_x"),
        exclude: None,
    });
    assert!(w.is_empty(), "no warning when all known: {w:?}");
}

#[test]
fn warn_unknown_fields_silent_without_include() {
    let w = capture_unknown_warning(ColumnSpec {
        include: None,
        exclude: Some("cf_x"),
    });
    assert!(w.is_empty(), "exclude-only never warns: {w:?}");
}

#[test]
fn write_bugs_partial_unknown_warns_with_new_wording_and_shows_known() {
    let bugs = vec![make_bug(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: Some("id,not_a_field"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(
        err.contains("ignoring unknown field(s): not_a_field"),
        "new warning wording: {err:?}"
    );
    assert!(out.contains("ID"), "known column shown:\n{out}");
}

fn sample_links() -> Vec<BugLink> {
    vec![BugLink {
        id: 2,
        relation: LinkRelation::DependsOn,
        direction: LinkRelation::DependsOn.direction(),
        depth: 1,
        summary: Some("dep".into()),
        status: Some("NEW".into()),
    }]
}

#[test]
fn write_bug_links_ndjson_is_one_object_per_line() {
    let mut buf = Vec::new();
    write_bug_links(&sample_links(), OutputFormat::Ndjson, &mut buf);
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s.lines().count(), 1);
    assert!(s.contains(r#""relation":"depends_on""#) && s.contains(r#""direction":"out""#));
}

#[test]
fn write_bug_links_json_is_array() {
    let mut buf = Vec::new();
    write_bug_links(&sample_links(), OutputFormat::Json, &mut buf);
    let v: serde_json::Value =
        crate::test_helpers::json_envelope_data(std::str::from_utf8(&buf).unwrap());
    assert!(v.is_array());
    assert_eq!(v[0]["depth"], 1);
}

#[test]
fn write_bug_links_table_has_headers_and_row() {
    let mut buf = Vec::new();
    write_bug_links(&sample_links(), OutputFormat::Table, &mut buf);
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("RELATION") && s.contains("DEPTH"));
    assert!(s.contains("depends_on") && s.contains('2'));
}
