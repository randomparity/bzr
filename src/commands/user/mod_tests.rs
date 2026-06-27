use crate::cli::UserAction;

fn create_action() -> UserAction {
    UserAction::Create {
        from_json: None,
        email: Some("new@test.com".into()),
        login: None,
        full_name: Some("New User".into()),
        password: None,
    }
}

fn update_action() -> UserAction {
    UserAction::Update {
        from_json: None,
        user: Some("alice@test.com".into()),
        real_name: Some("Alice".into()),
        email: None,
        disable_login: None,
        login_denied_text: None,
    }
}

fn search_action() -> UserAction {
    UserAction::Search {
        query: "alice".into(),
        details: false,
        projection: crate::cli::ProjectionArgs::default(),
    }
}

#[test]
fn capabilities_are_anonymous_for_search() {
    let capabilities = super::capabilities(&search_action());
    assert!(!capabilities.supports_dry_run());
    assert_eq!(capabilities.credential_requirement(), None);
}

#[test]
fn capabilities_allow_dry_run_and_credentials_for_writes() {
    let create = super::capabilities(&create_action());
    assert!(create.supports_dry_run());
    assert_eq!(create.credential_requirement(), Some("user create"));

    let update = super::capabilities(&update_action());
    assert!(update.supports_dry_run());
    assert_eq!(update.credential_requirement(), Some("user update"));
}
