#![expect(clippy::unwrap_used)]

use crate::cli::{QueryAction, ShowArgs};
use crate::test_helpers::setup_isolated_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn query_show_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_mock, _tmp, config_path) = setup_isolated_env().await;

    let err = crate::commands::query::execute(
        &QueryAction::Show(ShowArgs {
            name: "missing".into(),
        }),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_config_path_override(Some(config_path.clone())),
        &mut __cap_io.writers(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("query 'missing' not found"));
}
