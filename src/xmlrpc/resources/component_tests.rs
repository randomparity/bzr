#![expect(clippy::unwrap_used)]

use super::XmlRpcClient;
use crate::types::component::UpdateComponentParams;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn component_update_uses_rhbz_xmlrpc_request_shape() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<methodName>Component.update</methodName>"))
        .and(body_string_contains("<name>names</name>"))
        .and(body_string_contains("<name>product</name>"))
        .and(body_string_contains("<string>Widget</string>"))
        .and(body_string_contains("<name>component</name>"))
        .and(body_string_contains("<string>Core</string>"))
        .and(body_string_contains("<name>default_assignee</name>"))
        .and(body_string_contains("<name>is_active</name>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0"?><methodResponse><params><param><value><struct/></value></param></params></methodResponse>"#,
        ))
        .expect(1)
        .mount(&mock)
        .await;
    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), Some("test-key"));
    client
        .update_component(UpdateComponentParams {
            product: "Widget",
            component: "Core",
            description: Some("Updated"),
            default_assignee: Some("owner@example.test"),
            is_active: Some(false),
        })
        .await
        .unwrap();
}
