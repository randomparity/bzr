use super::{is_dry_runnable, requires_credentials};
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

fn update_action() -> ComponentAction {
    ComponentAction::Update {
        from_json: None,
        id: Some(1),
        product: None,
        component: None,
        name: Some("Renamed".into()),
        description: None,
        default_assignee: None,
    }
}

#[test]
fn requires_credentials_reads_need_none() {
    assert_eq!(
        requires_credentials(&ComponentAction::List {
            product: "P".into()
        }),
        None
    );
    assert_eq!(
        requires_credentials(&ComponentAction::View {
            product: "P".into(),
            name: "C".into(),
        }),
        None
    );
}

#[test]
fn requires_credentials_writes_name_the_command() {
    assert_eq!(
        requires_credentials(&create_action()),
        Some("component create")
    );
    assert_eq!(
        requires_credentials(&update_action()),
        Some("component update")
    );
}

#[test]
fn is_dry_runnable_only_for_mutations() {
    assert!(is_dry_runnable(&create_action()));
    assert!(is_dry_runnable(&update_action()));
    assert!(!is_dry_runnable(&ComponentAction::List {
        product: "P".into()
    }));
    assert!(!is_dry_runnable(&ComponentAction::View {
        product: "P".into(),
        name: "C".into(),
    }));
}
