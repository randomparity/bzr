use super::{is_dry_runnable, requires_credentials};
use crate::cli::GroupAction;

fn create_action() -> GroupAction {
    GroupAction::Create {
        from_json: None,
        name: Some("new-group".into()),
        description: Some("A test group".into()),
        is_active: Some(true),
    }
}

fn update_action() -> GroupAction {
    GroupAction::Update {
        from_json: None,
        group: Some("admin".into()),
        description: Some("Updated".into()),
        is_active: None,
    }
}

#[test]
fn requires_credentials_reads_need_none() {
    assert_eq!(
        requires_credentials(&GroupAction::ListUsers {
            group: "admin".into(),
            details: false,
        }),
        None
    );
    assert_eq!(
        requires_credentials(&GroupAction::View {
            group: "admin".into()
        }),
        None
    );
}

#[test]
fn requires_credentials_writes_name_the_command() {
    assert_eq!(
        requires_credentials(&GroupAction::AddUser {
            group: "admin".into(),
            user: "alice@test.com".into(),
        }),
        Some("group add-user")
    );
    assert_eq!(
        requires_credentials(&GroupAction::RemoveUser {
            group: "admin".into(),
            user: "bob@test.com".into(),
        }),
        Some("group remove-user")
    );
    assert_eq!(requires_credentials(&create_action()), Some("group create"));
    assert_eq!(requires_credentials(&update_action()), Some("group update"));
}

#[test]
fn is_dry_runnable_only_for_mutations() {
    assert!(is_dry_runnable(&create_action()));
    assert!(is_dry_runnable(&update_action()));
    assert!(!is_dry_runnable(&GroupAction::View {
        group: "admin".into()
    }));
    assert!(!is_dry_runnable(&GroupAction::ListUsers {
        group: "admin".into(),
        details: false,
    }));
    assert!(!is_dry_runnable(&GroupAction::AddUser {
        group: "admin".into(),
        user: "alice@test.com".into(),
    }));
    assert!(!is_dry_runnable(&GroupAction::RemoveUser {
        group: "admin".into(),
        user: "bob@test.com".into(),
    }));
}
