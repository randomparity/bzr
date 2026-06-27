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
fn capabilities_are_anonymous_for_reads() {
    let list = super::capabilities(&GroupAction::ListUsers {
        group: "admin".into(),
        details: false,
        projection: crate::cli::ProjectionArgs::default(),
    });
    assert!(!list.supports_dry_run());
    assert_eq!(list.credential_requirement(), None);

    let view = super::capabilities(&GroupAction::View {
        group: "admin".into(),
    });
    assert!(!view.supports_dry_run());
    assert_eq!(view.credential_requirement(), None);
}

#[test]
fn capabilities_require_credentials_for_membership_writes() {
    let add = super::capabilities(&GroupAction::AddUser {
        group: "admin".into(),
        user: "alice@test.com".into(),
    });
    assert!(!add.supports_dry_run());
    assert_eq!(add.credential_requirement(), Some("group add-user"));

    let remove = super::capabilities(&GroupAction::RemoveUser {
        group: "admin".into(),
        user: "bob@test.com".into(),
    });
    assert!(!remove.supports_dry_run());
    assert_eq!(remove.credential_requirement(), Some("group remove-user"));
}

#[test]
fn capabilities_allow_dry_run_and_credentials_for_create_update() {
    let create = super::capabilities(&create_action());
    assert!(create.supports_dry_run());
    assert_eq!(create.credential_requirement(), Some("group create"));

    let update = super::capabilities(&update_action());
    assert!(update.supports_dry_run());
    assert_eq!(update.credential_requirement(), Some("group update"));
}
