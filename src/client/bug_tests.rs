#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::client::test_helpers::{test_client, test_client_hybrid};

#[tokio::test]
async fn get_bug_history_returns_entries() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 42,
                "alias": [],
                "history": [
                    {
                        "who": "alice@example.com",
                        "when": "2025-01-15T10:30:00Z",
                        "changes": [
                            {
                                "field_name": "status",
                                "removed": "NEW",
                                "added": "ASSIGNED"
                            },
                            {
                                "field_name": "assigned_to",
                                "removed": "",
                                "added": "alice@example.com"
                            }
                        ]
                    },
                    {
                        "who": "bob@example.com",
                        "when": "2025-01-16T14:00:00Z",
                        "changes": [
                            {
                                "field_name": "status",
                                "removed": "ASSIGNED",
                                "added": "RESOLVED"
                            }
                        ]
                    }
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let history = client.get_bug_history_since(42, None).await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].who, "alice@example.com");
    assert_eq!(history[0].changes.len(), 2);
    assert_eq!(history[0].changes[0].field_name, "status");
    assert_eq!(history[0].changes[0].removed, "NEW");
    assert_eq!(history[0].changes[0].added, "ASSIGNED");
    assert_eq!(history[1].changes.len(), 1);
}

#[tokio::test]
async fn get_bug_history_empty() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/99/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 99, "alias": [], "history": []}]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let history = client.get_bug_history_since(99, None).await.unwrap();
    assert!(history.is_empty());
}

#[tokio::test]
async fn get_bug_history_with_attachment_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/10/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 10,
                "alias": [],
                "history": [{
                    "who": "carol@example.com",
                    "when": "2025-02-01T09:00:00Z",
                    "changes": [{
                        "field_name": "attachments.isobsolete",
                        "removed": "0",
                        "added": "1",
                        "attachment_id": 555
                    }]
                }]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let history = client.get_bug_history_since(10, None).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].changes[0].attachment_id, Some(555));
}

#[tokio::test]
async fn get_bug_history_since_filters_by_date() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/history"))
        .and(query_param("new_since", "2025-06-01T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 42,
                "alias": [],
                "history": [{
                    "who": "alice@example.com",
                    "when": "2025-06-15T10:00:00Z",
                    "changes": [{"field_name": "status", "removed": "NEW", "added": "ASSIGNED"}]
                }]
            }]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let history = client
        .get_bug_history_since(42, Some("2025-06-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn get_bug_passes_params() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .and(query_param("include_fields", "id,summary"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
        ))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client.get_bug("1", Some("id,summary"), None).await.unwrap();
    assert_eq!(bug.id, 1);
}

#[tokio::test]
async fn get_bug_default_fields_include_bug_json_fields() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .and(query_param(
            "include_fields",
            "id,summary,status,resolution,dupe_of,product,component,version,assigned_to,priority,severity,creation_time,last_change_time,creator,url,whiteboard,keywords,blocks,depends_on,cc,op_sys,rep_platform,deadline",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
        ))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client.get_bug("1", None, None).await.unwrap();

    assert_eq!(bug.id, 1);
}

#[tokio::test]
async fn get_bug_falls_back_on_100500() {
    let mock = MockServer::start().await;

    // Direct endpoint returns 100500 (server extension crash)
    Mock::given(method("GET"))
        .and(path("/rest/bug/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": BUGZILLA_INTERNAL_ERROR,
            "message": "Extension crash"
        })))
        .mount(&mock)
        .await;

    // Search endpoint returns the bug successfully
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"bugs": [{"id": 99, "summary": "fallback bug", "status": "NEW"}]}),
        ))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client.get_bug("99", None, None).await.unwrap();
    assert_eq!(bug.id, 99);
    assert_eq!(bug.summary, "fallback bug");
}

#[tokio::test]
async fn search_bugs_sends_option_fields() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("cc", "user@example.com"))
        .and(query_param("alias", "my-alias"))
        .and(query_param("summary", "crash"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": []
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        cc: Some("user@example.com".into()),
        alias: Some("my-alias".into()),
        summary: Some("crash".into()),
        limit: Some(25),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_product_filter() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "Product"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 217_630,
                "summary": "Test bug",
                "status": "WORKING",
                "product": "Product",
                "component": "Triage",
                "assigned_to": "test@example.com",
                "priority": "P1",
                "severity": "high",
                "creation_time": "2026-03-09T09:33:08Z",
                "last_change_time": "2026-03-18T05:49:05Z"
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        product: vec!["Product".into()],
        limit: Some(50),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].id, 217_630);
}

use crate::test_helpers::xmlrpc_bug_response;

