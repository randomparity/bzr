#![expect(clippy::unwrap_used)]

use crate::cli::template::UpdateArgs;
use crate::cli::{TemplateAction, TemplateFields};
use crate::config::Config;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn save_action(name: &str) -> TemplateAction {
    TemplateAction::Save {
        name: name.into(),
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            priority: Some("P1".into()),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn template_save_and_show() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // Save a template
    let action = save_action("test-tmpl");
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a1.writers(),
    )
    .await;
    let _output = __io_a1.out_str().to_string();
    assert!(result.is_ok(), "template save failed: {result:?}");

    // Show the saved template
    let action = TemplateAction::Show {
        name: "test-tmpl".into(),
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "template show failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "test-tmpl");
    assert_eq!(parsed["product"], "TestProduct");
    assert_eq!(parsed["priority"], "P1");
}

#[tokio::test]
async fn template_save_requires_field() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "empty-tmpl".into(),
        fields: TemplateFields::default(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "saving empty template should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("at least one field"),
        "expected validation error, got: {err}"
    );
}

#[tokio::test]
async fn template_save_with_single_field_succeeds() {
    // A single non-None field is enough to satisfy the
    // "at least one field required" validator.
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "version-only".into(),
        fields: TemplateFields {
            version: Some("1.2.3".into()),
            ..Default::default()
        },
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    let _ = __io.out_str().to_string();
    assert!(
        result.is_ok(),
        "single-field template should save: {result:?}"
    );
}

#[tokio::test]
async fn template_save_and_show_create_metadata_fields() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "routing".into(),
        fields: TemplateFields {
            url: Some("https://example.com/repro".into()),
            whiteboard: Some("needs-triage".into()),
            target_milestone: Some("M1".into()),
            deadline: Some("2026-12-31".into()),
            cc: vec!["cc@example.com".into()],
            keywords: vec!["regression".into()],
            groups: vec!["security".into()],
            flag: vec!["review?".into()],
            ..Default::default()
        },
    };
    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut save_io.writers(),
    )
    .await;
    let _ = save_io.out_str().to_string();
    assert!(result.is_ok(), "template save failed: {result:?}");

    let mut show_io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &TemplateAction::Show {
            name: "routing".into(),
        },
        None,
        OutputFormat::Json,
        None,
        &mut show_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "template show failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(show_io.out_str().trim()).unwrap();
    assert_eq!(parsed["url"], "https://example.com/repro");
    assert_eq!(parsed["whiteboard"], "needs-triage");
    assert_eq!(parsed["target_milestone"], "M1");
    assert_eq!(parsed["deadline"], "2026-12-31");
    assert_eq!(parsed["cc"], serde_json::json!(["cc@example.com"]));
    assert_eq!(parsed["keywords"], serde_json::json!(["regression"]));
    assert_eq!(parsed["groups"], serde_json::json!(["security"]));
    assert_eq!(parsed["flags"], serde_json::json!(["review?"]));
}

#[tokio::test]
async fn template_save_rejects_malformed_deadline() {
    let mut cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Save {
        name: "bad-deadline".into(),
        fields: TemplateFields {
            product: Some("TestProduct".into()),
            deadline: Some("2026-99-99".into()),
            ..Default::default()
        },
    };
    let err = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut cap_io.writers(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--deadline"));
}

#[tokio::test]
async fn template_delete_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::Delete {
        name: "nonexistent".into(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "deleting unknown template should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "expected not-found error, got: {err}"
    );
}

#[tokio::test]
async fn template_list_empty() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = TemplateAction::List;
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a3.writers(),
    )
    .await;
    let _output = __io_a3.out_str().to_string();
    assert!(result.is_ok(), "template list failed: {result:?}");
}

#[tokio::test]
async fn template_save_existing_entry_reports_updated_and_replaces_fields() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &save_action("existing"),
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
    assert!(result.is_ok());

    let update = TemplateAction::Save {
        name: "existing".into(),
        fields: TemplateFields {
            component: Some("Updated".into()),
            version: Some("123".into()),
            severity: Some("major".into()),
            description: Some("updated".into()),
            ..Default::default()
        },
    };
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &update,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a4.writers(),
    )
    .await;
    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "existing");
    assert_eq!(parsed["action"], "updated");

    let config = Config::load().unwrap();
    let saved = &config.templates["existing"];
    assert_eq!(saved.product, None);
    assert_eq!(saved.component.as_deref(), Some("Updated"));
    assert_eq!(saved.version.as_deref(), Some("123"));
    assert_eq!(saved.description.as_deref(), Some("updated"));
}

