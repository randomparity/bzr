use crate::cli::CommentAction;

#[test]
fn filter_comment_body_strips_html_comments() {
    let raw = "Hello\n<!-- Enter your comment above this line -->\nWorld";
    assert_eq!(super::filter_comment_body(raw), "Hello\nWorld");
}

#[test]
fn filter_comment_body_preserves_normal_text() {
    let raw = "Just a comment\nwith multiple lines";
    assert_eq!(super::filter_comment_body(raw), raw);
}

#[test]
fn filter_comment_body_empty_input() {
    assert_eq!(super::filter_comment_body(""), "");
}

#[test]
fn capabilities_are_anonymous_for_read_actions() {
    let list = super::capabilities(&CommentAction::List {
        bug_id: 42,
        since: None,
        projection: crate::cli::ProjectionArgs::default(),
    });
    assert!(!list.supports_dry_run());
    assert_eq!(list.credential_requirement(), None);

    let search_tags = super::capabilities(&CommentAction::SearchTags {
        query: "need".into(),
    });
    assert!(!search_tags.supports_dry_run());
    assert_eq!(search_tags.credential_requirement(), None);
}

#[test]
fn capabilities_require_credentials_for_write_actions() {
    let add = super::capabilities(&CommentAction::Add {
        bug_id: 42,
        body: Some("hi".into()),
        body_file: None,
        private: false,
    });
    assert!(!add.supports_dry_run());
    assert_eq!(add.credential_requirement(), Some("comment add"));

    let tag = super::capabilities(&CommentAction::Tag {
        comment_id: 100,
        add: vec!["needinfo".into()],
        remove: vec![],
    });
    assert!(!tag.supports_dry_run());
    assert_eq!(tag.credential_requirement(), Some("comment tag"));
}
