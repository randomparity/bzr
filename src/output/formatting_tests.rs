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

// ── projection-aware writers ─────────────────────────────────────

#[test]
fn write_formatted_projected_trims_ndjson_object() {
    use crate::validation::fields::FieldProjection;
    let proj = FieldProjection::resolve(Some("id"), None, &["id", "name"]).unwrap();
    let value = serde_json::json!({"id": 1, "name": "x"});
    let mut buf = Vec::new();
    write_formatted_projected(&value, OutputFormat::Ndjson, &proj, &mut buf, |_, _| {});
    assert_eq!(String::from_utf8(buf).unwrap().trim(), r#"{"id":1}"#);
}

#[test]
fn write_formatted_projected_table_ignores_projection() {
    use crate::validation::fields::FieldProjection;
    let proj = FieldProjection::resolve(Some("id"), None, &["id", "name"]).unwrap();
    let value = serde_json::json!({"id": 1, "name": "x"});
    let mut buf = Vec::new();
    write_formatted_projected(&value, OutputFormat::Table, &proj, &mut buf, |_, out| {
        let _ = writeln!(out, "TABLE");
    });
    assert_eq!(String::from_utf8(buf).unwrap().trim(), "TABLE");
}

#[test]
fn projected_resource_pattern_trims_each_ndjson_element() {
    use crate::validation::fields::FieldProjection;
    let proj = FieldProjection::resolve(Some("id"), None, &["id", "name"]).unwrap();
    let items = vec![
        serde_json::json!({"id": 1, "name": "a"}),
        serde_json::json!({"id": 2, "name": "b"}),
    ];
    let mut buf = Vec::new();
    let spec = TableSpec {
        empty_msg: "none",
        headers: &["ID"],
    };
    // Mirrors how the table-resource writers call the projected helper.
    write_formatted_projected(
        &items,
        OutputFormat::Ndjson,
        &proj,
        &mut buf,
        |items, out| {
            write_records_or_empty(items, spec, |_| vec!["x".to_string()], None, out);
        },
    );
    assert_eq!(
        String::from_utf8(buf).unwrap().trim(),
        "{\"id\":1}\n{\"id\":2}"
    );
}

#[test]
fn write_records_or_empty_prints_empty_message() {
    let items: Vec<serde_json::Value> = Vec::new();
    let mut buf = Vec::new();
    let spec = TableSpec {
        empty_msg: "none here",
        headers: &["ID"],
    };
    write_records_or_empty(&items, spec, |_| vec!["x".to_string()], None, &mut buf);
    assert_eq!(String::from_utf8(buf).unwrap().trim(), "none here");
}

#[test]
fn write_records_or_empty_populated_table_remains_unbounded_by_default() {
    let items = ["a very long value"];
    let mut buf = Vec::new();
    let spec = TableSpec {
        empty_msg: "none here",
        headers: &["Value"],
    };

    write_records_or_empty(
        &items,
        spec,
        |item| vec![(*item).to_string()],
        None,
        &mut buf,
    );

    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "+-------------------+\n| Value             |\n+-------------------+\n| a very long value |\n+-------------------+\n"
    );
}

// ── escape_terminal_controls ─────────────────────────────────────

/// Pins the exclusion ADR 0065 records rather than a behaviour it adds: the
/// JSON family is a published schema surface, so the escaping must not migrate
/// into it. `serde_json` escapes only `"`, `\`, and code points below `0x20`,
/// which is why the bidi override survives there and the ESC does not.
#[test]
fn json_family_output_is_not_escaped_for_bidi() {
    let value = serde_json::json!({ "summary": "ev\u{1b}[2Jil\u{202e}" });

    let mut json = Vec::new();
    write_json(&value, &mut json);
    let mut ndjson = Vec::new();
    write_ndjson(&value, &mut ndjson);

    for (rendered, what) in [(json, "write_json"), (ndjson, "write_ndjson")] {
        let rendered = String::from_utf8(rendered).unwrap();
        assert!(
            rendered.contains('\u{202e}'),
            "{what} must leave the bidi override to the JSON contract: {rendered:?}"
        );
        assert!(
            !rendered.contains("\\u{202e}"),
            "{what} must not carry bzr's non-JSON escape spelling: {rendered:?}"
        );
    }
}