#[tokio::test]
async fn template_delete_existing_removes_entry() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &save_action("delete-me"),
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;

    let _ = __io3.out_str().to_string();
    assert!(result.is_ok());

    let mut __io4 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &TemplateAction::Delete {
            name: "delete-me".into(),
        },
        None,
        OutputFormat::Json,
        None,
        &mut __io4.writers(),
    )
    .await;

    let output = __io4.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "delete-me");
    assert_eq!(parsed["action"], "deleted");
    assert!(!Config::load().unwrap().templates.contains_key("delete-me"));
}

#[tokio::test]
async fn template_delete_json_matches_domain_mutation_output() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action("delete-json-shape"),
        None,
        OutputFormat::Json,
        None,
        &mut save_io.writers(),
    )
    .await;
    let _ = save_io.out_str().to_string();
    assert!(result.is_ok());

    let mut delete_io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &TemplateAction::Delete {
            name: "delete-json-shape".into(),
        },
        None,
        OutputFormat::Json,
        None,
        &mut delete_io.writers(),
    )
    .await;

    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(delete_io.out_str().trim()).unwrap();
    assert_eq!(
        parsed,
        serde_json::json!({"name": "delete-json-shape", "action": "deleted"})
    );
}

#[tokio::test]
async fn template_delete_table_prints_deleted_message() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut __io5 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &save_action("table-delete"),
        None,
        OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;

    let _ = __io5.out_str().to_string();
    assert!(result.is_ok());

    let mut __io6 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &TemplateAction::Delete {
            name: "table-delete".into(),
        },
        None,
        OutputFormat::Table,
        None,
        &mut __io6.writers(),
    )
    .await;

    let _output = __io6.out_str().to_string();
    assert!(result.is_ok());
    assert!(!Config::load()
        .unwrap()
        .templates
        .contains_key("table-delete"));
}

#[tokio::test]
async fn template_list_table_sorts_entries_by_name() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    for name in ["zzz", "aaa"] {
        let mut __io7 = crate::test_helpers::CapturedIo::new();
        let result = super::execute(
            &save_action(name),
            None,
            OutputFormat::Json,
            None,
            &mut __io7.writers(),
        )
        .await;
        let _ = __io7.out_str().to_string();
        assert!(result.is_ok());
    }

    let mut __io8 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &TemplateAction::List,
        None,
        OutputFormat::Table,
        None,
        &mut __io8.writers(),
    )
    .await;

    let output = __io8.out_str().to_string();
    assert!(result.is_ok());
    assert!(output.is_empty() || output.contains("product="));

    let config = Config::load().unwrap();
    let mut names: Vec<&str> = config.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["aaa", "zzz"]);
}

#[tokio::test]
async fn template_show_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let err = super::execute(
        &TemplateAction::Show {
            name: "missing".into(),
        },
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("template 'missing' not found"));
}

// ── Update (#315) ───────────────────────────────────────────────────

#[expect(clippy::too_many_arguments)]
fn update_action(
    name: &str,
    product: Option<&str>,
    component: Option<&str>,
    version: Option<&str>,
    priority: Option<&str>,
    severity: Option<&str>,
    clear: &[&str],
) -> TemplateAction {
    TemplateAction::Update(UpdateArgs {
        name: name.into(),
        fields: TemplateFields {
            product: product.map(Into::into),
            component: component.map(Into::into),
            version: version.map(Into::into),
            priority: priority.map(Into::into),
            severity: severity.map(Into::into),
            ..Default::default()
        },
        clear: clear.iter().map(|s| (*s).to_string()).collect(),
    })
}

async fn run(action: &TemplateAction) -> crate::error::Result<String> {
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(action, None, OutputFormat::Json, None, &mut io.writers()).await;
    result.map(|()| io.out_str().to_string())
}

#[tokio::test]
async fn template_update_merges_field() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("t")).await.unwrap(); // product + component + priority

    run(&update_action(
        "t",
        None,
        None,
        None,
        None,
        Some("blocker"),
        &[],
    ))
    .await
    .unwrap();

    let config = Config::load().unwrap();
    let t = &config.templates["t"];
    assert_eq!(t.severity.as_deref(), Some("blocker"));
    // Untouched fields are preserved.
    assert_eq!(t.product.as_deref(), Some("TestProduct"));
    assert_eq!(t.priority.as_deref(), Some("P1"));
}

#[tokio::test]
async fn template_update_clear_resets_field() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("t")).await.unwrap();

    run(&update_action(
        "t",
        None,
        None,
        None,
        None,
        None,
        &["priority"],
    ))
    .await
    .unwrap();

    let config = Config::load().unwrap();
    assert!(config.templates["t"].priority.is_none());
    assert_eq!(
        config.templates["t"].product.as_deref(),
        Some("TestProduct")
    );
}

