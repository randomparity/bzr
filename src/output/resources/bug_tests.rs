#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::{Bug, FieldChange, HistoryEntry};

fn make_bug(id: u64, summary: &str, status: &str) -> Bug {
    Bug {
        id,
        summary: summary.into(),
        status: status.into(),
        resolution: None,
        dupe_of: None,
        deadline: None,
        product: Some("TestProduct".into()),
        component: Some("TestComponent".into()),
        version: Some("1.0".into()),
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
        rep_platform: None,
    }
}

fn make_history_entry() -> HistoryEntry {
    HistoryEntry {
        who: "editor@example.com".into(),
        when: "2025-04-01T12:00:00Z".into(),
        changes: vec![
            FieldChange {
                field_name: "status".into(),
                removed: "NEW".into(),
                added: "ASSIGNED".into(),
                attachment_id: None,
            },
            FieldChange {
                field_name: "flagtypes.name".into(),
                removed: String::new(),
                added: "review?".into(),
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

fn capture_history(format: OutputFormat, history: &[HistoryEntry]) -> String {
    let mut buf = Vec::new();
    write_history(history, format, &mut buf);
    String::from_utf8(buf).unwrap()
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
fn write_bugs_json_empty_renders_empty_array() {
    let output = capture_bugs(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
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
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
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
        include: Some("cf_custom_thing"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(
        err.contains("cf_custom_thing"),
        "warns about unknown field: {err:?}"
    );
    // All requested columns were unknown -> fall back to the default set.
    assert!(
        out.contains("ID") && out.contains("SUMMARY"),
        "fallback default columns:\n{out}"
    );
}

#[test]
fn write_bugs_json_ignores_columns() {
    let bugs = vec![make_bug(42, "x", "NEW")];
    let spec = ColumnSpec {
        include: Some("id"),
        exclude: None,
    };
    let (out, _err) = capture_bugs_spec(OutputFormat::Json, &bugs, spec);
    // JSON path is unchanged: full serialized struct, not column-filtered.
    assert!(
        out.contains("\"summary\""),
        "JSON still serializes summary:\n{out}"
    );
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

// ── canonical_field_list ─────────────────────────────────────────

#[test]
fn canonical_field_list_translates_aliases() {
    let got = canonical_field_list(Some("assignee,updated,created,reporter,platform"));
    assert_eq!(
        got.as_deref(),
        Some("assigned_to,last_change_time,creation_time,creator,rep_platform")
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
    assert_eq!(parsed["component"], "TestComponent");
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
fn write_bug_detail_table_shows_dupe_of() {
    let bug = crate::types::Bug {
        id: 42,
        summary: "duplicate source".into(),
        status: "RESOLVED".into(),
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
        rep_platform: None,
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
        summary: "Unicode summary — déjà vu".into(),
        status: "NEW".into(),
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
        rep_platform: None,
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
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
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
fn write_history_json_one_entry() {
    let history = vec![make_history_entry()];
    let json = serde_json::to_string_pretty(&history).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["who"], "editor@example.com");
    assert_eq!(parsed[0]["when"], "2025-04-01T12:00:00Z");
    let changes = parsed[0]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["field_name"], "status");
    assert_eq!(changes[0]["removed"], "NEW");
    assert_eq!(changes[0]["added"], "ASSIGNED");
    assert_eq!(changes[1]["attachment_id"], 99);
}

#[test]
fn write_history_json_empty() {
    let history: Vec<HistoryEntry> = vec![];
    let json = serde_json::to_string_pretty(&history).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_history_table_renders_changes() {
    let history = vec![make_history_entry()];
    let output = capture_history(OutputFormat::Table, &history);
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
    let output = capture_history(OutputFormat::Table, &[]);
    assert!(!output.contains("Change"));
    assert!(!output.contains('─'));
}

#[test]
fn write_history_json_via_write() {
    let history = vec![make_history_entry()];
    let output = capture_history(OutputFormat::Json, &history);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["who"], "editor@example.com");
}

// ── multi_bug_view ───────────────────────────────────────────────

fn no_color() {
    colored::control::set_override(false);
}

fn sample_bug(id: u64, summary: &str) -> Bug {
    Bug {
        id,
        summary: summary.into(),
        status: "NEW".into(),
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
        rep_platform: None,
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
        include: Some("id,cf_custom"),
        exclude: None,
    };
    assert!(validate_table_columns(spec).is_ok());
}

#[test]
fn validate_table_columns_errors_for_all_unknown_include() {
    let spec = ColumnSpec {
        include: Some("cf_custom"),
        exclude: None,
    };
    let err = validate_table_columns(spec).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        err.to_string().contains("cf_custom"),
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
fn validate_table_columns_ok_for_all_blank_include() {
    let spec = ColumnSpec {
        include: Some(",,"),
        exclude: None,
    };
    assert!(validate_table_columns(spec).is_ok());
}

// ── warn_json_field_selection (F1) ───────────────────────────────

fn capture_json_warning(spec: ColumnSpec<'_>) -> String {
    let mut err = Vec::new();
    warn_json_field_selection(spec, true, &mut err);
    String::from_utf8(err).unwrap()
}

#[test]
fn warn_json_field_selection_warns_for_include() {
    let warning = capture_json_warning(ColumnSpec {
        include: Some("summary"),
        exclude: None,
    });
    assert!(
        warning.contains("null/empty"),
        "warns about value-blanking: {warning:?}"
    );
    assert!(
        warning.contains("id is always present"),
        "discloses the id exception: {warning:?}"
    );
}

#[test]
fn warn_json_field_selection_silent_when_not_interactive() {
    let mut err = Vec::new();
    warn_json_field_selection(
        ColumnSpec {
            include: Some("summary"),
            exclude: None,
        },
        false,
        &mut err,
    );
    let warning = String::from_utf8(err).unwrap();
    assert!(
        warning.is_empty(),
        "no warning when stderr is not a terminal: {warning:?}"
    );
}

#[test]
fn warn_json_field_selection_silent_for_blank_selection() {
    for blank in ["", ",,"] {
        let warning = capture_json_warning(ColumnSpec {
            include: Some(blank),
            exclude: None,
        });
        assert!(
            warning.is_empty(),
            "blank include {blank:?} is no selection: {warning:?}"
        );
    }
}

#[test]
fn warn_json_field_selection_warns_for_exclude() {
    let warning = capture_json_warning(ColumnSpec {
        include: None,
        exclude: Some("status"),
    });
    assert!(
        warning.contains("null/empty"),
        "warns about value-blanking: {warning:?}"
    );
}

#[test]
fn warn_json_field_selection_silent_without_selection() {
    let warning = capture_json_warning(ColumnSpec::default());
    assert!(
        warning.is_empty(),
        "no warning when no field selection is active: {warning:?}"
    );
}

#[test]
fn warn_json_field_selection_silent_for_exclude_id_only() {
    for excl in ["id", "ID", "id,"] {
        let warning = capture_json_warning(ColumnSpec {
            include: None,
            exclude: Some(excl),
        });
        assert!(
            warning.is_empty(),
            "excluding only id restricts nothing (client re-adds id): {excl:?} -> {warning:?}"
        );
    }
}

#[test]
fn warn_json_field_selection_warns_for_exclude_non_id() {
    let warning = capture_json_warning(ColumnSpec {
        include: None,
        exclude: Some("id,cc"),
    });
    assert!(
        warning.contains("null/empty"),
        "a non-id field is genuinely dropped, so warn: {warning:?}"
    );
}

#[test]
fn write_bugs_partial_unknown_warns_with_new_wording_and_shows_known() {
    let bugs = vec![make_bug(1, "summary text", "NEW")];
    let spec = ColumnSpec {
        include: Some("id,cf_x"),
        exclude: None,
    };
    let (out, err) = capture_bugs_spec(OutputFormat::Table, &bugs, spec);
    assert!(
        err.contains("ignoring field(s) with no table column: cf_x"),
        "new warning wording: {err:?}"
    );
    assert!(out.contains("ID"), "known column shown:\n{out}");
}