#[tokio::test]
async fn hybrid_search_rest_has_results_no_xmlrpc_call() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "REST bug", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams {
        product: vec!["P".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].summary, "REST bug");
}

#[tokio::test]
async fn hybrid_search_rest_empty_with_filters_falls_back_to_xmlrpc() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(99, "XML-RPC bug")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams {
        product: vec!["P".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
    assert_eq!(bugs[0].id, 99);
    assert_eq!(bugs[0].summary, "XML-RPC bug");
}

#[tokio::test]
async fn hybrid_search_rest_empty_with_quicksearch_only_no_fallback() {
    // Regression: issue #152. quicksearch is a free-text predicate evaluated
    // by the same server-side parser for both REST and XML-RPC, so an empty
    // REST result is authoritative. The fallback used to fire here and could
    // hang for the full request timeout on servers with slow XML-RPC.
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("quicksearch", "anything"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams {
        quicksearch: Some("anything".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn hybrid_search_rest_empty_with_summary_only_no_fallback() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("summary", "kernel panic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams {
        summary: Some("kernel panic".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn hybrid_search_xmlrpc_fallback_timeout_returns_empty_rest_result() {
    // Regression: issue #152. When the structured-filter empty-fallback fires
    // but XML-RPC takes too long, we cap the wait and fall back to the empty
    // REST result rather than hanging the user.
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    // XML-RPC mock that doesn't respond before the test cap fires.
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_bug_response(42, "would-be-result"))
                .set_delay(std::time::Duration::from_secs(10)),
        )
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams {
        product: vec!["P".into()],
        ..Default::default()
    };

    let start = std::time::Instant::now();
    let bugs = client
        .search_bugs_hybrid(&params, std::time::Duration::from_millis(200))
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(bugs.is_empty(), "expected empty REST fallback");
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "fallback cap did not fire in time: elapsed={elapsed:?}"
    );
}

#[tokio::test]
async fn hybrid_search_rest_empty_without_filters_no_fallback() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams::default();
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn hybrid_get_bug_rest_500_falls_back_to_xmlrpc() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(42, "XML-RPC result")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let bug = client.get_bug("42", None, None).await.unwrap();
    assert_eq!(bug.id, 42);
    assert_eq!(bug.summary, "XML-RPC result");
}

#[tokio::test]
async fn hybrid_get_bug_rest_401_does_not_fall_back() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 102,
            "message": "Invalid API key"
        })))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let err = client.get_bug("42", None, None).await.unwrap_err();
    assert!(
        err.to_string().contains("Invalid API key"),
        "should propagate auth error, got: {err}"
    );
}

#[test]
fn has_negated_filters_detects_negation() {
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        ..Default::default()
    };
    assert!(super::has_negated_filters(&params));
}

#[test]
fn has_negated_filters_false_for_positive_only() {
    let params = SearchParams {
        status: vec!["NEW".into()],
        ..Default::default()
    };
    assert!(!super::has_negated_filters(&params));
}

#[test]
fn has_raw_boolean_chart_params_detects_f1() {
    let params = SearchParams {
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "equals".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        ..Default::default()
    };
    assert!(super::has_raw_boolean_chart_params(&params));
}

#[test]
fn has_raw_boolean_chart_params_false_for_non_chart() {
    let params = SearchParams {
        raw_params: vec![("product".into(), "Firefox".into())],
        ..Default::default()
    };
    assert!(!super::has_raw_boolean_chart_params(&params));
}

#[test]
fn has_raw_boolean_chart_params_false_for_empty() {
    let params = SearchParams::default();
    assert!(!super::has_raw_boolean_chart_params(&params));
}