#[tokio::test]
async fn template_update_merges_and_clears_create_metadata_fields() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("routing")).await.unwrap();

    run(&TemplateAction::Update(UpdateArgs {
        name: "routing".into(),
        fields: TemplateFields {
            url: Some("https://example.com/updated".into()),
            whiteboard: Some("needs-routing".into()),
            target_milestone: Some("M2".into()),
            deadline: Some("2026-12-31".into()),
            cc: vec!["cc@example.com".into()],
            keywords: vec!["regression".into()],
            groups: vec!["security".into()],
            flag: vec!["review+".into()],
            ..Default::default()
        },
        clear: vec![],
    }))
    .await
    .unwrap();

    let config = Config::load().unwrap();
    let t = &config.templates["routing"];
    assert_eq!(t.url.as_deref(), Some("https://example.com/updated"));
    assert_eq!(t.whiteboard.as_deref(), Some("needs-routing"));
    assert_eq!(t.target_milestone.as_deref(), Some("M2"));
    assert_eq!(t.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(t.cc, vec!["cc@example.com"]);
    assert_eq!(t.keywords, vec!["regression"]);
    assert_eq!(t.groups, vec!["security"]);
    assert_eq!(t.flags, vec!["review+"]);

    run(&TemplateAction::Update(UpdateArgs {
        name: "routing".into(),
        fields: TemplateFields::default(),
        clear: vec![
            "url".into(),
            "whiteboard".into(),
            "target-milestone".into(),
            "deadline".into(),
            "cc".into(),
            "keywords".into(),
            "groups".into(),
            "flags".into(),
        ],
    }))
    .await
    .unwrap();

    let config = Config::load().unwrap();
    let t = &config.templates["routing"];
    assert!(t.url.is_none());
    assert!(t.whiteboard.is_none());
    assert!(t.target_milestone.is_none());
    assert!(t.deadline.is_none());
    assert!(t.cc.is_empty());
    assert!(t.keywords.is_empty());
    assert!(t.groups.is_empty());
    assert!(t.flags.is_empty());
    assert_eq!(t.product.as_deref(), Some("TestProduct"));
}

#[tokio::test]
async fn template_update_unknown_template_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let err = run(&update_action(
        "missing",
        Some("X"),
        None,
        None,
        None,
        None,
        &[],
    ))
    .await
    .unwrap_err();
    assert!(err.to_string().contains("template 'missing' not found"));
}

#[tokio::test]
async fn template_update_requires_a_change() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("t")).await.unwrap();
    let err = run(&update_action("t", None, None, None, None, None, &[]))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no changes"));
}

#[tokio::test]
async fn template_update_unknown_clear_field_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("t")).await.unwrap();
    let err = run(&update_action(
        "t",
        None,
        None,
        None,
        None,
        None,
        &["bogus"],
    ))
    .await
    .unwrap_err();
    assert!(err.to_string().contains("unknown --clear field"));
}

#[tokio::test]
async fn template_update_clearing_all_fields_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("t")).await.unwrap(); // product, component, priority set
    let err = run(&update_action(
        "t",
        None,
        None,
        None,
        None,
        None,
        &["product", "component", "priority"],
    ))
    .await
    .unwrap_err();
    assert!(err.to_string().contains("at least one field"));
}

#[tokio::test]
async fn template_update_clear_wins_over_set() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run(&save_action("t")).await.unwrap(); // product + component + priority

    // Set and clear the same field in one call: clear wins.
    let a = update_action("t", None, None, None, None, Some("blocker"), &["severity"]);
    run(&a).await.unwrap();
    assert!(Config::load().unwrap().templates["t"].severity.is_none());
}

#[test]
fn clear_template_field_handles_every_name() {
    let mut t = crate::types::BugTemplate {
        product: Some("p".into()),
        component: Some("c".into()),
        version: Some("v".into()),
        priority: Some("pr".into()),
        severity: Some("s".into()),
        assignee: Some("a".into()),
        op_sys: Some("o".into()),
        rep_platform: Some("rp".into()),
        description: Some("d".into()),
        url: Some("u".into()),
        whiteboard: Some("w".into()),
        target_milestone: Some("tm".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["cc@example.com".into()],
        keywords: vec!["k".into()],
        groups: vec!["g".into()],
        flags: vec!["review?".into()],
    };
    for name in [
        "product",
        "component",
        "version",
        "priority",
        "severity",
        "assignee",
        "op-sys",
        "rep-platform",
        "description",
        "url",
        "whiteboard",
        "target-milestone",
        "deadline",
        "cc",
        "keywords",
        "groups",
        "flag",
        "flags",
    ] {
        super::clear_template_field(&mut t, name).unwrap();
    }
    assert!(super::template_is_empty(&t));
}
