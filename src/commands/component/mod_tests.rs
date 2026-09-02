use crate::cli::ComponentAction;

fn create_action() -> ComponentAction {
    ComponentAction::Create {
        from_json: None,
        product: Some("P".into()),
        name: Some("C".into()),
        description: Some("D".into()),
        default_assignee: Some("dev@test.com".into()),
    }
}

#[test]
fn capabilities_are_anonymous_for_reads() {
    let list = super::capabilities(&ComponentAction::List {
        product: "P".into(),
        projection: crate::cli::ProjectionArgs::default(),
    });
    assert!(!list.supports_dry_run());
    assert_eq!(list.credential_requirement(), None);

    let view = super::capabilities(&ComponentAction::View {
        product: "P".into(),
        name: "C".into(),
        projection: crate::cli::ProjectionArgs::default(),
    });
    assert!(!view.supports_dry_run());
    assert_eq!(view.credential_requirement(), None);
}

#[test]
fn capabilities_allow_dry_run_and_credentials_for_writes() {
    let create = super::capabilities(&create_action());
    assert!(create.supports_dry_run());
    assert_eq!(create.credential_requirement(), Some("component create"));
}
