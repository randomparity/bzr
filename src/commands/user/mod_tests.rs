use super::{is_dry_runnable, requires_credentials};
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
    }
}

#[test]
fn requires_credentials_reads_need_none() {
    assert_eq!(requires_credentials(&search_action()), None);
}

#[test]
fn requires_credentials_writes_name_the_command() {
    assert_eq!(requires_credentials(&create_action()), Some("user create"));
    assert_eq!(requires_credentials(&update_action()), Some("user update"));
}

#[test]
fn is_dry_runnable_only_for_mutations() {
    assert!(is_dry_runnable(&create_action()));
    assert!(is_dry_runnable(&update_action()));
    assert!(!is_dry_runnable(&search_action()));
}
