#![expect(clippy::unwrap_used)]

use super::XmlRpcClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_http_client() -> reqwest::Client {
    reqwest::Client::new()
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let err = client.get_bug("1").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("500"), "should contain status code: {msg}");
}
