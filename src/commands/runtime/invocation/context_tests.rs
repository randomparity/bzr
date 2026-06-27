use super::*;
use crate::types::output::OutputFormat;
use crate::types::transport::ApiMode;

#[test]
fn server_getter_returns_configured_value() {
    let ctx = CommandContext::new(Some("prod"), OutputFormat::Table, None);
    assert_eq!(ctx.server(), Some("prod"));
}

#[test]
fn server_getter_returns_none_when_not_set() {
    let ctx = CommandContext::new(None, OutputFormat::Table, None);
    assert!(ctx.server().is_none());
}

#[test]
fn assume_yes_getter_returns_true_when_set() {
    let ctx = CommandContext::new(None, OutputFormat::Table, None).with_assume_yes(true);
    assert!(ctx.assume_yes());
}

#[test]
fn assume_yes_getter_returns_false_by_default() {
    let ctx = CommandContext::new(None, OutputFormat::Table, None);
    assert!(!ctx.assume_yes());
}

#[test]
fn assume_yes_getter_returns_false_when_explicitly_set_false() {
    let ctx = CommandContext::new(None, OutputFormat::Table, None).with_assume_yes(false);
    assert!(!ctx.assume_yes());
}

#[test]
fn retry_max_getter_returns_configured_value() {
    let ctx = CommandContext::new(None, OutputFormat::Table, None).with_retry_max(3);
    assert_eq!(ctx.retry_max(), 3);
}

#[test]
fn retry_max_getter_returns_zero_by_default() {
    let ctx = CommandContext::new(None, OutputFormat::Table, None);
    assert_eq!(ctx.retry_max(), 0);
}

#[test]
fn retry_max_getter_distinguishes_nonzero_from_zero() {
    let ctx_default = CommandContext::new(None, OutputFormat::Table, None);
    let ctx_one = CommandContext::new(None, OutputFormat::Table, None).with_retry_max(1);
    let ctx_five = CommandContext::new(None, OutputFormat::Table, None).with_retry_max(5);
    assert_eq!(ctx_default.retry_max(), 0);
    assert_eq!(ctx_one.retry_max(), 1);
    assert_eq!(ctx_five.retry_max(), 5);
    assert_ne!(ctx_one.retry_max(), ctx_five.retry_max());
}

#[test]
fn with_server_override_changes_server() {
    let ctx = CommandContext::new(Some("prod"), OutputFormat::Table, None);
    let ctx2 = ctx.with_server(Some("staging"));
    assert_eq!(ctx2.server(), Some("staging"));
}

#[test]
fn with_server_override_clears_server() {
    let ctx = CommandContext::new(Some("prod"), OutputFormat::Table, None);
    let ctx2 = ctx.with_server(None);
    assert!(ctx2.server().is_none());
}

#[test]
fn api_mode_getter_returns_configured_value() {
    let ctx = CommandContext::new(None, OutputFormat::Table, Some(ApiMode::Rest));
    assert_eq!(ctx.api(), Some(ApiMode::Rest));
}
