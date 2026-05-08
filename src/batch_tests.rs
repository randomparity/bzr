use super::*;

#[test]
fn batch_outcome_all_success_is_complete() {
    let outcome = BatchOutcome::new(3, 0);

    assert!(outcome.ensure_complete().is_ok());
}

#[test]
fn batch_outcome_failures_return_partial_failure_error() {
    let outcome = BatchOutcome::new(2, 1);

    assert!(matches!(
        outcome.ensure_complete(),
        Err(crate::error::BzrError::BatchPartialFailure {
            succeeded: 2,
            failed: 1,
        })
    ));
}
