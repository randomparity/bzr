#![expect(clippy::disallowed_methods, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use super::XmlRpcClient;
use crate::error::BzrError;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

fn multibyte_body_crossing_preview_boundary() -> String {
    let mut body = "a".repeat(511);
    body.push('é');
    body.push_str(" trailing");
    body
}

fn spawn_truncated_http_error_server() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _addr)) = listener.accept() else {
            return;
        };
        let _ = stream.read(&mut [0_u8; 1024]);
        let _ = stream
            .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 32\r\n\r\noops");
    });

    (format!("http://127.0.0.1:{port}"), handle)
}

fn xmlrpc_fault_response(code: i64, message: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
        <methodResponse>
          <fault>
            <value>
              <struct>
                <member>
                  <name>faultCode</name>
                  <value><int>{code}</int></value>
                </member>
                <member>
                  <name>faultString</name>
                  <value><string>{message}</string></value>
                </member>
              </struct>
            </value>
          </fault>
        </methodResponse>"#
    )
}

#[tokio::test]
async fn fault_response_maps_to_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_fault_response(102, "Access Denied")),
        )
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let err = client.get_bug("1").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("102"), "should contain fault code: {msg}");
    assert!(
        msg.contains("Access Denied"),
        "should contain message: {msg}"
    );
}

#[tokio::test]
async fn http_error_maps_to_xmlrpc_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let err = client.get_bug("1").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("500"), "should contain status code: {msg}");
}

#[tokio::test]
async fn http_error_preview_handles_multibyte_debug_preview_boundary() {
    let (_capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let mock = MockServer::start().await;
    let body = multibyte_body_crossing_preview_boundary();
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500).set_body_string(body.clone()))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let err = client.call("Bug.get", BTreeMap::new()).await.unwrap_err();

    assert!(
        matches!(
            &err,
            BzrError::HttpStatus { status: 500, body: returned } if returned == &body
        ),
        "expected HTTP 500 with original body, got: {err}"
    );
}

#[tokio::test]
async fn http_error_debug_body_redacts_api_key() {
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::DEBUG);
    let mock = MockServer::start().await;
    let secret = "XmlRpcSecret123";
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string(format!("request Bugzilla_api_key={secret} rejected")),
        )
        .mount(&mock)
        .await;
    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));

    let _ = client.call("Bug.get", BTreeMap::new()).await;

    let log = capture.output();
    assert!(
        !log.contains(secret),
        "API key leaked in tracing output: {log}"
    );
    assert!(
        log.contains("Bugzilla_api_key=[REDACTED]"),
        "redaction marker missing from tracing output: {log}"
    );
}

#[tokio::test]
async fn http_error_body_read_failure_preserves_context() {
    let (url, handle) = spawn_truncated_http_error_server();
    let client = XmlRpcClient::new(test_http_client(), &url, Some("test-key"));

    let err = client.call("Bug.get", BTreeMap::new()).await.unwrap_err();
    handle.join().unwrap();

    match err {
        BzrError::HttpStatus { status, body } => {
            assert_eq!(status, 500);
            assert!(
                body.contains("failed to read response body"),
                "body should preserve read failure context, got: {body}"
            );
        }
        other => assert!(matches!(other, BzrError::HttpStatus { .. })),
    }
}

#[tokio::test]
async fn anonymous_xmlrpc_call_omits_api_key() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), None);
    let _ = client.call("Bug.get", BTreeMap::new()).await;

    let requests = mock.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body = std::str::from_utf8(&requests[0].body).unwrap();
    assert!(!body.contains(crate::bugzilla_auth::AUTH_QUERY_PARAM));
}
