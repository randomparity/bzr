#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use super::{add_vec_filters, extract_bugs, value_to_bug};
use crate::error::BzrError;
use crate::test_helpers::xmlrpc_bug_response;
use crate::types::SearchParams;
use crate::xmlrpc::protocol::Value;
use crate::xmlrpc::protocol::XmlRpcClient;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn search_bugs_returns_results() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains(
            "<name>offset</name><value><int>7</int></value>",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(42, "Test bug")),
        )
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        product: vec!["TestProduct".into()],
        limit: Some(10),
        offset: Some(7),
        ..Default::default()
    };

    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].id, 42);
    assert_eq!(bugs[0].summary.as_deref(), Some("Test bug"));
    assert_eq!(bugs[0].status.as_deref(), Some("NEW"));
    assert_eq!(bugs[0].product.as_deref(), Some("TestProduct"));

    let requests = mock.received_requests().await.unwrap();
    let body = String::from_utf8(requests[0].body.clone()).unwrap();
    assert!(body.contains("<name>include_fields</name>"));
    assert!(body.contains("<string>target_milestone</string>"));
    for view_only in ["groups", "estimated_time", "remaining_time"] {
        assert!(
            !body.contains(&format!("<string>{view_only}</string>")),
            "default search should not request view-only field {view_only}"
        );
    }
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        product: vec!["Empty".into()],
        ..Default::default()
    };

    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_rejects_id_outside_xmlrpc_integer_range() {
    let client = XmlRpcClient::new(test_http_client(), "http://127.0.0.1:1", None);
    let params = SearchParams {
        id: vec![u64::try_from(i64::MAX).unwrap() + 1],
        ..Default::default()
    };

    let err = client.search_bugs(&params).await.unwrap_err();

    assert!(matches!(
        err,
        BzrError::InputValidation { message: ref msg, .. }
            if msg.contains("bug ID") && msg.contains("XML-RPC signed integer range")
    ));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let bug = client.get_bug("100").await.unwrap();
    assert_eq!(bug.id, 100);
    assert_eq!(bug.summary.as_deref(), Some("Specific bug"));
}