#[tokio::test]
async fn search_bugs_multi_value_sends_repeated_params() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("status", "NEW"))
        .and(query_param("status", "ASSIGNED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "Bug 1", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
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
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("f1", "bug_status"))
        .and(query_param("o1", "notequals"))
        .and(query_param("v1", "CLOSED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 2, "summary": "Open bug", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_mixed_positive_and_negated() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("status", "NEW"))
        .and(query_param("f1", "bug_severity"))
        .and(query_param("o1", "notequals"))
        .and(query_param("v1", "enhancement"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 3, "summary": "Real bug", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        status: vec!["NEW".into()],
        severity: vec!["!enhancement".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_negations_in_two_fields_use_distinct_indices() {
    // Two negated values across different fields must produce
    // f1/o1/v1 and f2/o2/v2 — not collide on the same index.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("f1", "bug_status"))
        .and(query_param("o1", "notequals"))
        .and(query_param("v1", "CLOSED"))
        .and(query_param("f2", "bug_severity"))
        .and(query_param("o2", "notequals"))
        .and(query_param("v2", "enhancement"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        severity: vec!["!enhancement".into()],
        ..Default::default()
    };
    client.search_bugs(&params).await.unwrap();
}

#[test]
fn has_raw_boolean_chart_params_false_for_non_chart_letter_with_digit() {
    // "a1" matches the "letter+digit" shape but the prefix is not
    // f/o/v — must not be treated as a boolean chart parameter.
    let params = SearchParams {
        raw_params: vec![("a1".into(), "value".into())],
        ..Default::default()
    };
    assert!(!super::has_raw_boolean_chart_params(&params));
}

#[tokio::test]
async fn hybrid_search_bugs_with_raw_params_does_not_xmlrpc_fallback() {
    // Raw boolean chart params require REST and bypass the Hybrid
    // mode's empty-results XML-RPC retry: an empty REST result must
    // be returned as-is, not masked by a successful XML-RPC retry.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;
    // If the Hybrid arm were entered, it would see an empty result
    // with active filters and retry via XML-RPC, returning this
    // non-empty list and breaking the assertion below.
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(99, "xmlrpc-only")),
        )
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let params = SearchParams {
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "equals".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(
        bugs.is_empty(),
        "expected empty REST result, not XML-RPC fallback; got {bugs:?}"
    );
}

#[tokio::test]
async fn hybrid_get_bug_falls_back_on_residual_100500_error() {
    // The Hybrid arm catches a residual 100500 from get_bug_rest's
    // own retry chain (direct → search) and retries on XML-RPC.
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": BUGZILLA_INTERNAL_ERROR,
            "message": "Extension crash"
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": BUGZILLA_INTERNAL_ERROR,
            "message": "Extension crash"
        })))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_bug_response(42, "recovered via xmlrpc")),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let bug = client.get_bug("42", None, None).await.unwrap();
    assert_eq!(bug.id, 42);
    assert_eq!(bug.summary, "recovered via xmlrpc");
}

#[tokio::test]
async fn search_bugs_rejects_negated_plus_raw_boolean_chart() {
    let mock = MockServer::start().await;
    let client = test_client(&mock.uri());
    let params = SearchParams {
        status: vec!["!CLOSED".into()],
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "equals".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        ..Default::default()
    };
    let result = client.search_bugs(&params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("cannot combine negated filters"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn search_bugs_all_fields_reach_server() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "Firefox"))
        .and(query_param("component", "General"))
        .and(query_param("status", "NEW"))
        .and(query_param("assigned_to", "dev@test.com"))
        .and(query_param("creator", "reporter@test.com"))
        .and(query_param("priority", "P1"))
        .and(query_param("severity", "major"))
        .and(query_param("cc", "watcher@test.com"))
        .and(query_param("alias", "my-bug"))
        .and(query_param("id", "42"))
        .and(query_param("limit", "10"))
        .and(query_param("summary", "crash"))
        .and(query_param("quicksearch", "qs-term"))
        .and(query_param("include_fields", "id,summary"))
        .and(query_param("exclude_fields", "cc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "crash", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        product: vec!["Firefox".into()],
        component: vec!["General".into()],
        status: vec!["NEW".into()],
        assigned_to: vec!["dev@test.com".into()],
        creator: vec!["reporter@test.com".into()],
        priority: vec!["P1".into()],
        severity: vec!["major".into()],
        cc: Some("watcher@test.com".into()),
        alias: Some("my-bug".into()),
        id: vec![42],
        limit: Some(10),
        summary: Some("crash".into()),
        quicksearch: Some("qs-term".into()),
        include_fields: Some("id,summary".into()),
        exclude_fields: Some("cc".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_sends_creation_time_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("creation_time", "2026-04-01T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_last_change_time_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("last_change_time", "2026-04-15T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        last_change_time: Some("2026-04-15T00:00:00Z".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_whiteboard_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("whiteboard", "needs-review"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        whiteboard: vec!["needs-review".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_target_milestone_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("target_milestone", "5.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        target_milestone: vec!["5.0".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_version_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("version", "9.4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        version: vec!["9.4".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_op_sys_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("op_sys", "Linux"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        op_sys: vec!["Linux".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_platform_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("platform", "x86_64"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        platform: vec!["x86_64".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_resolution_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("resolution", "FIXED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        resolution: vec!["FIXED".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_qa_contact_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("qa_contact", "qa@example.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        qa_contact: vec!["qa@example.com".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_sends_url_query_param() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("url", "github.com/foo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        url: vec!["github.com/foo".into()],
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert!(bugs.is_empty());
}

#[tokio::test]
async fn search_bugs_negation_resolution_uses_notequals() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("f1", "resolution"))
        .and(query_param("o1", "notequals"))
        .and(query_param("v1", "FIXED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        resolution: vec!["!FIXED".into()],
        ..Default::default()
    };
    client.search_bugs(&params).await.unwrap();
}

#[tokio::test]
async fn search_bugs_negation_whiteboard_uses_notsubstring() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("f1", "status_whiteboard"))
        .and(query_param("o1", "notsubstring"))
        .and(query_param("v1", "wip"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        whiteboard: vec!["!wip".into()],
        ..Default::default()
    };
    client.search_bugs(&params).await.unwrap();
}

#[tokio::test]
async fn search_bugs_negation_url_uses_notsubstring() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("f1", "bug_file_loc"))
        .and(query_param("o1", "notsubstring"))
        .and(query_param("v1", "github.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        url: vec!["!github.com".into()],
        ..Default::default()
    };
    client.search_bugs(&params).await.unwrap();
}

// ── force_id_fields (F1) ─────────────────────────────────────────

#[test]
fn force_id_fields_prepends_id_to_idless_include() {
    let (inc, exc) = force_id_fields(Some("summary,status"), None);
    assert_eq!(inc.as_deref(), Some("id,summary,status"));
    assert_eq!(exc, None);
}

#[test]
fn force_id_fields_leaves_include_with_id_unchanged() {
    let (inc, _) = force_id_fields(Some("summary,id,status"), None);
    assert_eq!(inc.as_deref(), Some("summary,id,status"));
}

#[test]
fn force_id_fields_matches_id_case_insensitively() {
    let (inc, _) = force_id_fields(Some("summary,ID"), None);
    assert_eq!(inc.as_deref(), Some("summary,ID"));
}

#[test]
fn force_id_fields_trims_tokens_when_detecting_id() {
    let (inc, _) = force_id_fields(Some(" id , summary "), None);
    assert_eq!(inc.as_deref(), Some(" id , summary "));
}

#[test]
fn force_id_fields_include_none_stays_none() {
    let (inc, _) = force_id_fields(None, None);
    assert_eq!(inc, None);
}

#[test]
fn force_id_fields_strips_id_from_exclude() {
    let (_, exc) = force_id_fields(None, Some("id,status"));
    assert_eq!(exc.as_deref(), Some("status"));
}

#[test]
fn force_id_fields_exclude_only_id_becomes_none() {
    let (_, exc) = force_id_fields(None, Some("id"));
    assert_eq!(exc, None);
}

#[test]
fn force_id_fields_exclude_without_id_unchanged() {
    let (_, exc) = force_id_fields(None, Some("status,priority"));
    assert_eq!(exc.as_deref(), Some("status,priority"));
}

// ── F1: id is always fetched at the client entry points ──────────

#[tokio::test]
async fn search_bugs_prepends_id_to_idless_include_fields() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("include_fields", "id,summary,status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 7, "summary": "s", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        include_fields: Some("summary,status".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}

#[tokio::test]
async fn search_bugs_strips_id_from_exclude_fields() {
    use wiremock::matchers::query_param_is_missing;
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param_is_missing("exclude_fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        exclude_fields: Some("id".into()),
        ..Default::default()
    };
    client.search_bugs(&params).await.unwrap();
}

#[tokio::test]
async fn get_bug_prepends_id_to_idless_include_fields() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .and(query_param("include_fields", "id,summary,status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client
        .get_bug("1", Some("summary,status"), None)
        .await
        .unwrap();
    assert_eq!(bug.id, 1);
}

#[tokio::test]
async fn get_bug_via_search_fallback_carries_id() {
    let mock = MockServer::start().await;
    // Direct endpoint crashes with 100500, forcing the search fallback.
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": BUGZILLA_INTERNAL_ERROR,
            "message": "Extension crash"
        })))
        .mount(&mock)
        .await;
    // The search fallback must request id even though the caller omitted it.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("id", "1"))
        .and(query_param("include_fields", "id,summary,status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let bug = client
        .get_bug("1", Some("summary,status"), None)
        .await
        .unwrap();
    assert_eq!(bug.id, 1);
}

#[tokio::test]
async fn xmlrpc_search_idless_include_fields_carries_id() {
    use crate::client::test_helpers::test_client_xmlrpc;
    use wiremock::matchers::body_string_contains;
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("<name>include_fields</name>"))
        .and(body_string_contains("<string>id</string>"))
        .and(body_string_contains("<string>summary</string>"))
        .and(body_string_contains("<string>status</string>"))
        .respond_with(ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(5, "x")))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_xmlrpc(&mock.uri());
    let params = SearchParams {
        include_fields: Some("summary,status".into()),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await.unwrap();
    assert_eq!(bugs.len(), 1);
}
