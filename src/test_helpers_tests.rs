use super::*;

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

#[test]
fn make_attachment_sets_common_defaults() {
    let att = make_attachment(10, 42, "patch.diff", "Fix patch", None);

    assert_eq!(att.id, 10);
    assert_eq!(att.bug_id, 42);
    assert_eq!(att.file_name, "patch.diff");
    assert_eq!(att.summary, "Fix patch");
    assert_eq!(att.content_type, "text/plain");
    assert_eq!(att.creator.as_deref(), Some("author@example.com"));
    assert_eq!(att.creation_time.as_deref(), Some("2025-03-01T09:00:00Z"));
    assert_eq!(
        att.last_change_time.as_deref(),
        Some("2025-03-02T10:00:00Z")
    );
    assert_eq!(att.size, 1234);
    assert!(!att.is_obsolete);
    assert!(!att.is_private);
    assert!(!att.is_patch);
    assert!(att.data.is_none());
}

#[test]
fn make_attachment_preserves_inline_data() {
    let att = make_attachment(10, 42, "patch.diff", "Fix patch", Some("aGVsbG8=".into()));

    assert_eq!(att.data.as_deref(), Some("aGVsbG8="));
}