#[test]
fn write_field_family_escapes_labels_and_values() {
    let mut buf = Vec::new();
    write_field(&mut buf, "la\u{1b}bel", "va\u{202e}lue");
    write_optional_field(&mut buf, "op\u{202e}t", Some("so\u{1b}me"));
    write_list_field(
        &mut buf,
        "li\u{1b}st",
        &["fi\u{202e}rst".to_string(), "sec\u{1b}ond".to_string()],
    );

    let output = String::from_utf8(buf).unwrap();
    assert!(
        !output.contains('\u{1b}') && !output.contains('\u{202e}'),
        "no detail row may carry a raw control or bidi character: {output:?}"
    );
    assert_eq!(
        output.matches("\\u{1b}").count(),
        4,
        "every label and value in the family is escaped: {output:?}"
    );
    assert_eq!(
        output.matches("\\u{202e}").count(),
        3,
        "every label and value in the family is escaped: {output:?}"
    );
}

#[test]
fn write_status_field_escapes_the_status_and_still_colours_it() {
    let mut buf = Vec::new();
    write_status_field(&mut buf, "Sta\u{1b}tus", "NEW\u{202e}");

    let output = String::from_utf8(buf).unwrap();
    assert!(
        !output.contains('\u{1b}') && !output.contains('\u{202e}'),
        "the server's own controls must not survive: {output:?}"
    );
    assert!(
        output.contains("\\u{1b}") && output.contains("\\u{202e}"),
        "both must survive in escaped form: {output:?}"
    );
    // Colour is asserted on the `ColoredString` rather than on emitted ANSI:
    // `colored::control::set_override` is process-global and flakes parallel
    // tests that assert on colourless output. A real status carries no control
    // character, so escaping is the identity on it and the colour match holds.
    assert_eq!(escape_terminal_controls("NEW"), "NEW");
    assert_eq!(
        colorize_status(&escape_terminal_controls("NEW")).fgcolor,
        Some(Color::Green),
        "escaping must not defeat the status-colour match"
    );
}

#[test]
fn escape_terminal_controls_escapes_cc_and_bidi_only() {
    let escaped = escape_terminal_controls(
        "a\u{1b}b\tc\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}d\u{2066}\u{2067}\u{2068}\u{2069}\
         e\u{200e}\u{200f}f\u{61c}g\u{200c}h\u{200d}i\u{200b}j\u{feff}ké\\",
    );

    assert_eq!(
        escaped,
        "a\\u{1b}b\\tc\\u{202a}\\u{202b}\\u{202c}\\u{202d}\\u{202e}\
         d\\u{2066}\\u{2067}\\u{2068}\\u{2069}e\\u{200e}\\u{200f}f\\u{61c}\
         g\u{200c}h\u{200d}i\u{200b}j\u{feff}ké\\",
        "must escape Cc and the Trojan-Source bidi set and nothing else"
    );
}

fn table(headers: &[&str], rows: &[&[&str]]) -> tabled::Table {
    let mut builder = tabled::builder::Builder::default();
    builder.push_record(headers.iter().copied());
    for row in rows {
        builder.push_record(row.iter().copied());
    }
    builder.build()
}

fn display_width(line: &str) -> usize {
    line.chars()
        .map(|character| match character {
            '\u{1100}'..='\u{115F}'
            | '\u{2E80}'..='\u{A4CF}'
            | '\u{AC00}'..='\u{D7A3}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FE10}'..='\u{FE19}'
            | '\u{FE30}'..='\u{FE6F}'
            | '\u{FF00}'..='\u{FF60}'
            | '\u{FFE0}'..='\u{FFE6}' => 2,
            _ => 1,
        })
        .sum()
}

