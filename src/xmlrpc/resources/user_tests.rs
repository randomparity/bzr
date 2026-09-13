#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use super::extract_id;
use crate::types::CreateUserParams;
use crate::xmlrpc::protocol::Value;
use crate::xmlrpc::protocol::XmlRpcClient;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

#[test]
fn extract_id_requires_struct_with_integer_id() {
    let err = extract_id(&Value::String("oops".into())).unwrap_err();
    assert!(err.to_string().contains("expected struct response"));

    let err = extract_id(&Value::Struct(BTreeMap::new())).unwrap_err();
    assert!(err.to_string().contains("missing id field"));
}

#[tokio::test]
async fn create_user_returns_id_from_response() {
    let mock = MockServer::start().await;
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <methodResponse>
          <params>
            <param>
              <value>
                <struct>
                  <member>
                    <name>id</name>
                    <value><int>4242</int></value>
                  </member>
                </struct>
              </value>
            </param>
          </params>
        </methodResponse>"#;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("User.create"))
        .and(body_string_contains("alice@example.com"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = CreateUserParams {
        email: "alice@example.com".into(),
        login: Some("alice".into()),
        full_name: Some("Alice Example".into()),
        password: Some("hunter2".into()),
    };
    let id = client.create_user(&params).await.unwrap();
    assert_eq!(id, 4242);
}

#[tokio::test]
async fn login_and_logout_use_user_methods() {
    let mock = MockServer::start().await;
    let login = r#"<?xml version="1.0"?><methodResponse><params><param><value><struct><member><name>token</name><value><string>token-1</string></value></member></struct></value></param></params></methodResponse>"#;
    let logout = r#"<?xml version="1.0"?><methodResponse><params><param><value><struct /></value></param></params></methodResponse>"#;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("User.login"))
        .and(body_string_contains("restrict_login"))
        .respond_with(ResponseTemplate::new(200).set_body_string(login))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("User.logout"))
        .and(body_string_contains("Bugzilla_token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(logout))
        .mount(&mock)
        .await;
    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), None);
    let token = client
        .login("alice@example.test", "secret", true)
        .await
        .unwrap();
    client.logout(&token).await.unwrap();
}
