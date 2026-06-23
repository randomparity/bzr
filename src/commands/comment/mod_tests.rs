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
fn requires_credentials_none_for_read_actions() {
    assert_eq!(
        super::requires_credentials(&CommentAction::List {
            bug_id: 42,
            since: None,
        }),
        None
    );
    assert_eq!(
        super::requires_credentials(&CommentAction::SearchTags {
            query: "need".into(),
        }),
        None
    );
}

#[test]
fn requires_credentials_names_the_write_command() {
    assert_eq!(
        super::requires_credentials(&CommentAction::Add {
            bug_id: 42,
            body: Some("hi".into()),
            body_file: None,
            private: false,
        }),
        Some("comment add")
    );
    assert_eq!(
        super::requires_credentials(&CommentAction::Tag {
            comment_id: 100,
            add: vec!["needinfo".into()],
            remove: vec![],
        }),
        Some("comment tag")
    );
}
