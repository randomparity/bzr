#![expect(clippy::unwrap_used)]

use crate::cli::{ExternalBugAction, ExternalBugArgs, RemoveExternalBugArgs};
use crate::types::OutputFormat;

#[tokio::test]
async fn external_bug_rejects_empty_external_id_before_connecting() {
    let action = crate::cli::BugAction::ExternalBug(ExternalBugArgs {
        action: ExternalBugAction::Remove(RemoveExternalBugArgs {
            id: 42,
            tracker: 7,
            external_id: "  ".into(),
        }),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let error = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.exit_code(), 7);
    assert!(error.to_string().contains("--external-id"));
}

#[tokio::test]
async fn external_bug_rejects_login_tokens_before_xmlrpc_dispatch() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &tmp,
        &format!("default_server = \"token\"\n\n[servers.token]\nurl = \"{}\"\ntoken = \"login-token\"\n", mock.uri()),
    );
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;
    let action = crate::cli::BugAction::ExternalBug(ExternalBugArgs {
        action: ExternalBugAction::Remove(RemoveExternalBugArgs {
            id: 42,
            tracker: 7,
            external_id: "EXT-1".into(),
        }),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let error = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path)),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.exit_code(), 9);
    assert!(error.to_string().contains("requires an API key"));
}

#[tokio::test]
async fn external_bug_dry_run_does_not_connect() {
    let action = crate::cli::BugAction::ExternalBug(ExternalBugArgs {
        action: ExternalBugAction::Remove(RemoveExternalBugArgs {
            id: 42,
            tracker: 7,
            external_id: "EXT-1".into(),
        }),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await
    .unwrap();
    let output: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(output["action"], "dry-run");
}
