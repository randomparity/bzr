#![expect(clippy::unwrap_used)]

use colored::Color;

use super::*;
use crate::types::OutputFormat;

// ── truncate ─────────────────────────────────────────────────────

#[test]
fn truncate_short_string_unchanged() {
    assert_eq!(truncate("hello", 10), "hello");
}

#[test]
fn truncate_exact_length_unchanged() {
    assert_eq!(truncate("abcde", 5), "abcde");
}

#[test]
fn truncate_long_string_adds_ellipsis() {
    let result = truncate("abcdefghij", 7);
    assert_eq!(result, "abcd...");
    assert_eq!(result.len(), 7);
}

#[test]
fn truncate_unicode_counts_chars_not_bytes() {
    let input = "\u{1f600}\u{1f601}\u{1f602}\u{1f603}";
    let result = truncate(input, 4);
    assert_eq!(result, input);
    let result = truncate(input, 3);
    assert_eq!(result, "...");
}

// ── colorize_status ──────────────────────────────────────────────

#[test]
fn colorize_status_new_is_green() {
    assert_eq!(colorize_status("NEW").fgcolor, Some(Color::Green));
}

#[test]
fn colorize_status_assigned_is_yellow() {
    assert_eq!(colorize_status("ASSIGNED").fgcolor, Some(Color::Yellow));
}

#[test]
fn colorize_status_resolved_is_red() {
    assert_eq!(colorize_status("RESOLVED").fgcolor, Some(Color::Red));
}

#[test]
fn colorize_status_unknown_passes_through() {
    let result = colorize_status("CUSTOM");
    assert_eq!(result.fgcolor, None);
    assert!(result.contains("CUSTOM"));
}

#[test]
fn colorize_status_case_insensitive() {
    assert_eq!(colorize_status("new").fgcolor, Some(Color::Green));
}

#[test]
fn colorize_status_known_statuses_map_to_distinct_colors() {
    // Inspect the `ColoredString`'s color metadata directly instead of
    // forcing `colored`'s process-global override on. The override is shared
    // across the whole test binary, so flipping it raced with every
    // concurrent test that asserts colorless buffer output. Checking the
    // `fgcolor` field per status still kills `delete match arm` mutations: a
    // deleted arm falls through to the catch-all `.normal()` (None).
    for (status, want) in [
        ("NEW", Some(Color::Green)),
        ("UNCONFIRMED", Some(Color::Green)),
        ("ASSIGNED", Some(Color::Yellow)),
        ("IN_PROGRESS", Some(Color::Yellow)),
        ("RESOLVED", Some(Color::Red)),
        ("VERIFIED", Some(Color::Red)),
        ("CLOSED", Some(Color::Red)),
    ] {
        assert_eq!(
            colorize_status(status).fgcolor,
            want,
            "unexpected color for status {status:?}"
        );
    }
    assert_eq!(colorize_status("CUSTOM").fgcolor, None);
}

// ── shorten_email ────────────────────────────────────────────────

#[test]
fn shorten_email_strips_domain() {
    assert_eq!(shorten_email("alice@example.com"), "alice");
}

#[test]
fn shorten_email_no_at_unchanged() {
    assert_eq!(shorten_email("alice"), "alice");
}

#[test]
fn shorten_email_empty_string() {
    assert_eq!(shorten_email(""), "");
}

#[test]
fn shorten_email_uses_first_at_symbol() {
    assert_eq!(shorten_email("alice@dev@example.com"), "alice");
}

#[test]
fn yes_no_formats_boolean_values() {
    assert_eq!(yes_no(true), "Yes");
    assert_eq!(yes_no(false), "No");
}

#[test]
fn opt_yes_no_formats_optional_boolean_values() {
    assert_eq!(opt_yes_no(Some(true)), "Yes");
    assert_eq!(opt_yes_no(Some(false)), "No");
    assert_eq!(opt_yes_no(None), "-");
}

