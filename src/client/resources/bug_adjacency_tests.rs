#![expect(clippy::unwrap_used)]

use crate::client::test_helpers::test_client;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EXPECTED_FIELDS: &str = "id,summary,status,resolution,product,version,assigned_to,last_change_time,target_milestone,blocks,depends_on";

fn complete_bug(id: u64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "summary": "Strict adjacency",
        "status": "NEW",
        "resolution": "",
        "product": "TestProduct",
        "version": "unspecified",
        "assigned_to": "owner@example.invalid",
        "last_change_time": "2026-08-29T00:00:00Z",
        "target_milestone": "---",
        "blocks": [9, 8, 9],
        "depends_on": [3, 2, 3]
    })
}

#[tokio::test]
async fn bug_adjacency_rest_uses_single_query_identity_and_trailing_slash() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [complete_bug(42)],
            "faults": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let bug = client.get_bug_adjacency("42").await.unwrap().unwrap();

    assert_eq!(bug.id, 42);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let pairs = requests[0]
        .url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    assert_eq!(
        pairs,
        vec![
            ("ids".into(), "42".into()),
            ("include_fields".into(), EXPECTED_FIELDS.into()),
            ("permissive".into(), "1".into()),
        ]
    );
}

#[tokio::test]
async fn bug_adjacency_rest_keeps_slash_alias_in_query_data() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [complete_bug(55)],
            "faults": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server.uri());
    let bug = client
        .get_bug_adjacency("release/2026")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(bug.id, 55);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/rest/bug/");
    assert!(requests[0]
        .url
        .query_pairs()
        .any(|(key, value)| key == "ids" && value == "release/2026"));
}

#[tokio::test]
async fn bug_adjacency_rest_classifies_only_closed_resource_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": "101",
            "message": "Invalid Bug ID"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = test_client(&server.uri())
        .get_bug_adjacency("999999")
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(outcome, crate::types::BugAdjacencyError::NotFoundId);
}

async fn get_rest_outcome(
    requested: &str,
    body: serde_json::Value,
) -> crate::error::Result<
    std::result::Result<crate::types::BugAdjacencyBug, crate::types::BugAdjacencyError>,
> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;
    test_client(&server.uri())
        .get_bug_adjacency(requested)
        .await
}

#[tokio::test]
async fn bug_adjacency_rest_rejects_mismatched_numeric_success_identity() {
    let result = get_rest_outcome(
        "42",
        serde_json::json!({"bugs": [complete_bug(43)], "faults": []}),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));
}

#[tokio::test]
async fn bug_adjacency_rest_accepts_alias_to_canonical_success() {
    let bug = get_rest_outcome(
        "release/2026",
        serde_json::json!({"bugs": [complete_bug(55)], "faults": []}),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(bug.id, 55);
}

#[tokio::test]
async fn bug_adjacency_rest_treats_signed_text_as_alias() {
    let bug = get_rest_outcome(
        "+1",
        serde_json::json!({"bugs": [complete_bug(55)], "faults": []}),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(bug.id, 55);
}

#[tokio::test]
async fn bug_adjacency_rest_rejects_mismatched_fault_identities() {
    for (requested, fault) in [
        (
            "42",
            serde_json::json!({"id": 43, "faultCode": 101, "faultString": "missing"}),
        ),
        (
            "release/2026",
            serde_json::json!({"id": "other", "faultCode": 100, "faultString": "missing"}),
        ),
    ] {
        let result = get_rest_outcome(
            requested,
            serde_json::json!({"bugs": [], "faults": [fault]}),
        )
        .await;
        assert!(matches!(
            result,
            Err(crate::error::BzrError::DataIntegrity(_))
        ));
    }
}

#[tokio::test]
async fn bug_adjacency_rest_requires_both_adjacency_arrays() {
    for missing in ["blocks", "depends_on"] {
        let mut bug = complete_bug(42);
        bug.as_object_mut().unwrap().remove(missing);
        let result = get_rest_outcome("42", serde_json::json!({"bugs": [bug]})).await;
        assert!(matches!(
            result,
            Err(crate::error::BzrError::DataIntegrity(_))
        ));
    }
}

#[tokio::test]
async fn bug_adjacency_rest_rejects_nonexclusive_or_open_success_envelopes() {
    let fault = serde_json::json!({
        "id": 42,
        "faultCode": 101,
        "faultString": "missing"
    });
    for body in [
        serde_json::json!({"bugs": [], "faults": []}),
        serde_json::json!({"bugs": [complete_bug(42), complete_bug(42)], "faults": []}),
        serde_json::json!({"bugs": [complete_bug(42)], "faults": [fault]}),
        serde_json::json!({"bugs": [complete_bug(42)], "extra": true}),
        serde_json::json!({
            "error": true,
            "code": 101,
            "bugs": [complete_bug(42)],
            "faults": []
        }),
    ] {
        let result = get_rest_outcome("42", body).await;
        assert!(matches!(
            result,
            Err(crate::error::BzrError::DataIntegrity(_))
        ));
    }
}

#[tokio::test]
async fn bug_adjacency_rest_rejects_ids_above_signed_64_bit_range() {
    let too_large = u64::try_from(i64::MAX).unwrap() + 1;
    let result = get_rest_outcome(
        "alias",
        serde_json::json!({"bugs": [complete_bug(too_large)]}),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));

    let mut bug = complete_bug(42);
    bug["blocks"] = serde_json::json!([too_large]);
    let result = get_rest_outcome("42", serde_json::json!({"bugs": [bug]})).await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));
}

