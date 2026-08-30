#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::parse_strict_response;
use crate::client::test_helpers::test_client_xmlrpc;
use crate::xmlrpc::protocol::Value;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn xml_success(id: i64, summary: Option<&str>, blocks: &[i64], depends_on: &[i64]) -> String {
    let summary = summary.map_or_else(String::new, |value| {
        format!("<member><name>summary</name><value><string>{value}</string></value></member>")
    });
    let array = |values: &[i64]| {
        values.iter().fold(String::new(), |mut output, value| {
            write!(output, "<value><int>{value}</int></value>").unwrap();
            output
        })
    };
    let blocks = array(blocks);
    let depends_on = array(depends_on);
    format!(
        r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>
          <member><name>bugs</name><value><array><data><value><struct>
            <member><name>id</name><value><int>{id}</int></value></member>
            {summary}
            <member><name>status</name><value><string>NEW</string></value></member>
            <member><name>resolution</name><value><string></string></value></member>
            <member><name>blocks</name><value><array><data>{blocks}</data></array></value></member>
            <member><name>depends_on</name><value><array><data>{depends_on}</data></array></value></member>
          </struct></value></data></array></value></member>
          <member><name>faults</name><value><array><data></data></array></value></member>
        </struct></value></param></params></methodResponse>"#
    )
}

fn xml_fault(id: &str, code: i64) -> String {
    format!(
        r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>
          <member><name>bugs</name><value><array><data></data></array></value></member>
          <member><name>faults</name><value><array><data><value><struct>
            <member><name>id</name><value><string>{id}</string></value></member>
            <member><name>faultCode</name><value><int>{code}</int></value></member>
            <member><name>faultString</name><value><string>discarded</string></value></member>
          </struct></value></data></array></value></member>
        </struct></value></param></params></methodResponse>"#
    )
}

#[tokio::test]
async fn bug_adjacency_xmlrpc_sends_one_identity_permissive_and_fixed_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<methodName>Bug.get</methodName>"))
        .and(body_string_contains("<name>ids</name>"))
        .and(body_string_contains("<string>release/2026</string>"))
        .and(body_string_contains("<name>permissive</name>"))
        .and(body_string_contains("<boolean>1</boolean>"))
        .and(body_string_contains("<name>include_fields</name>"))
        .and(body_string_contains("<string>blocks</string>"))
        .and(body_string_contains("<string>depends_on</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml_success(
            55,
            Some("Alias bug"),
            &[9, 8, 9],
            &[3, 2, 3],
        )))
        .expect(1)
        .mount(&server)
        .await;

    let bug = test_client_xmlrpc(&server.uri())
        .get_bug_adjacency("release/2026")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(bug.id, 55);
    assert_eq!(bug.blocks, vec![8, 9]);
    assert_eq!(bug.depends_on, vec![2, 3]);
    let requests = server.received_requests().await.unwrap();
    let body = std::str::from_utf8(&requests[0].body).unwrap();
    assert_eq!(body.matches("<name>ids</name>").count(), 1);
    assert_eq!(body.matches("<string>release/2026</string>").count(), 1);
}

#[tokio::test]
async fn bug_adjacency_xmlrpc_preserves_alias_fault_identity() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml_fault("missing/alias", 100)))
        .expect(1)
        .mount(&server)
        .await;

    let error = test_client_xmlrpc(&server.uri())
        .get_bug_adjacency("missing/alias")
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(error, crate::types::BugAdjacencyError::NotFoundAlias);
}

#[tokio::test]
async fn bug_adjacency_xmlrpc_treats_signed_text_as_alias() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<string>+1</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xml_success(
            55,
            Some("Alias bug"),
            &[],
            &[],
        )))
        .expect(1)
        .mount(&server)
        .await;
    let bug = test_client_xmlrpc(&server.uri())
        .get_bug_adjacency("+1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bug.id, 55);
}