#[tokio::test]
async fn get_bug_by_id_parses_dupe_of() {
    let mock = MockServer::start().await;
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <methodResponse><params><param><value><struct>
          <member><name>bugs</name><value><array><data>
            <value><struct>
              <member><name>id</name><value><int>100</int></value></member>
              <member><name>summary</name><value><string>Duplicate bug</string></value></member>
              <member><name>status</name><value><string>RESOLVED</string></value></member>
              <member><name>resolution</name><value><string>DUPLICATE</string></value></member>
              <member><name>dupe_of</name><value><int>99</int></value></member>
            </struct></value>
          </data></array></value></member>
        </struct></value></param></params></methodResponse>"#;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let bug = client.get_bug("100").await.unwrap();

    assert_eq!(bug.dupe_of, Some(99));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let bug = client.get_bug("my-alias").await.unwrap();
    assert_eq!(bug.id, 55);
    assert_eq!(bug.summary.as_deref(), Some("Alias bug"));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_role_negations_use_nowordssubstr() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<string>assigned_to</string>"))
        .and(body_string_contains("<string>reporter</string>"))
        .and(body_string_contains("<string>qa_contact</string>"))
        .and(body_string_contains("<string>nowordssubstr</string>"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(2, "Role bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        assigned_to: vec!["!alice".into()],
        creator: vec!["!bob".into()],
        qa_contact: vec!["!carol".into()],
        ..Default::default()
    };
    client.search_bugs(&params).await.unwrap();
}

#[tokio::test]
async fn search_bugs_xmlrpc_rejects_zero_word_role_negation_before_request() {
    let mock = MockServer::start().await;
    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        assigned_to: vec!["! , \t".into()],
        ..Default::default()
    };

    let err = client.search_bugs(&params).await.unwrap_err();
    assert!(matches!(err, BzrError::InputValidation { .. }));
    assert!(mock.received_requests().await.unwrap().is_empty());
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
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
fn extract_bugs_rejects_non_array_payload() {
    let mut payload = BTreeMap::new();
    payload.insert("bugs".into(), Value::String("wrong".into()));
    let err = extract_bugs(&Value::Struct(payload)).unwrap_err();
    assert!(err.to_string().contains("expected bugs array"));
}

#[test]
fn value_to_bug_captures_custom_fields() {
    let mut payload = BTreeMap::new();
    payload.insert("id".into(), Value::Int(42));
    payload.insert("summary".into(), Value::String("custom".into()));
    payload.insert("cf_release".into(), Value::String("9.6".into()));
    payload.insert("x_extension".into(), Value::String("ignored".into()));

    let bug = value_to_bug(&Value::Struct(payload)).unwrap();

    assert_eq!(bug.custom_fields["cf_release"], serde_json::json!("9.6"));
    assert!(!bug.custom_fields.contains_key("x_extension"));
}

#[test]
fn value_to_bug_captures_groups_and_time_tracking_fields() {
    let mut payload = BTreeMap::new();
    payload.insert("id".into(), Value::Int(42));
    payload.insert(
        "groups".into(),
        Value::Array(vec![Value::String("functest-grp".into())]),
    );
    payload.insert("estimated_time".into(), Value::Double(8.0));
    payload.insert("remaining_time".into(), Value::Double(5.0));

    let bug = value_to_bug(&Value::Struct(payload)).unwrap();

    assert_eq!(bug.groups, vec!["functest-grp"]);
    assert_eq!(bug.estimated_time, Some(8.0));
    assert_eq!(bug.remaining_time, Some(5.0));
}

#[test]
fn value_to_bug_rejects_malformed_group_and_time_tracking_fields() {
    let malformed = [
        ("groups", Value::String("functest-grp".into())),
        (
            "groups",
            Value::Array(vec![Value::String("functest-grp".into()), Value::Int(7)]),
        ),
        ("estimated_time", Value::String("8".into())),
        ("remaining_time", Value::Int(5)),
        ("estimated_time", Value::Double(f64::NAN)),
        ("remaining_time", Value::Double(f64::INFINITY)),
    ];

    for (field, value) in malformed {
        let mut payload = BTreeMap::new();
        payload.insert("id".into(), Value::Int(42));
        payload.insert(field.into(), value);

        let result = value_to_bug(&Value::Struct(payload));

        assert!(
            matches!(&result, Err(BzrError::XmlRpc(message)) if message.contains(field)),
            "malformed {field} should name the field: {result:?}"
        );
    }
}

#[test]
fn value_to_bug_converts_custom_field_arrays() {
    let mut payload = BTreeMap::new();
    payload.insert("id".into(), Value::Int(42));
    payload.insert(
        "cf_targets".into(),
        Value::Array(vec![
            Value::String("9.6".into()),
            Value::String("9.7".into()),
        ]),
    );

    let bug = value_to_bug(&Value::Struct(payload)).unwrap();

    assert_eq!(
        bug.custom_fields["cf_targets"],
        serde_json::json!(["9.6", "9.7"])
    );
}

#[test]
fn value_to_bug_converts_custom_field_scalars_without_failing() {
    let mut payload = BTreeMap::new();
    payload.insert("id".into(), Value::Int(42));
    payload.insert("cf_score".into(), Value::Double(12.5));
    payload.insert("cf_bad_score".into(), Value::Double(f64::INFINITY));
    payload.insert("cf_data".into(), Value::Base64(vec![1, 2, 3]));

    let bug = value_to_bug(&Value::Struct(payload)).unwrap();

    assert_eq!(bug.custom_fields["cf_score"], serde_json::json!(12.5));
    assert_eq!(bug.custom_fields["cf_bad_score"], serde_json::json!("inf"));
    assert_eq!(bug.custom_fields["cf_data"], serde_json::json!("AQID"));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        last_change_time: Some("2026-04-15T00:00:00Z".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
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

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        whiteboard: vec!["!wip".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_target_milestone() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>target_milestone</name>"))
        .and(body_string_contains("<string>5.0</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "TM bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        target_milestone: vec!["5.0".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_version() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>version</name>"))
        .and(body_string_contains("<string>9.4</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "Ver bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        version: vec!["9.4".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_op_sys() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>op_sys</name>"))
        .and(body_string_contains("<string>Linux</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "OS bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        op_sys: vec!["Linux".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_platform() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>platform</name>"))
        .and(body_string_contains("<string>x86_64</string>"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "Plat bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        platform: vec!["x86_64".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_qa_contact() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>qa_contact</name>"))
        .and(body_string_contains("<string>qa@example.com</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "QA bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        qa_contact: vec!["qa@example.com".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_sends_url() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>url</name>"))
        .and(body_string_contains("<string>github.com/foo</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "URL bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        url: vec!["github.com/foo".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_xmlrpc_negation_resolution_uses_notequals() {
    // Boolean chart on the XML-RPC path uses fN/oN/vN keys with
    // string values. For exact-match fields the operator must be
    // `notequals`, not `notsubstring`.
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<string>resolution</string>"))
        .and(body_string_contains("<string>notequals</string>"))
        .and(body_string_contains("<string>FIXED</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "Res bug")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let params = SearchParams {
        resolution: vec!["!FIXED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}
