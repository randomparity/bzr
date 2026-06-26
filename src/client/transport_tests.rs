#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::client::test_helpers::{test_client, test_client_query_param};
use crate::client::{BugzillaClientConfig, UserDetailLevel};
use crate::types::transport::{ApiMode, AuthMethod};

fn has_no_auth_header(req: &wiremock::Request) -> bool {
    !req.headers
        .contains_key(crate::bugzilla_auth::AUTH_HEADER_NAME)
}

fn has_no_auth_query_param(req: &wiremock::Request) -> bool {
    req.url
        .query_pairs()
        .all(|(name, _)| name != crate::bugzilla_auth::AUTH_QUERY_PARAM)
}

#[test]
fn safe_url_strips_query_params() {
    let url = reqwest::Url::parse(&format!(
        "https://bugzilla.example.com/rest/bug/1?{}=secret",
        crate::bugzilla_auth::AUTH_QUERY_PARAM
    ))
    .unwrap();
    let safe = BugzillaClient::safe_url(&url);
    assert!(
        !safe.contains("secret"),
        "API key should be stripped: {safe}"
    );
    assert!(
        safe.contains("/rest/bug/1"),
        "path should be preserved: {safe}"
    );
}

#[test]
fn safe_url_preserves_path() {
    let url = reqwest::Url::parse("https://bugzilla.example.com/rest/bug/42").unwrap();
    let safe = BugzillaClient::safe_url(&url);
    assert_eq!(safe, "https://bugzilla.example.com/rest/bug/42");
}

#[test]
fn apply_auth_adds_query_param_credentials() {
    let client = test_client_query_param("https://bugzilla.example.com");
    let request = client
        .apply_auth(client.http.get(client.url("bug")))
        .build()
        .unwrap();
    let expected_query = format!("{AUTH_QUERY_PARAM}=test-key");
    assert_eq!(request.url().query(), Some(expected_query.as_str()));
}

#[tokio::test]
async fn anonymous_client_sends_no_api_key_header_or_query() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = BugzillaClient::new(BugzillaClientConfig {
        base_url: &mock.uri(),
        credential: None,
        auth_method: None,
        api_mode: ApiMode::Rest,
        email_hint: None,
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();

    let value = client.get_json_value("version").await.unwrap();
    assert_eq!(value["version"], "5.1.2");

    let requests = mock.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .headers
        .get(crate::bugzilla_auth::AUTH_HEADER_NAME)
        .is_none());
    assert!(requests[0]
        .url
        .query_pairs()
        .all(|(name, _)| name != crate::bugzilla_auth::AUTH_QUERY_PARAM));
}

#[test]
fn alternate_auth_rejects_invalid_header_characters() {
    let client = BugzillaClient::new(BugzillaClientConfig {
        base_url: "https://bugzilla.example.com",
        credential: Some("bad\nkey"),
        auth_method: Some(AuthMethod::QueryParam),
        api_mode: ApiMode::Rest,
        email_hint: None,
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();

    let builder = client.http.get(client.url("bug"));
    let err = client.apply_alternate_auth(builder).unwrap_err();
    assert!(err.to_string().contains("invalid header characters"));
}

#[tokio::test]
async fn auth_fallback_header_to_query_param_on_401() {
    let mock = MockServer::start().await;
    // Success response requires query param auth (registered first)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
        .and(has_no_auth_header)
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "name": "alice@example.com"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    // First request returns 401 (registered second, checked first by LIFO)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .up_to_n_times(1)
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let users = client
        .search_users("alice", UserDetailLevel::Basic)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("alice@example.com"));
}

#[tokio::test]
async fn auth_fallback_query_param_to_header_on_401() {
    let mock = MockServer::start().await;
    // Success response requires header auth (registered first)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(wiremock::matchers::header(
            crate::bugzilla_auth::AUTH_HEADER_NAME,
            "test-key",
        ))
        .and(has_no_auth_query_param)
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 2, "name": "bob@example.com"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    // First request returns 401 (registered second, checked first by LIFO)
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .up_to_n_times(1)
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_query_param(&mock.uri());
    let users = client
        .search_users("bob", UserDetailLevel::Basic)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("bob@example.com"));
}

