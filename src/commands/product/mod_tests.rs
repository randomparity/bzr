use super::{is_dry_runnable, requires_credentials};
use crate::cli::ProductAction;
use crate::types::ProductListType;

fn create_action() -> ProductAction {
    ProductAction::Create {
        from_json: None,
        name: Some("P".into()),
        description: Some("D".into()),
        version: Some("1.0".into()),
        is_open: Some(true),
    }
}

fn update_action() -> ProductAction {
    ProductAction::Update {
        from_json: None,
        name: Some("P".into()),
        description: Some("D".into()),
        default_milestone: None,
        is_open: None,
    }
}

#[test]
fn requires_credentials_reads_need_none() {
    assert_eq!(
        requires_credentials(&ProductAction::List {
            r#type: ProductListType::Accessible,
        }),
        None
    );
    assert_eq!(
        requires_credentials(&ProductAction::View { name: "P".into() }),
        None
    );
}

#[test]
fn requires_credentials_writes_name_the_command() {
    assert_eq!(
        requires_credentials(&create_action()),
        Some("product create")
    );
    assert_eq!(
        requires_credentials(&update_action()),
        Some("product update")
    );
}

#[test]
fn is_dry_runnable_only_for_mutations() {
    assert!(is_dry_runnable(&create_action()));
    assert!(is_dry_runnable(&update_action()));
    assert!(!is_dry_runnable(&ProductAction::List {
        r#type: ProductListType::Accessible,
    }));
    assert!(!is_dry_runnable(&ProductAction::View { name: "P".into() }));
}
