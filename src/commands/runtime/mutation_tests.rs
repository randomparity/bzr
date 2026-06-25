#![expect(clippy::unwrap_used, clippy::panic)]

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::mutation::{ensure_batch_complete, run, DryRunPreview};
use crate::output::result_types::ResourceKind;
use crate::test_helpers::CapturedIo;
use crate::types::OutputFormat;

#[derive(Serialize)]
struct SampleParams {
    name: String,
}

#[tokio::test]
async fn dry_run_emits_preview_and_skips_execute() {
    let ctx = CommandContext::new(None, OutputFormat::Json, None).with_dry_run(true);
    let executed = AtomicBool::new(false);
    let mut io = CapturedIo::new();

    let result = run(
        &ctx,
        &mut io.writers(),
        DryRunPreview {
            resource: ResourceKind::Product,
            params: SampleParams {
                name: "Widget".to_string(),
            },
            message: "Would create product 'Widget'".to_string(),
        },
        |_client, _params: SampleParams| async {
            executed.store(true, Ordering::SeqCst);
            panic!("execute must not run on a dry run");
        },
    )
    .await;

    assert!(result.is_ok(), "dry-run mutation failed: {result:?}");
    assert!(
        !executed.load(Ordering::SeqCst),
        "execute closure ran on a dry run"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["resource"], "product");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["name"], "Widget");
}

#[test]
fn ensure_batch_complete_allows_all_successes() {
    assert!(ensure_batch_complete(2, 0).is_ok());
}

#[test]
fn ensure_batch_complete_reports_partial_failure_counts() {
    let err = ensure_batch_complete(1, 2).unwrap_err();

    assert!(matches!(
        err,
        crate::error::BzrError::BatchPartialFailure {
            succeeded: 1,
            failed: 2
        }
    ));
}
