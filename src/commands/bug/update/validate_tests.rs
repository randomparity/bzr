#![expect(clippy::unwrap_used)]
//! Direct tests for the `bug update` field-combination validators
//! ([`super::validate_draft`] and the [`super::validate_args`] wrapper):
//! `--dupe-of` cannot pair with `--status`/`--resolution`, and `--alias` is
//! single-bug only.

use crate::cli::UpdateArgs;

use super::{validate_args, validate_draft, BugUpdateDraft};

#[test]
fn validate_draft_accepts_empty_draft() {
    validate_draft(&BugUpdateDraft::default(), &[1]).unwrap();
}

#[test]
fn validate_draft_accepts_dupe_of_alone() {
    let draft = BugUpdateDraft {
        dupe_of: Some(99),
        ..Default::default()
    };
    validate_draft(&draft, &[1]).unwrap();
}

#[test]
fn validate_draft_rejects_dupe_of_with_status() {
    let draft = BugUpdateDraft {
        dupe_of: Some(99),
        status: Some("RESOLVED".into()),
        ..Default::default()
    };
    let err = validate_draft(&draft, &[1]).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("--dupe-of") && msg.contains("--status")),
        "expected dupe-of/status conflict, got {err:?}"
    );
}

#[test]
fn validate_draft_rejects_dupe_of_with_resolution() {
    let draft = BugUpdateDraft {
        dupe_of: Some(99),
        resolution: Some("FIXED".into()),
        ..Default::default()
    };
    let err = validate_draft(&draft, &[1]).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("--dupe-of") && msg.contains("--resolution")),
        "expected dupe-of/resolution conflict, got {err:?}"
    );
}

#[test]
fn validate_draft_rejects_alias_with_multiple_ids() {
    let draft = BugUpdateDraft {
        alias: Some("short-name".into()),
        ..Default::default()
    };
    let err = validate_draft(&draft, &[1, 2]).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("--alias")),
        "expected --alias single-bug error, got {err:?}"
    );
}

#[test]
fn validate_draft_accepts_alias_with_single_id() {
    let draft = BugUpdateDraft {
        alias: Some("short-name".into()),
        ..Default::default()
    };
    validate_draft(&draft, &[1]).unwrap();
}

#[test]
fn validate_args_rejects_alias_with_multiple_ids() {
    let args = UpdateArgs {
        ids: vec![1, 2],
        alias: Some("short-name".into()),
        ..Default::default()
    };
    let err = validate_args(&args).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("--alias")),
        "expected --alias single-bug error, got {err:?}"
    );
}

#[test]
fn validate_args_accepts_valid_single_bug_update() {
    let args = UpdateArgs {
        ids: vec![1],
        status: Some("RESOLVED".into()),
        ..Default::default()
    };
    validate_args(&args).unwrap();
}
