#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

fn test_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

use crate::test_helpers::xmlrpc_bug_response;

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

/// Wrap an inner array of `<value><struct>...</struct></value>` items in
/// the standard `bugs -> {bug_id} -> array` XML-RPC response envelope.
fn xmlrpc_bugs_envelope(bug_id: u64, inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>bugs</name><value><struct>\
                <member><name>{bug_id}</name><value><array><data>{inner}</data></array></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

/// Wrap inner `<member>...</member>` entries in the
/// `attachments -> {attachment_id}` XML-RPC response envelope used by
/// `Bug.attachments` when called with `attachment_ids`.
fn xmlrpc_attachments_keyed_envelope(inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>attachments</name><value><struct>{inner}</struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

#[tokio::test]
async fn search_bugs_returns_results() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(42, "Test bug")),
        )
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        product: vec!["TestProduct".into()],
        limit: Some(10),
        ..Default::default()
    };

    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].id, 42);
    assert_eq!(bugs[0].summary, "Test bug");
    assert_eq!(bugs[0].status, "NEW");
    assert_eq!(bugs[0].product.as_deref(), Some("TestProduct"));
}

#[tokio::test]
async fn search_bugs_empty_result() {
    let mock = MockServer::start().await;
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <methodResponse>
          <params>
            <param>
              <value>
                <struct>
                  <member>
                    <name>bugs</name>
                    <value><array><data></data></array></value>
                  </member>
                </struct>
              </value>
            </param>
          </params>
        </methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        product: vec!["Empty".into()],
        ..Default::default()
    };

    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn get_bug_by_id() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(100, "Specific bug")),
        )
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let bug = client.get_bug("100").await.unwrap();
    assert_eq!(bug.id, 100);
    assert_eq!(bug.summary, "Specific bug");
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
async fn get_bug_by_alias() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(55, "Alias bug")),
        )
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let bug = client.get_bug("my-alias").await.unwrap();
    assert_eq!(bug.id, 55);
    assert_eq!(bug.summary, "Alias bug");
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

