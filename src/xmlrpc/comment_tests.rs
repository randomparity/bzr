#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use super::value_to_comment;
use crate::xmlrpc::client::XmlRpcClient;
use crate::xmlrpc::value::Value;

#[test]
fn value_to_comment_parses_int_is_private() {
    let mut comment = BTreeMap::new();
    comment.insert("id".into(), Value::Int(1001));
    comment.insert("bug_id".into(), Value::Int(42));
    comment.insert("count".into(), Value::Int(1));
    comment.insert("text".into(), Value::String("private".into()));
    comment.insert("creator".into(), Value::String("alice@test".into()));
    comment.insert("is_private".into(), Value::Int(1));

    let parsed = value_to_comment(&Value::Struct(comment)).unwrap();
    assert!(parsed.is_private);
}

#[test]
fn comments_with_attachment_id_propagates_field() {
    let mut comment = BTreeMap::new();
    comment.insert("id".into(), Value::Int(1002));
    comment.insert("bug_id".into(), Value::Int(42));
    comment.insert("count".into(), Value::Int(2));
    comment.insert("text".into(), Value::String("see attachment".into()));
    comment.insert("creator".into(), Value::String("alice@test".into()));
    comment.insert("attachment_id".into(), Value::Int(99));

    let parsed = value_to_comment(&Value::Struct(comment)).unwrap();
    assert_eq!(parsed.attachment_id, Some(99));
}

#[test]
fn comments_without_attachment_id_yields_none() {
    let mut comment = BTreeMap::new();
    comment.insert("id".into(), Value::Int(1003));
    comment.insert("bug_id".into(), Value::Int(42));
    comment.insert("count".into(), Value::Int(3));
    comment.insert("text".into(), Value::String("plain comment".into()));
    comment.insert("creator".into(), Value::String("alice@test".into()));

    let parsed = value_to_comment(&Value::Struct(comment)).unwrap();
    assert_eq!(parsed.attachment_id, None);
}

#[tokio::test]
async fn xmlrpc_get_comments_since_parses_full_response() {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    let response_xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
  <member><name>bugs</name><value><struct>
    <member><name>42</name><value><struct>
      <member><name>comments</name><value><array><data>
        <value><struct>
          <member><name>id</name><value><int>1001</int></value></member>
          <member><name>bug_id</name><value><int>42</int></value></member>
          <member><name>count</name><value><int>0</int></value></member>
          <member><name>text</name><value><string>public 0</string></value></member>
          <member><name>creator</name><value><string>alice@test</string></value></member>
          <member><name>creation_time</name><value><dateTime.iso8601>20260101T00:00:00</dateTime.iso8601></value></member>
          <member><name>is_private</name><value><boolean>0</boolean></value></member>
        </struct></value>
        <value><struct>
          <member><name>id</name><value><int>1002</int></value></member>
          <member><name>bug_id</name><value><int>42</int></value></member>
          <member><name>count</name><value><int>1</int></value></member>
          <member><name>text</name><value><string>private 1</string></value></member>
          <member><name>creator</name><value><string>bob@test</string></value></member>
          <member><name>creation_time</name><value><dateTime.iso8601>20260102T00:00:00</dateTime.iso8601></value></member>
          <member><name>is_private</name><value><boolean>1</boolean></value></member>
        </struct></value>
      </data></array></value></member>
    </struct></value></member>
  </struct></value></member>
</struct></value></param></params></methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.comments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), "test-key");
    let comments = client.get_comments_since(42, None).await.unwrap();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].count, 0);
    assert!(!comments[0].is_private);
    assert_eq!(comments[1].count, 1);
    assert!(comments[1].is_private);
    assert_eq!(comments[1].text, "private 1");
}

#[tokio::test]
async fn xmlrpc_get_comments_since_serializes_new_since() {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    let empty_response = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
  <member><name>bugs</name><value><struct></struct></value></member>
</struct></value></param></params></methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.comments"))
        .and(body_string_contains("new_since"))
        .and(body_string_contains("2026-01-01T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_string(empty_response))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), "test-key");
    let comments = client
        .get_comments_since(42, Some("2026-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert!(comments.is_empty());
}
