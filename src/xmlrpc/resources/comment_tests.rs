#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use super::value_to_comment;
use crate::error::BzrError;
use crate::xmlrpc::protocol::Value;
use crate::xmlrpc::protocol::XmlRpcClient;

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
    assert_eq!(parsed.is_private, Some(true));
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

#[test]
fn xmlrpc_get_comments_since_maps_absent_tags_to_empty() {
    let mut comment = BTreeMap::new();
    comment.insert("id".into(), Value::Int(1004));

    let parsed = value_to_comment(&Value::Struct(comment)).unwrap();
    assert!(parsed.tags.is_empty());
}

#[test]
fn xmlrpc_get_comments_since_maps_non_array_tags_to_empty() {
    let mut comment = BTreeMap::new();
    comment.insert("id".into(), Value::Int(1005));
    comment.insert("tags".into(), Value::String("needs-info".into()));

    let parsed = value_to_comment(&Value::Struct(comment)).unwrap();
    assert!(parsed.tags.is_empty());
}

#[test]
fn xmlrpc_get_comments_since_discards_non_string_tag_members() {
    let mut comment = BTreeMap::new();
    comment.insert("id".into(), Value::Int(1006));
    comment.insert(
        "tags".into(),
        Value::Array(vec![
            Value::String("needs-info".into()),
            Value::Int(7),
            Value::String("follow-up".into()),
        ]),
    );

    let parsed = value_to_comment(&Value::Struct(comment)).unwrap();
    assert_eq!(parsed.tags, vec!["needs-info", "follow-up"]);
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
          <member><name>tags</name><value><array><data>
            <value><string>needs-info</string></value>
            <value><string>follow-up</string></value>
          </data></array></value></member>
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

    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), Some("test-key"));
    let comments = client.get_comments_since(42, None).await.unwrap();

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].count, Some(0));
    assert_eq!(comments[0].is_private, Some(false));
    assert_eq!(comments[0].tags, vec!["needs-info", "follow-up"]);
    assert_eq!(comments[1].count, Some(1));
    assert_eq!(comments[1].is_private, Some(true));
    assert_eq!(comments[1].text.as_deref(), Some("private 1"));
}

#[tokio::test]
async fn xmlrpc_get_comments_since_rejects_bug_id_outside_xmlrpc_integer_range() {
    let client = XmlRpcClient::new(reqwest::Client::new(), "http://127.0.0.1:1", None);

    let err = client
        .get_comments_since(u64::try_from(i64::MAX).unwrap() + 1, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        BzrError::InputValidation { message: ref msg, .. }
            if msg.contains("bug ID") && msg.contains("XML-RPC signed integer range")
    ));
}

#[tokio::test]
async fn xmlrpc_get_comments_since_serializes_new_since() {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    // The bug's key is present with an empty `comments` array — a real empty
    // thread. A `bugs` map with no entry for the bug means "no record of it"
    // and is `NotFound` since #699, which would mask what this test asserts:
    // that `new_since` reaches the request body.
    let empty_response = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
  <member><name>bugs</name><value><struct>
    <member><name>42</name><value><struct>
      <member><name>comments</name><value><array><data></data></array></value></member>
    </struct></value></member>
  </struct></value></member>
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

    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), Some("test-key"));
    let comments = client
        .get_comments_since(42, Some("2026-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert!(comments.is_empty());
}

/// A response whose `bugs` map carries no entry for the requested bug is the
/// server saying it has no record of it, not that the thread is empty. Before
/// #699 this returned `Ok(vec![])`, which at N>1 dropped the bug from the flat
/// array with no header, no stderr line, and no failure entry.
#[tokio::test]
async fn xmlrpc_missing_bugs_key_is_not_found() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    let response_xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
  <member><name>bugs</name><value><struct>
    <member><name>7</name><value><struct>
      <member><name>comments</name><value><array><data></data></array></value></member>
    </struct></value></member>
  </struct></value></member>
</struct></value></param></params></methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), Some("test-key"));
    let err = client.get_comments_since(42, None).await.unwrap_err();

    assert!(
        matches!(&err, BzrError::NotFound { resource, id } if *resource == "bug" && id == "42"),
        "expected NotFound for bug 42, got: {err:?}"
    );
    // The loop in `comment list` relies on this classification to skip the bug
    // under `--permissive` instead of bailing the whole call.
    assert!(err.is_permissive_bug_view_error());
}

/// The boundary that keeps a genuinely empty thread working: Bugzilla returns
/// the key with an empty `comments` array for a bug that simply has none.
#[tokio::test]
async fn xmlrpc_present_key_with_empty_comments_is_still_ok() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    let response_xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
  <member><name>bugs</name><value><struct>
    <member><name>42</name><value><struct>
      <member><name>comments</name><value><array><data></data></array></value></member>
    </struct></value></member>
  </struct></value></member>
</struct></value></param></params></methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(reqwest::Client::new(), &mock.uri(), Some("test-key"));
    let comments = client.get_comments_since(42, None).await.unwrap();
    assert!(comments.is_empty());
}