fn records(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|cell| (*cell).to_string()).collect())
        .collect()
}

#[test]
fn write_table_records_escapes_cells_and_headers() {
    let mut buf = Vec::new();
    write_table_records(
        &["N\u{202e}AME"],
        records(&[&["ev\u{1b}[2Jil\u{202e}"]]),
        None,
        &mut buf,
    );

    let output = String::from_utf8(buf).unwrap();
    assert!(
        !output.contains('\u{1b}') && !output.contains('\u{202e}'),
        "no raw control or bidi character may reach the terminal: {output:?}"
    );
    assert!(
        output.contains("\\u{1b}") && output.contains("\\u{202e}"),
        "both must survive in escaped form: {output:?}"
    );
}

#[test]
fn write_table_records_unbounded_preserves_existing_table_bytes() {
    let expected = format!("{}\n", table(&["Name", "State"], &[&["long enough", "OK"]]));
    let mut buf = Vec::new();

    write_table_records(
        &["Name", "State"],
        records(&[&["long enough", "OK"]]),
        None,
        &mut buf,
    );

    assert_eq!(String::from_utf8(buf).unwrap(), expected);
}

#[test]
fn write_table_records_wraps_ascii_lines_to_injected_width() {
    let mut buf = Vec::new();
    write_table_records(
        &["Description", "State"],
        records(&[&["a long sequence of words that must wrap", "OK"]]),
        Some(24),
        &mut buf,
    );

    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("must"));
    assert!(output.lines().all(|line| display_width(line) <= 24));
}

#[test]
fn write_table_records_wraps_unicode_lines_to_injected_display_width() {
    let mut buf = Vec::new();
    write_table_records(
        &["City", "State"],
        records(&[&["東京市場東京市場", "OK"]]),
        Some(20),
        &mut buf,
    );

    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("東京"));
    assert!(output.lines().all(|line| display_width(line) <= 20));
}

#[test]
fn write_table_records_clamps_to_structural_floor_without_replacing_width_two_scalar() {
    let mut buf = Vec::new();
    write_table_records(
        &["Description", "State"],
        records(&[&[
            "a very long ASCII value that should surrender width first",
            "東",
        ]]),
        Some(1),
        &mut buf,
    );

    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains('東'));
    assert!(!output.contains('�'));
    assert!(output.lines().all(|line| display_width(line) <= 11));
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

// ── versioned envelope ───────────────────────────────────────────

#[test]
fn write_json_wraps_payload_in_versioned_envelope() {
    let payload = serde_json::json!([{"id": 1}, {"id": 2}]);
    let mut buf = Vec::new();
    write_json(&payload, &mut buf);
    let parsed: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();

    let obj = parsed.as_object().unwrap();
    let keys: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["data", "schema_version"].into_iter().collect(),
        "envelope must have exactly schema_version and data"
    );
    assert_eq!(
        parsed["schema_version"].as_str().unwrap(),
        crate::output::SCHEMA_VERSION
    );
    assert_eq!(parsed["data"], payload, "data must equal the bare payload");
}

#[test]
fn write_json_family_json_arm_is_enveloped() {
    let value = serde_json::json!({"count": 0});
    let mut buf = Vec::new();
    write_json_family(&value, OutputFormat::Json, &mut buf);
    let parsed: serde_json::Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
    assert_eq!(parsed["data"], value);
    assert_eq!(
        parsed["schema_version"].as_str().unwrap(),
        crate::output::SCHEMA_VERSION
    );
}

#[test]
fn schema_version_is_three_part_semver() {
    let parts: Vec<&str> = crate::output::SCHEMA_VERSION.split('.').collect();
    assert_eq!(parts.len(), 3, "SCHEMA_VERSION must be MAJOR.MINOR.PATCH");
    for part in parts {
        assert!(
            !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()),
            "SCHEMA_VERSION component {part:?} must be all digits"
        );
    }
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