// ── OutputFormat parsing ─────────────────────────────────────────

#[test]
fn output_format_from_str() {
    assert_eq!(
        "table".parse::<OutputFormat>().unwrap(),
        OutputFormat::Table
    );
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!(
        "ndjson".parse::<OutputFormat>().unwrap(),
        OutputFormat::Ndjson
    );
    assert!("JSON".parse::<OutputFormat>().is_err());
    assert!("Table".parse::<OutputFormat>().is_err());
    assert!("xml".parse::<OutputFormat>().is_err());
    let err = "XML".parse::<OutputFormat>().unwrap_err();
    assert!(err.contains("expected 'table', 'json', or 'ndjson'"));
}

// ── NDJSON rendering ─────────────────────────────────────────────

#[test]
fn ndjson_array_emits_one_compact_line_per_element() {
    let value = serde_json::json!([{"id": 1}, {"id": 2}, {"id": 3}]);
    let mut buf = Vec::new();
    write_ndjson(&value, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert_eq!(out, "{\"id\":1}\n{\"id\":2}\n{\"id\":3}\n");
}

#[test]
fn ndjson_empty_array_emits_nothing() {
    let value = serde_json::json!([]);
    let mut buf = Vec::new();
    write_ndjson(&value, &mut buf);
    assert!(buf.is_empty());
}

#[test]
fn ndjson_single_object_emits_one_compact_line() {
    let value = serde_json::json!({"resource": "bug", "action": "updated", "id": 7});
    let mut buf = Vec::new();
    write_ndjson(&value, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    // One line, no pretty-print whitespace, trailing newline only.
    assert_eq!(out.lines().count(), 1);
    assert!(!out.contains("  "));
    assert!(out.ends_with('\n'));
}

#[test]
fn json_family_json_is_pretty_ndjson_is_compact() {
    let value = serde_json::json!([{"id": 1}]);
    let mut pretty = Vec::new();
    write_json_family(&value, OutputFormat::Json, &mut pretty);
    let pretty = String::from_utf8(pretty).unwrap();
    assert!(pretty.contains('\n') && pretty.contains("  "));

    let mut compact = Vec::new();
    write_json_family(&value, OutputFormat::Ndjson, &mut compact);
    assert_eq!(String::from_utf8(compact).unwrap(), "{\"id\":1}\n");
}

#[test]
fn output_format_default_is_table() {
    assert_eq!(OutputFormat::default(), OutputFormat::Table);
}

// ── print_result ─────────────────────────────────────────────────

#[test]
fn print_result_json_serializes_value() {
    let value = serde_json::json!({"id": 42});
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"id":42}"#);
}

// ── mask_api_key tests ──────────────────────────────────────────

#[test]
fn mask_api_key_long_key_shows_prefix() {
    assert_eq!(mask_api_key("abcdefghijklmnop"), "abcdefgh...");
}

#[test]
fn mask_api_key_short_key_fully_masked() {
    assert_eq!(mask_api_key("short"), "***");
}

#[test]
fn mask_api_key_exactly_8_chars_fully_masked() {
    assert_eq!(mask_api_key("12345678"), "***");
}

#[test]
fn mask_api_key_empty_string_fully_masked() {
    assert_eq!(mask_api_key(""), "***");
}

#[test]
fn mask_api_key_multibyte_char_at_boundary_does_not_panic() {
    // 'é' is two bytes spanning byte offset 7..9, so byte-slicing at 8
    // lands mid-codepoint. Masking must count chars, not bytes.
    let result = mask_api_key("1234567é9abcdef");
    assert_eq!(result, "1234567é...");
}

#[test]
fn truncate_max_chars_below_ellipsis_width_does_not_panic() {
    // max_chars < 3 must not underflow `max_chars - 3`.
    assert_eq!(truncate("abcdefg", 2), "...");
    assert_eq!(truncate("abcdefg", 0), "...");
}
