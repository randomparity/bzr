#![expect(clippy::unwrap_used)]

use super::{validate, UpdateArgs};

fn args<'a>(
    description: Option<&'a str>,
    default_assignee: Option<&'a str>,
    is_active: Option<bool>,
) -> UpdateArgs<'a> {
    UpdateArgs {
        product: "Widget",
        component: "Core",
        description,
        default_assignee,
        is_active,
    }
}

#[test]
fn update_requires_one_mutable_field() {
    let error = validate(&args(None, None, None)).unwrap_err();
    assert!(error.to_string().contains("no fields to update"));
}

#[test]
fn update_allows_false_active_value() {
    assert!(validate(&args(None, None, Some(false))).is_ok());
}

#[test]
fn update_rejects_blank_target() {
    let mut input = args(Some("changed"), None, None);
    input.component = " ";
    let error = validate(&input).unwrap_err();
    assert!(error.to_string().contains("--component must not be empty"));
}