#[tokio::test]
async fn search_bugs_multi_value_sends_array() {
    let mock = MockServer::start().await;
    // Verify the XML body contains both status values as array members
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<string>NEW</string>"))
        .and(body_string_contains("<string>ASSIGNED</string>"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "Multi bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        status: vec!["NEW".into(), "ASSIGNED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_negation_sends_boolean_chart() {
    let mock = MockServer::start().await;
    // Verify the XML body contains boolean chart params for negation
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<string>bug_status</string>"))
        .and(body_string_contains("<string>notequals</string>"))
        .and(body_string_contains("<string>CLOSED</string>"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(2, "Open bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_fields_and_ids_use_xmlrpc_arrays() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>ids</name>"))
        .and(body_string_contains("<int>42</int>"))
        .and(body_string_contains("<name>include_fields</name>"))
        .and(body_string_contains("<string>id</string>"))
        .and(body_string_contains("<string>summary</string>"))
        .and(body_string_contains("<name>exclude_fields</name>"))
        .and(body_string_contains("<string>cc</string>"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(42, "Field bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        id: vec![42],
        include_fields: Some("id, summary".into()),
        exclude_fields: Some("cc".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].id, 42);
}

#[tokio::test]
async fn get_bug_empty_result_is_not_found() {
    let mock = MockServer::start().await;
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <methodResponse>
          <params>
            <param>
              <value>
                <struct>
                  <member>
                    <name>bugs</name>
                    <value><array><data></data></array></value>
                  </member>
                </struct>
              </value>
            </param>
          </params>
        </methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let err = client.get_bug("42").await.unwrap_err();
    assert!(matches!(
        err,
        BzrError::NotFound {
            resource: "bug",
            ..
        }
    ));
}

#[test]
fn extract_id_requires_struct_with_integer_id() {
    let err = extract_id(&Value::String("oops".into())).unwrap_err();
    assert!(err.to_string().contains("expected struct response"));

    let err = extract_id(&Value::Struct(BTreeMap::new())).unwrap_err();
    assert!(err.to_string().contains("missing id field"));
}

#[test]
fn extract_bugs_rejects_non_array_payload() {
    let mut payload = BTreeMap::new();
    payload.insert("bugs".into(), Value::String("wrong".into()));
    let err = extract_bugs(&Value::Struct(payload)).unwrap_err();
    assert!(err.to_string().contains("expected bugs array"));
}

#[test]
fn value_to_group_info_parses_membership_and_optional_fields() {
    let mut member = BTreeMap::new();
    member.insert("id".into(), Value::Int(7));
    member.insert("name".into(), Value::String("alice@example.com".into()));
    member.insert("real_name".into(), Value::String("Alice".into()));
    member.insert("email".into(), Value::String("alice@example.com".into()));

    let mut group = BTreeMap::new();
    group.insert("id".into(), Value::Int(1));
    group.insert("name".into(), Value::String("admin".into()));
    group.insert("description".into(), Value::String("Administrators".into()));
    group.insert("is_active".into(), Value::Bool(true));
    group.insert(
        "membership".into(),
        Value::Array(vec![Value::Struct(member)]),
    );

    let info = value_to_group_info(&Value::Struct(group)).unwrap();
    assert_eq!(info.name, "admin");
    assert!(info.is_active);
    assert_eq!(info.membership.len(), 1);
    assert_eq!(info.membership[0].id, 7);
    assert_eq!(info.membership[0].real_name.as_deref(), Some("Alice"));
}

#[test]
fn value_to_group_info_parses_int_is_active() {
    let mut group = BTreeMap::new();
    group.insert("id".into(), Value::Int(1));
    group.insert("name".into(), Value::String("admin".into()));
    group.insert("description".into(), Value::String("Administrators".into()));
    group.insert("is_active".into(), Value::Int(1));
    group.insert("membership".into(), Value::Array(Vec::new()));

    let info = value_to_group_info(&Value::Struct(group)).unwrap();
    assert!(info.is_active);
}

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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = CreateUserParams {
        email: "alice@example.com".into(),
        login: Some("alice".into()),
        full_name: Some("Alice Example".into()),
        password: Some("hunter2".into()),
    };
    let id = client.create_user(&params).await.unwrap();
    assert_eq!(id, 4242);
}

#[test]
fn add_vec_filters_increments_chart_index_per_negation() {
    let params = SearchParams {
        product: vec!["!Bad".into(), "!Worse".into()],
        ..Default::default()
    };
    let mut rpc = BTreeMap::new();
    add_vec_filters(&mut rpc, &params);

    assert_eq!(rpc.get("f1").and_then(Value::as_str), Some("product"));
    assert_eq!(rpc.get("o1").and_then(Value::as_str), Some("notequals"));
    assert_eq!(rpc.get("v1").and_then(Value::as_str), Some("Bad"));
    assert_eq!(rpc.get("f2").and_then(Value::as_str), Some("product"));
    assert_eq!(rpc.get("o2").and_then(Value::as_str), Some("notequals"));
    assert_eq!(rpc.get("v2").and_then(Value::as_str), Some("Worse"));
}

#[test]
fn get_nonempty_str_filters_empty_and_non_string() {
    let mut m = BTreeMap::new();
    m.insert("empty".into(), Value::String(String::new()));
    m.insert("filled".into(), Value::String("x".into()));
    m.insert("not_string".into(), Value::Int(5));
    assert!(get_nonempty_str(&m, "empty").is_none());
    assert_eq!(get_nonempty_str(&m, "filled").as_deref(), Some("x"));
    assert!(get_nonempty_str(&m, "not_string").is_none());
    assert!(get_nonempty_str(&m, "missing").is_none());
}

#[test]
fn get_datetime_str_covers_datetime_string_and_fallthrough() {
    let mut m = BTreeMap::new();
    m.insert("dt".into(), Value::DateTime("2024-01-01T00:00:00".into()));
    m.insert("s_full".into(), Value::String("2024-02-02".into()));
    m.insert("s_empty".into(), Value::String(String::new()));
    m.insert("other".into(), Value::Int(42));
    assert_eq!(
        get_datetime_str(&m, "dt").as_deref(),
        Some("2024-01-01T00:00:00")
    );
    assert_eq!(
        get_datetime_str(&m, "s_full").as_deref(),
        Some("2024-02-02")
    );
    assert!(get_datetime_str(&m, "s_empty").is_none());
    assert!(get_datetime_str(&m, "other").is_none());
    assert!(get_datetime_str(&m, "missing").is_none());
}

#[test]
fn get_str_array_returns_strings_only() {
    let mut m = BTreeMap::new();
    m.insert(
        "tags".into(),
        Value::Array(vec![
            Value::String("alpha".into()),
            Value::String("beta".into()),
            Value::Int(99),
        ]),
    );
    m.insert("not_array".into(), Value::String("oops".into()));
    assert_eq!(
        get_str_array(&m, "tags"),
        vec!["alpha".to_string(), "beta".to_string()]
    );
    assert!(get_str_array(&m, "not_array").is_empty());
    assert!(get_str_array(&m, "missing").is_empty());
}

#[test]
fn get_int_array_returns_ints_only() {
    let mut m = BTreeMap::new();
    m.insert(
        "blocks".into(),
        Value::Array(vec![
            Value::Int(42),
            Value::Int(100),
            Value::String("nope".into()),
        ]),
    );
    m.insert("not_array".into(), Value::Int(5));
    assert_eq!(get_int_array(&m, "blocks"), vec![42_u64, 100]);
    assert!(get_int_array(&m, "not_array").is_empty());
    assert!(get_int_array(&m, "missing").is_empty());
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

#[tokio::test]
async fn xmlrpc_get_attachments_parses_full_response() {
    let mock = MockServer::start().await;
    let inner = "\
        <value><struct>\
            <member><name>id</name><value><int>2001</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>public.txt</string></value></member>\
            <member><name>summary</name><value><string>public file</string></value></member>\
            <member><name>content_type</name><value><string>text/plain</string></value></member>\
            <member><name>creator</name><value><string>alice@test</string></value></member>\
            <member><name>creation_time</name><value><dateTime.iso8601>20260101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>last_change_time</name><value><dateTime.iso8601>20260101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>size</name><value><int>11</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>0</int></value></member>\
            <member><name>data</name><value><base64>aGVsbG8gd29ybGQK</base64></value></member>\
        </struct></value>\
        <value><struct>\
            <member><name>id</name><value><int>2002</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>private.bin</string></value></member>\
            <member><name>summary</name><value><string>private file</string></value></member>\
            <member><name>content_type</name><value><string>application/octet-stream</string></value></member>\
            <member><name>creator</name><value><string>bob@test</string></value></member>\
            <member><name>creation_time</name><value><dateTime.iso8601>20260102T00:00:00</dateTime.iso8601></value></member>\
            <member><name>last_change_time</name><value><dateTime.iso8601>20260102T00:00:00</dateTime.iso8601></value></member>\
            <member><name>size</name><value><int>4</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>1</int></value></member>\
            <member><name>data</name><value><base64>YmVlZg==</base64></value></member>\
        </struct></value>";
    let response_xml = xmlrpc_bugs_envelope(42, inner);

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let attachments = client.get_attachments(42).await.unwrap();

    assert_eq!(attachments.len(), 2);
    assert_eq!(attachments[0].id, 2001);
    assert_eq!(attachments[0].file_name, "public.txt");
    assert!(!attachments[0].is_private);
    assert_eq!(attachments[0].size, 11);
    assert_eq!(attachments[0].data.as_deref(), Some("aGVsbG8gd29ybGQK"));
    assert_eq!(attachments[1].id, 2002);
    assert_eq!(attachments[1].file_name, "private.bin");
    assert!(attachments[1].is_private);
    assert_eq!(attachments[1].data.as_deref(), Some("YmVlZg=="));
}

#[tokio::test]
async fn xmlrpc_get_attachments_excludes_data_field_from_request() {
    // Stock Bugzilla 5.0 REST `attachment list` omits the base64 `data`
    // field; the XML-RPC list path must match by passing
    // `exclude_fields: ["data"]` so we don't inline multi-MB payloads
    // in `bzr attachment list` output. Download paths get data via
    // get_attachment_by_id, which does not exclude it.
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_bugs_envelope(42, "");

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .and(body_string_contains("exclude_fields"))
        .and(body_string_contains("data"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let _ = client.get_attachments(42).await.unwrap();
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_request_body_omits_exclude_fields() {
    use wiremock::matchers::body_string_contains;
    let mock = MockServer::start().await;

    // If the request carried exclude_fields, the download path would
    // get data:None and fail. Mock will only match a request that
    // does NOT contain "exclude_fields" via a custom matcher.
    let response_xml = xmlrpc_attachments_keyed_envelope(
        "<member><name>9</name><value><struct>\
            <member><name>id</name><value><int>9</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>y.bin</string></value></member>\
            <member><name>summary</name><value><string>y</string></value></member>\
            <member><name>content_type</name><value><string>application/octet-stream</string></value></member>\
            <member><name>size</name><value><int>2</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>0</int></value></member>\
            <member><name>data</name><value><base64>YmU=</base64></value></member>\
        </struct></value></member>",
    );
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("attachment_ids"))
        .and(NotBodyContains("exclude_fields"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let attachment = client.get_attachment_by_id(9).await.unwrap();
    assert_eq!(attachment.data.as_deref(), Some("YmU="));
}

#[tokio::test]
async fn search_bugs_sends_creation_time_and_last_change_time() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>creation_time</name>"))
        .and(body_string_contains(
            "<string>2026-04-01T00:00:00Z</string>",
        ))
        .and(body_string_contains("<name>last_change_time</name>"))
        .and(body_string_contains(
            "<string>2026-04-15T00:00:00Z</string>",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(7, "Date-filtered bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        last_change_time: Some("2026-04-15T00:00:00Z".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

/// Custom wiremock matcher: body must NOT contain the given substring.
struct NotBodyContains(&'static str);

impl wiremock::Match for NotBodyContains {
    fn matches(&self, request: &wiremock::Request) -> bool {
        !std::str::from_utf8(&request.body)
            .map(|s| s.contains(self.0))
            .unwrap_or(false)
    }
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_parses_response() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_attachments_keyed_envelope(
        "<member><name>2002</name><value><struct>\
            <member><name>id</name><value><int>2002</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>private.bin</string></value></member>\
            <member><name>summary</name><value><string>private file</string></value></member>\
            <member><name>content_type</name><value><string>application/octet-stream</string></value></member>\
            <member><name>size</name><value><int>4</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>1</int></value></member>\
            <member><name>data</name><value><base64>YmVlZg==</base64></value></member>\
        </struct></value></member>",
    );
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .and(body_string_contains("attachment_ids"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let attachment = client.get_attachment_by_id(2002).await.unwrap();
    assert_eq!(attachment.id, 2002);
    assert!(attachment.is_private);
    assert_eq!(attachment.data.as_deref(), Some("YmVlZg=="));
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_not_found_returns_error() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_attachments_keyed_envelope("");
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let err = client.get_attachment_by_id(9999).await.unwrap_err();
    assert!(matches!(
        err,
        BzrError::NotFound {
            resource: "attachment",
            ..
        }
    ));
}

#[tokio::test]
async fn xmlrpc_get_attachments_returns_empty_when_bug_has_none() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_bugs_envelope(42, "");
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let attachments = client.get_attachments(42).await.unwrap();
    assert!(attachments.is_empty());
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_whiteboard() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>whiteboard</name>"))
        .and(body_string_contains("<string>needs-review</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "WB bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        whiteboard: vec!["needs-review".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_resolution() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>resolution</name>"))
        .and(body_string_contains("<string>FIXED</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "Res bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        resolution: vec!["FIXED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_negation_whiteboard_uses_notsubstring() {
    // Boolean chart on the XML-RPC path uses fN/oN/vN keys with
    // string values. For substring fields the operator must be
    // `notsubstring`, not `notequals`.
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<string>status_whiteboard</string>"))
        .and(body_string_contains("<string>notsubstring</string>"))
        .and(body_string_contains("<string>wip</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "WB bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
    let params = SearchParams {
        whiteboard: vec!["!wip".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}
