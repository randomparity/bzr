#![expect(clippy::unwrap_used)]

use crate::types::ProgressFormat;

#[test]
fn page_event_emits_compact_ndjson_line() {
    let mut buf: Vec<u8> = Vec::new();
    super::page_event(Some(ProgressFormat::Ndjson), &mut buf, 2, 100);
    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "{\"event\":\"page\",\"n\":2,\"fetched\":100}\n"
    );
}

#[test]
fn batch_event_emits_cumulative_counts() {
    let mut buf: Vec<u8> = Vec::new();
    super::batch_event(
        Some(ProgressFormat::Ndjson),
        &mut buf,
        &super::BatchProgress {
            n: 3,
            total: 10,
            ok: 2,
            failed: 1,
        },
    );
    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "{\"event\":\"batch\",\"n\":3,\"total\":10,\"ok\":2,\"failed\":1}\n"
    );
}

#[test]
fn done_event_emits_fetched() {
    let mut buf: Vec<u8> = Vec::new();
    super::done_event(Some(ProgressFormat::Ndjson), &mut buf, 500);
    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "{\"event\":\"done\",\"fetched\":500}\n"
    );
}

#[test]
fn error_event_emits_type_and_exit_code() {
    let mut buf: Vec<u8> = Vec::new();
    super::error_event(Some(ProgressFormat::Ndjson), &mut buf, "http", 5);
    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "{\"event\":\"error\",\"error_type\":\"http\",\"exit_code\":5}\n"
    );
}

#[test]
fn none_mode_writes_nothing() {
    let mut buf: Vec<u8> = Vec::new();
    super::page_event(None, &mut buf, 1, 1);
    super::batch_event(
        None,
        &mut buf,
        &super::BatchProgress {
            n: 1,
            total: 1,
            ok: 1,
            failed: 0,
        },
    );
    super::done_event(None, &mut buf, 1);
    super::error_event(None, &mut buf, "io", 6);
    assert!(buf.is_empty(), "None mode must emit nothing");
}
