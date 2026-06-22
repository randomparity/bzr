#![expect(clippy::unwrap_used)]

use std::collections::HashMap;

use super::*;
use crate::types::{BugTemplate, OutputFormat};

fn make_template() -> BugTemplate {
    BugTemplate {
        product: Some("Widget".into()),
        component: Some("Backend".into()),
        version: None,
        priority: Some("P1".into()),
        severity: Some("major".into()),
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: Some("Default description".into()),
        url: Some("https://example.com/repro".into()),
        whiteboard: Some("needs-triage".into()),
        target_milestone: Some("M1".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["cc@example.com".into()],
        keywords: vec!["regression".into()],
        groups: vec!["security".into()],
        flags: vec!["review?".into()],
    }
}

#[test]
fn template_saved_json() {
    let json = serde_json::json!({"name": "my-tmpl", "action": "saved"});
    let parsed: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&json).unwrap()).unwrap();
    assert_eq!(parsed["name"], "my-tmpl");
    assert_eq!(parsed["action"], "saved");
}

#[test]
fn template_list_json_serializes() {
    let mut templates: HashMap<String, BugTemplate> = HashMap::new();
    templates.insert("default".into(), make_template());
    let json = serde_json::to_string_pretty(&templates).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["default"].is_object());
    assert_eq!(parsed["default"]["product"], "Widget");
    assert_eq!(parsed["default"]["component"], "Backend");
    assert_eq!(
        parsed["default"]["cc"],
        serde_json::json!(["cc@example.com"])
    );
    assert_eq!(parsed["default"]["flags"], serde_json::json!(["review?"]));
}

#[test]
fn template_detail_json_with_flatten() {
    #[derive(serde::Serialize)]
    struct TemplateView<'a> {
        name: &'a str,
        #[serde(flatten)]
        template: &'a BugTemplate,
    }
    let template = make_template();
    let view = TemplateView {
        name: "test-tmpl",
        template: &template,
    };
    let json = serde_json::to_string_pretty(&view).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["name"], "test-tmpl");
    assert_eq!(parsed["product"], "Widget");
    assert_eq!(parsed["priority"], "P1");
    assert_eq!(parsed["target_milestone"], "M1");
    assert!(parsed["version"].is_null());
}

#[test]
fn template_empty_fields_omitted_in_json() {
    let template = BugTemplate {
        product: None,
        component: None,
        version: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: None,
        url: None,
        whiteboard: None,
        target_milestone: None,
        deadline: None,
        cc: vec![],
        keywords: vec![],
        groups: vec![],
        flags: vec![],
    };
    let json = serde_json::to_string(&template).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.as_object().unwrap().is_empty());
}

#[test]
fn template_saved_message_renders_table_text() {
    assert_eq!(
        template_saved_message("default", "Saved"),
        "Saved template 'default'"
    );
}

#[test]
fn template_summary_line_renders_all_present_fields() {
    let line = template_summary_line("aaa", &make_template());
    assert_eq!(
        line,
        "aaa (product=Widget, component=Backend, priority=P1, severity=major)"
    );
}

#[test]
fn template_summary_line_without_fields_is_name_only() {
    let line = template_summary_line(
        "zzz",
        &BugTemplate {
            product: None,
            component: None,
            version: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            description: None,
            url: None,
            whiteboard: None,
            target_milestone: None,
            deadline: None,
            cc: vec![],
            keywords: vec![],
            groups: vec![],
            flags: vec![],
        },
    );
    assert_eq!(line, "zzz");
}

fn capture_detail(name: &str, template: &BugTemplate, format: OutputFormat) -> String {
    let mut buf = Vec::new();
    write_template_detail(name, template, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_list(templates: &HashMap<String, BugTemplate>, format: OutputFormat) -> String {
    let mut buf = Vec::new();
    write_template_list(templates, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

#[test]
fn write_template_detail_table_renders_missing_fields_as_dash() {
    let template = BugTemplate {
        product: Some("Widget".into()),
        component: None,
        version: None,
        priority: Some("P1".into()),
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: Some("Default description".into()),
        url: Some("https://example.com/repro".into()),
        whiteboard: Some("needs-triage".into()),
        target_milestone: Some("M1".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["cc@example.com".into()],
        keywords: vec!["regression".into()],
        groups: vec!["security".into()],
        flags: vec!["review?".into()],
    };

    let output = capture_detail("default", &template, OutputFormat::Table);

    assert!(output.contains("Name"));
    assert!(output.contains("default"));
    assert!(output.contains("Product"));
    assert!(output.contains("Widget"));
    assert!(output.contains("Component"));
    assert!(output.contains("  -"));
    assert!(output.contains("Description"));
    assert!(output.contains("Default description"));
    assert!(output.contains("URL"));
    assert!(output.contains("https://example.com/repro"));
    assert!(output.contains("Target Milestone"));
    assert!(output.contains("M1"));
    assert!(output.contains("CC"));
    assert!(output.contains("cc@example.com"));
    assert!(output.contains("Flags"));
    assert!(output.contains("review?"));
}

#[test]
fn write_template_list_table_renders_sorted_summaries() {
    let mut templates: HashMap<String, BugTemplate> = HashMap::new();
    templates.insert("zzz".into(), make_template());
    templates.insert(
        "aaa".into(),
        BugTemplate {
            product: Some("Alpha".into()),
            component: None,
            version: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            description: None,
            url: None,
            whiteboard: None,
            target_milestone: None,
            deadline: None,
            cc: vec![],
            keywords: vec![],
            groups: vec![],
            flags: vec![],
        },
    );

    let output = capture_list(&templates, OutputFormat::Table);

    let aaa_pos = output.find("aaa").expect("aaa should appear in output");
    let zzz_pos = output.find("zzz").expect("zzz should appear in output");
    assert!(aaa_pos < zzz_pos, "templates should be sorted by name");
    assert!(output.contains("product=Alpha"));
    assert!(output.contains("product=Widget"));
}

#[test]
fn write_template_list_table_announces_empty() {
    let templates: HashMap<String, BugTemplate> = HashMap::new();
    let output = capture_list(&templates, OutputFormat::Table);
    assert!(output.contains("No templates configured."));
}
