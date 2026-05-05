#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn parse_flag_with_request() {
    let flags = parse_flags(&["review?(alice@example.com)".into()]).unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].name, "review");
    assert_eq!(flags[0].status, FlagStatus::Request);
    assert_eq!(flags[0].requestee.as_deref(), Some("alice@example.com"));
}

#[test]
fn parse_flag_grant() {
    let flags = parse_flags(&["review+".into()]).unwrap();
    assert_eq!(flags[0].name, "review");
    assert_eq!(flags[0].status, FlagStatus::Grant);
    assert!(flags[0].requestee.is_none());
}

#[test]
fn parse_flag_deny() {
    let flags = parse_flags(&["review-".into()]).unwrap();
    assert_eq!(flags[0].status, FlagStatus::Deny);
}

#[test]
fn parse_flag_no_status_char_fails() {
    let err = parse_flags(&["review".into()]).unwrap_err();
    assert!(err.to_string().contains("must contain"));
}

#[test]
fn parse_flag_empty_name_fails() {
    let err = parse_flags(&["?".into()]).unwrap_err();
    assert!(err.to_string().contains("cannot be empty"));
}

#[test]
fn parse_flag_bad_requestee_fails() {
    let err = parse_flags(&["review?alice".into()]).unwrap_err();
    assert!(err.to_string().contains("parentheses"));
}

#[test]
fn parse_flag_unclosed_paren_fails() {
    let err = parse_flags(&["review?(alice".into()]).unwrap_err();
    assert!(err.to_string().contains("parentheses"));
}

#[test]
fn parse_flag_unopened_paren_fails() {
    let err = parse_flags(&["review?alice)".into()]).unwrap_err();
    assert!(err.to_string().contains("parentheses"));
}

#[test]
fn parse_flag_clear() {
    let flags = parse_flags(&["reviewX".into()]).unwrap();
    assert_eq!(flags[0].name, "review");
    assert_eq!(flags[0].status, FlagStatus::Clear);
    assert!(flags[0].requestee.is_none());
}

#[test]
fn parse_multiple_flags() {
    let flags = parse_flags(&["review+".into(), "approval?".into()]).unwrap();
    assert_eq!(flags.len(), 2);
    assert_eq!(flags[0].name, "review");
    assert_eq!(flags[0].status, FlagStatus::Grant);
    assert_eq!(flags[1].name, "approval");
    assert_eq!(flags[1].status, FlagStatus::Request);
}

#[test]
fn parse_empty_flags() {
    let flags = parse_flags(&[]).unwrap();
    assert!(flags.is_empty());
}