#[tokio::test]
async fn auth_fallback_retryable_status_uses_retry_budget() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
        .and(has_no_auth_header)
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string("slow down"),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
        .and(has_no_auth_header)
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 3, "name": "carol@example.com"}]
        })))
        .with_priority(2)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(wiremock::matchers::header(
            crate::bugzilla_auth::AUTH_HEADER_NAME,
            "test-key",
        ))
        .and(has_no_auth_query_param)
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .expect(2)
        .with_priority(3)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(1);
    let users = client
        .search_users("carol", UserDetailLevel::Basic)
        .await
        .unwrap();

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("carol@example.com"));
}

#[tokio::test]
async fn auth_fallback_both_fail_returns_original_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .search_users("anyone", UserDetailLevel::Basic)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("410") || msg.contains("log in"),
        "expected auth error: {msg}"
    );
}

#[tokio::test]
async fn anonymous_client_does_not_retry_401_with_alternate_auth() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": true,
            "code": 410,
            "message": "You must log in."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = BugzillaClient::new(BugzillaClientConfig {
        base_url: &mock.uri(),
        credential: None,
        auth_method: None,
        api_mode: ApiMode::Rest,
        email_hint: None,
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();

    let err = client
        .search_users("alice", UserDetailLevel::Basic)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("410"));
    assert_eq!(mock.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn non_401_errors_do_not_trigger_fallback() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": true,
            "code": 51,
            "message": "You are not authorized."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let err = client
        .search_users("anyone", UserDetailLevel::Basic)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not authorized"));
}

// ── Transient retry (#311) ──────────────────────────────────────────

fn bug_ok_body() -> serde_json::Value {
    serde_json::json!({ "bugs": [{ "id": 1, "summary": "ok", "status": "NEW" }] })
}

#[tokio::test]
async fn retry_recovers_after_transient_503() {
    let mock = MockServer::start().await;
    // First attempt 503, then a healthy 200. Higher priority (lower number)
    // and up_to_n_times(1) makes the 503 serve exactly once.
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bug_ok_body()))
        .with_priority(2)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(1);
    let bug = client.get_bug("1", None, None).await.unwrap();
    assert_eq!(bug.id, 1);
}

#[tokio::test]
async fn retry_recovers_after_429_with_retry_after() {
    let mock = MockServer::start().await;
    // Retry-After: 0 keeps the test fast while still exercising header parsing.
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string("slow down"),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bug_ok_body()))
        .with_priority(2)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(1);
    assert_eq!(client.get_bug("1", None, None).await.unwrap().id, 1);
}

#[tokio::test]
async fn retry_exhausted_surfaces_http_error() {
    let mock = MockServer::start().await;
    // Always 500; with retry_max=1 the endpoint is hit exactly twice
    // (initial + one retry) and the final error is returned.
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(2)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(1);
    let err = client.get_bug("1", None, None).await.unwrap_err();
    assert_eq!(err.exit_code(), 5, "exhausted retries keep HTTP exit code");
}

#[tokio::test]
async fn no_retry_on_client_error_404() {
    let mock = MockServer::start().await;
    // 404 is a caller error: it must be hit exactly once even with a budget.
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(404).set_body_string("missing"))
        .expect(1)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(3);
    assert!(client.get_bug("1", None, None).await.is_err());
}

#[tokio::test]
async fn no_retry_on_post_5xx_mutation() {
    let mock = MockServer::start().await;
    // A POST Bug.create returning 503 must NOT be retried even with a budget:
    // the create may already have been applied, so a replay could duplicate it.
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .expect(1)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(3);
    let params = crate::types::CreateBugParams::default();
    assert!(client.create_bug(&params).await.is_err());
}

#[tokio::test]
async fn no_retry_on_put_update_5xx() {
    let mock = MockServer::start().await;
    // PUT Bug.update is HTTP-idempotent but not effect-idempotent in bzr
    // (`--work-time` accumulates, `--comment` posts atomically), so a 5xx must
    // not be retried: the endpoint is hit exactly once despite the budget.
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .expect(1)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(3);
    let params = crate::types::UpdateBugParams::default();
    assert!(client.update_bug(1, &params).await.is_err());
}

#[tokio::test]
async fn no_retry_when_budget_zero() {
    let mock = MockServer::start().await;
    // Default budget is 0: a 503 is not retried (hit exactly once).
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    assert!(client.get_bug("1", None, None).await.is_err());
}
