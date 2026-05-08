#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn extract_json_skips_leading_noise() {
    let parsed = extract_json("warning: mixed output\n{\"id\":42,\"name\":\"bug\"}\n");
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["name"], "bug");
}

#[test]
fn extract_json_finds_bracketed_payload_with_trailing_noise() {
    let parsed = extract_json("noise\n[{\"id\":1},{\"id\":2}]\nextra output");
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[1]["id"], 2);
}

#[test]
fn xmlrpc_bug_response_contains_expected_bug_fields() {
    let xml = xmlrpc_bug_response(42, "Crash on startup");
    assert!(xml.contains("<int>42</int>"));
    assert!(xml.contains("<string>Crash on startup</string>"));
    assert!(xml.contains("<name>status</name>"));
    assert!(xml.contains("<string>NEW</string>"));
}

#[test]
fn captured_io_starts_empty() {
    let io = CapturedIo::new();
    assert!(io.out.is_empty());
    assert!(io.err.is_empty());
}

#[test]
fn captured_io_writers_route_to_owned_buffers() {
    let mut io = CapturedIo::new();
    {
        let w = io.writers();
        let _ = writeln!(w.out, "to stdout");
        let _ = writeln!(w.err, "to stderr");
    }
    assert_eq!(io.out_str(), "to stdout\n");
    assert_eq!(io.err_str(), "to stderr\n");
}
