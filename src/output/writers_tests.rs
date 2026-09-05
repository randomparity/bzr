use std::ffi::OsStr;

use super::{resolve_table_width, TableWidth, Writers};

#[test]
fn writers_new_holds_two_streams_and_writes_independently() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    {
        let w = Writers::new(&mut out, &mut err);
        let _ = writeln!(w.out, "hello stdout");
        let _ = writeln!(w.err, "hello stderr");
    }
    assert_eq!(out, b"hello stdout\n");
    assert_eq!(err, b"hello stderr\n");
}

#[test]
fn writers_out_and_err_are_independent_buffers() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    {
        let w = Writers::new(&mut out, &mut err);
        let _ = w.out.write_all(b"A");
        let _ = w.err.write_all(b"B");
        let _ = w.out.write_all(b"C");
    }
    assert_eq!(out, b"AC");
    assert_eq!(err, b"B");
}

#[test]
fn table_width_explicit_value_overrides_detected_width() {
    assert_eq!(
        resolve_table_width(Some(OsStr::new("80")), Some(120)),
        TableWidth(Some(80))
    );
}

#[test]
fn table_width_absent_uses_detected_width() {
    assert_eq!(resolve_table_width(None, Some(120)), TableWidth(Some(120)));
}

#[test]
fn table_width_absent_values_leave_width_unbounded() {
    assert_eq!(resolve_table_width(None, None), TableWidth(None));
}

#[test]
fn table_width_invalid_zero_falls_back_to_detected_width() {
    assert_eq!(
        resolve_table_width(Some(OsStr::new("0")), Some(120)),
        TableWidth(Some(120))
    );
}

#[test]
fn table_width_invalid_text_falls_back_to_detected_width() {
    assert_eq!(
        resolve_table_width(Some(OsStr::new("wide")), Some(120)),
        TableWidth(Some(120))
    );
}

#[test]
fn table_width_out_of_range_falls_back_to_detected_width() {
    assert_eq!(
        resolve_table_width(Some(OsStr::new("65536")), Some(120)),
        TableWidth(Some(120))
    );
}

#[test]
fn table_width_zero_detection_leaves_width_unbounded() {
    assert_eq!(resolve_table_width(None, Some(0)), TableWidth(None));
}

#[test]
fn writers_table_width_defaults_to_none() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let writers = Writers::new(&mut out, &mut err);

    assert_eq!(writers.table_width(), None);
}

#[test]
fn writers_table_width_retains_directly_injected_value() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let writers = Writers::with_table_width(&mut out, &mut err, TableWidth(Some(80)));

    assert_eq!(writers.table_width(), Some(80));
}

#[cfg(unix)]
#[test]
fn table_width_non_unicode_override_warns_without_rendering_raw_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::WARN);
    let invalid = std::ffi::OsString::from_vec(vec![b'8', 0xFF]);

    assert_eq!(
        resolve_table_width(Some(&invalid), Some(120)),
        TableWidth(Some(120))
    );
    let warning = capture.output();
    assert!(warning.contains("BZR_TABLE_WIDTH"));
    assert!(!warning.contains("8�"));
}