#[tokio::test]
async fn bug_adjacency_rest_normalizes_missing_and_empty_scalars_byte_equivalently() {
    let server = MockServer::start().await;
    let missing = serde_json::json!({"id": 55, "blocks": [], "depends_on": []});
    let empty = serde_json::json!({
        "id": 55,
        "summary": "",
        "status": "",
        "resolution": "",
        "product": "",
        "version": "",
        "assigned_to": "",
        "last_change_time": "",
        "target_milestone": "",
        "blocks": [],
        "depends_on": []
    });
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .and(query_param("ids", "missing-scalars"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": [missing]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .and(query_param("ids", "empty-scalars"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": [empty]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = test_client(&server.uri());
    let missing = client
        .get_bug_adjacency("missing-scalars")
        .await
        .unwrap()
        .unwrap();
    let empty = client
        .get_bug_adjacency("empty-scalars")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&missing).unwrap(),
        serde_json::to_vec(&empty).unwrap()
    );
}

#[tokio::test]
async fn bug_adjacency_hybrid_never_falls_back_to_xmlrpc() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "login required"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string("unused"))
        .expect(0)
        .mount(&server)
        .await;

    let result = crate::client::test_helpers::test_client_hybrid(&server.uri())
        .get_bug_adjacency("42")
        .await;
    assert!(result.is_err());
}

async fn assert_hybrid_rest_error_without_xmlrpc(status: u16, body: serde_json::Value) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string("unused"))
        .expect(0)
        .mount(&server)
        .await;
    assert!(
        crate::client::test_helpers::test_client_hybrid(&server.uri())
            .get_bug_adjacency("42")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn bug_adjacency_hybrid_never_falls_back_on_empty_or_internal_error() {
    assert_hybrid_rest_error_without_xmlrpc(200, serde_json::json!({"bugs": [], "faults": []}))
        .await;
    assert_hybrid_rest_error_without_xmlrpc(
        500,
        serde_json::json!({"error": true, "code": 100_500, "message": "internal"}),
    )
    .await;
}

#[tokio::test]
async fn bug_adjacency_hybrid_never_falls_back_on_transport_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(200))
                .set_body_json(serde_json::json!({"bugs": [complete_bug(42)]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string("unused"))
        .expect(0)
        .mount(&server)
        .await;
    let client = crate::client::test_helpers::test_client_hybrid_with_timeout(
        &server.uri(),
        std::time::Duration::from_millis(25),
    );
    assert!(client.get_bug_adjacency("42").await.is_err());
}

#[tokio::test]
async fn bug_adjacency_hybrid_never_follows_redirect_or_invokes_xmlrpc() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "/landed")
                .set_body_json(serde_json::json!({"bugs": [complete_bug(42)]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/landed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [complete_bug(42)]
        })))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string("unused"))
        .expect(0)
        .mount(&server)
        .await;
    assert!(
        crate::client::test_helpers::test_client_hybrid(&server.uri())
            .get_bug_adjacency("42")
            .await
            .is_err()
    );
}
