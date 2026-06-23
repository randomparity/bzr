#![expect(clippy::unwrap_used)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use super::*;
use crate::types::AuthMethod;

fn spawn_truncated_http_response_server() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _addr)) = listener.accept() else {
            return;
        };
        let _ = stream.read(&mut [0_u8; 1024]);
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Length: 32\r\nContent-Type: application/json\r\n\r\n{\"id\"",
        );
    });

    (format!("http://127.0.0.1:{port}/rest/whoami"), handle)
}

#[test]
fn whoami_response_deserializes_with_id() {
    let json = r#"{"id": 42, "name": "user@example.com"}"#;
    let resp: WhoamiProbeResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, 42);
}

#[test]
fn whoami_response_id_zero_means_unauthenticated() {
    let json = r#"{"id": 0}"#;
    let resp: WhoamiProbeResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, 0);
}

#[test]
fn whoami_response_missing_id_defaults_zero() {
    let json = r#"{"name": "test"}"#;
    let resp: WhoamiProbeResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, 0);
}

#[tokio::test]
async fn whoami_response_body_read_error_is_network_error() {
    let (url, handle) = spawn_truncated_http_response_server();
    let client = reqwest::Client::new();

    let outcome = super::probe_whoami(client.get(url), AuthMethod::Header).await;
    handle.join().unwrap();

    assert!(matches!(outcome, WhoamiOutcome::NetworkError(_)));
}
