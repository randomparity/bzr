use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::GroupAction;
use crate::test_helpers::setup_isolated_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn group_remove_user_sends_put() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (mock, _tmp, config_path) = setup_isolated_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/bob%40test%2Ecom"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"users": [{"id": 2, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::RemoveUser {
        group: "admin".into(),
        user: "bob@test.com".into(),
    };
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group remove_user failed: {result:?}");
}
