#![expect(clippy::unwrap_used)]

use crate::cli::{QueryAction, ShowArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn query_show_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let err = crate::commands::query::execute(
        &QueryAction::Show(ShowArgs {
            name: "missing".into(),
        }),
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("query 'missing' not found"));
}
