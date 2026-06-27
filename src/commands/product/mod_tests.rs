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
fn capabilities_are_anonymous_for_reads() {
    let list = super::capabilities(&ProductAction::List {
        r#type: ProductListType::Accessible,
        projection: crate::cli::ProjectionArgs::default(),
    });
    assert!(!list.supports_dry_run());
    assert_eq!(list.credential_requirement(), None);

    let view = super::capabilities(&ProductAction::View {
        name: "P".into(),
        projection: crate::cli::ProjectionArgs::default(),
    });
    assert!(!view.supports_dry_run());
    assert_eq!(view.credential_requirement(), None);
}

#[test]
fn capabilities_allow_dry_run_and_credentials_for_writes() {
    let create = super::capabilities(&create_action());
    assert!(create.supports_dry_run());
    assert_eq!(create.credential_requirement(), Some("product create"));

    let update = super::capabilities(&update_action());
    assert!(update.supports_dry_run());
    assert_eq!(update.credential_requirement(), Some("product update"));
}
