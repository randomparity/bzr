#![expect(clippy::disallowed_methods, clippy::unwrap_used)]

use super::*;

use tracing::instrument::WithSubscriber as _;

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
fn tracing_capture_follows_an_async_future_across_threads() {
    let (capture, _guard) = TracingCapture::install(tracing::Level::DEBUG);
    let future = async { tracing::debug!("cross-thread tracing marker") }.with_current_subscriber();

    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future);
    })
    .join()
    .unwrap();

    assert!(capture.output().contains("cross-thread tracing marker"));
}

fn emit_callsite_registration_marker() {
    tracing::debug!("callsite registration marker");
}

#[test]
fn tracing_capture_survives_callsite_registration_on_another_thread() {
    let (capture, _guard) = TracingCapture::install(tracing::Level::DEBUG);

    std::thread::spawn(emit_callsite_registration_marker)
        .join()
        .unwrap();
    emit_callsite_registration_marker();

    assert_eq!(
        capture
            .output()
            .matches("callsite registration marker")
            .count(),
        1
    );
}