fn strict_value_bug(mut bug: BTreeMap<String, Value>) -> Value {
    let mut top = BTreeMap::new();
    top.insert(
        "bugs".into(),
        Value::Array(vec![Value::Struct(std::mem::take(&mut bug))]),
    );
    top.insert("faults".into(), Value::Array(Vec::new()));
    Value::Struct(top)
}

fn minimal_value_bug(id: i64) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("id".into(), Value::Int(id)),
        ("blocks".into(), Value::Array(Vec::new())),
        ("depends_on".into(), Value::Array(Vec::new())),
    ])
}

#[test]
fn bug_adjacency_xmlrpc_rejects_mismatched_numeric_success_identity() {
    let result = parse_strict_response(&strict_value_bug(minimal_value_bug(43)), "42");
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));
}

#[test]
fn bug_adjacency_xmlrpc_requires_both_adjacency_arrays() {
    for missing in ["blocks", "depends_on"] {
        let mut bug = minimal_value_bug(42);
        bug.remove(missing);
        let result = parse_strict_response(&strict_value_bug(bug), "42");
        assert!(matches!(
            result,
            Err(crate::error::BzrError::DataIntegrity(_))
        ));
    }
}

#[test]
fn bug_adjacency_xmlrpc_rejects_nonexclusive_or_open_success_envelopes() {
    let bug = Value::Struct(minimal_value_bug(42));
    let fault = Value::Struct(BTreeMap::from([
        ("id".into(), Value::Int(42)),
        ("faultCode".into(), Value::Int(101)),
    ]));
    for top in [
        BTreeMap::from([
            ("bugs".into(), Value::Array(Vec::new())),
            ("faults".into(), Value::Array(Vec::new())),
        ]),
        BTreeMap::from([
            ("bugs".into(), Value::Array(vec![bug.clone(), bug.clone()])),
            ("faults".into(), Value::Array(Vec::new())),
        ]),
        BTreeMap::from([
            ("bugs".into(), Value::Array(vec![bug.clone()])),
            ("faults".into(), Value::Array(vec![fault])),
        ]),
        BTreeMap::from([
            ("bugs".into(), Value::Array(vec![bug])),
            ("extra".into(), Value::Bool(true)),
        ]),
    ] {
        let result = parse_strict_response(&Value::Struct(top), "42");
        assert!(matches!(
            result,
            Err(crate::error::BzrError::DataIntegrity(_))
        ));
    }
}

#[test]
fn bug_adjacency_xmlrpc_normalizes_missing_and_empty_scalars_byte_equivalently() {
    let missing = parse_strict_response(&strict_value_bug(minimal_value_bug(55)), "alias")
        .unwrap()
        .unwrap();
    let mut empty_bug = minimal_value_bug(55);
    for field in [
        "summary",
        "status",
        "resolution",
        "product",
        "version",
        "assigned_to",
        "last_change_time",
        "target_milestone",
    ] {
        empty_bug.insert(field.into(), Value::String(String::new()));
    }
    let empty = parse_strict_response(&strict_value_bug(empty_bug), "alias")
        .unwrap()
        .unwrap();

    assert_eq!(
        serde_json::to_vec(&missing).unwrap(),
        serde_json::to_vec(&empty).unwrap()
    );
}

#[tokio::test]
async fn bug_adjacency_xmlrpc_rejects_mismatched_fault_identities() {
    for (requested, response) in [
        ("42", xml_fault("43", 101)),
        ("release/2026", xml_fault("other", 100)),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(response))
            .expect(1)
            .mount(&server)
            .await;
        let result = test_client_xmlrpc(&server.uri())
            .get_bug_adjacency(requested)
            .await;
        assert!(matches!(
            result,
            Err(crate::error::BzrError::DataIntegrity(_))
        ));
    }
}

#[tokio::test]
async fn bug_adjacency_xmlrpc_non_success_is_fatal_before_body_parsing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500).set_body_string(xml_success(
            42,
            Some("unused"),
            &[],
            &[],
        )))
        .expect(1)
        .mount(&server)
        .await;
    let result = test_client_xmlrpc(&server.uri())
        .get_bug_adjacency("42")
        .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::HttpStatus { status: 500, .. })
    ));
}
