#![expect(clippy::unwrap_used, clippy::panic)]

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
    // First request returns 401. wiremock stable-sorts by priority and takes
    // the FIRST match, so with equal priorities the first-registered mock wins;
    // what separates these two is the auth matchers above, which the header
    // attempt fails, not registration order.
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
    // First request returns 401. wiremock stable-sorts by priority and takes
    // the FIRST match, so with equal priorities the first-registered mock wins;
    // what separates these two is the auth matchers above, which the header
    // attempt fails, not registration order.
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
async fn strict_adjacency_sends_once_without_alternate_auth_or_retry() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .and(wiremock::matchers::header(
            crate::bugzilla_auth::AUTH_HEADER_NAME,
            "test-key",
        ))
        .and(has_no_auth_query_param)
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 102
        })))
        .expect(0)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(10);
    assert!(client.get_bug_adjacency("42").await.is_err());
}

#[tokio::test]
async fn strict_adjacency_does_not_spend_transient_retry_budget() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .expect(1)
        .mount(&mock)
        .await;

    let mut client = test_client(&mock.uri());
    client.set_retry_max(10);
    assert!(client.get_bug_adjacency("42").await.is_err());
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

// ── Alternate-auth body classification (#715, ADR 0057) ─────────────

/// Mount the two halves of a 401 alternate-auth fallback on `route`. The mocks
/// share identical matchers, so ordering alone separates them: wiremock
/// stable-sorts by priority and serves the first match, and equal priorities
/// keep insertion order — so `first` is registered first and capped at one
/// serve, and the retry falls through to `retried`.
async fn mount_auth_fallback_on(
    mock: &MockServer,
    route: &str,
    first: ResponseTemplate,
    retried: ResponseTemplate,
) {
    Mock::given(method("GET"))
        .and(path(route.to_string()))
        .respond_with(first)
        .up_to_n_times(1)
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(route.to_string()))
        .respond_with(retried)
        .expect(1)
        .mount(mock)
        .await;
}

/// The common case: the fallback on `/rest/bug/1`.
async fn mount_auth_fallback(
    mock: &MockServer,
    first: ResponseTemplate,
    retried: ResponseTemplate,
) {
    mount_auth_fallback_on(mock, "/rest/bug/1", first, retried).await;
}

fn bugzilla_error(code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({"error": true, "code": code, "message": message})
}

fn login_required() -> ResponseTemplate {
    ResponseTemplate::new(401).set_body_json(bugzilla_error(410, "You must log in"))
}

#[tokio::test]
async fn auth_fallback_relays_a_policy_refusal_from_the_retried_body() {
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        login_required(),
        ResponseTemplate::new(401).set_body_json(bugzilla_error(
            120,
            "you are not allowed to restrict bugs to this group in the 'FuncTestProd' product",
        )),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, message } => {
            assert_eq!(code, 120, "the retried body's code must win");
            assert!(
                message.contains("not allowed to restrict bugs"),
                "the retried body's message must win: {message}"
            );
        }
        other => panic!("expected a relayed Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_keeps_the_original_401_when_the_retry_also_fails_to_log_in() {
    let mock = MockServer::start().await;
    // The two bodies must differ, or `Original` and `Refused` would build the
    // identical error from identical bytes and the test would pass whatever the
    // band check does. 300 is `invalid_login_or_password`.
    mount_auth_fallback(
        &mock,
        login_required(),
        ResponseTemplate::new(401).set_body_json(bugzilla_error(
            300,
            "The username or password you entered is not valid",
        )),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, message } => {
            assert_eq!(code, 410, "an authentication code must not be relayed");
            assert!(
                message.contains("must log in"),
                "the original's message must stand: {message}"
            );
        }
        other => panic!("expected the original Api 410, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_keeps_the_original_401_when_the_retry_carries_no_envelope() {
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        login_required(),
        ResponseTemplate::new(401).set_body_string("<html>Proxy Authentication Required</html>"),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 410),
        other => panic!("expected the original Api 410, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_keeps_the_original_401_when_the_retried_envelope_has_no_code() {
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        login_required(),
        ResponseTemplate::new(401)
            .set_body_json(serde_json::json!({"error": true, "message": "refused"})),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 410),
        other => panic!("expected the original Api 410, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_relays_a_403_policy_refusal() {
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        login_required(),
        ResponseTemplate::new(403).set_body_json(bugzilla_error(120, "refused by policy")),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 120),
        other => panic!("expected the relayed Api 120, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_keeps_the_original_when_a_403_retry_carries_an_auth_code() {
    // This is the case that proves FORBIDDEN belongs in the classification
    // guard. With 403 classified, the retried authentication code keeps the
    // original 410. Without it, the 403 is Replaced and `check_response_status`
    // reports the retried 300 — so the two routes disagree here, unlike the
    // relay case above, where both produce the same Api { code: 120 }.
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        login_required(),
        ResponseTemplate::new(403).set_body_json(bugzilla_error(
            300,
            "The username or password you entered is not valid",
        )),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 410, "the original 401 must stand"),
        other => panic!("expected the original Api 410, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_treats_a_retried_410_as_an_authentication_failure() {
    // Pins the `|| code == LOGIN_REQUIRED` half of the band check, which the
    // 300..=399 band does not cover. The first attempt answers 300 so the
    // expected value cannot be produced by the retried body: without the
    // LOGIN_REQUIRED clause the retried 410 is relayed and the assertion sees
    // 410 instead of 300. That is #715's own failure mode arriving from the
    // other direction — "You must log in" reported as a policy refusal.
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        ResponseTemplate::new(401).set_body_json(bugzilla_error(
            300,
            "The username or password you entered is not valid",
        )),
        ResponseTemplate::new(401).set_body_json(bugzilla_error(410, "You must log in")),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 300, "410 is an authentication code"),
        other => panic!("expected the original Api 300, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_relays_a_refusal_when_the_original_401_carried_no_envelope() {
    // The second user-visible transition recorded in ADR 0057: when the first
    // attempt's 401 carries no Bugzilla envelope — an HTML challenge page from
    // a fronting proxy, say — and the retry's does, the reported error moves
    // from `HttpStatus` (exit 5, error.type "http", structured key "status") to
    // `Api` (exit 4, error.type "api", structured key "api_code"). The retried
    // envelope is the server's real answer; the bare 401 was not.
    let mock = MockServer::start().await;
    mount_auth_fallback(
        &mock,
        ResponseTemplate::new(401).set_body_string("<html>Unauthorized</html>"),
        ResponseTemplate::new(401).set_body_json(bugzilla_error(120, "policy refusal")),
    )
    .await;

    let client = test_client(&mock.uri());
    let err = client.get_json_value("bug/1").await.unwrap_err();
    assert_eq!(
        err.exit_code(),
        4,
        "an envelope-carrying retry reports as Api"
    );
    assert_eq!(err.error_type(), "api");
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 120),
        other => panic!("expected the relayed Api 120, got {other:?}"),
    }
}

#[tokio::test]
async fn auth_fallback_band_edges_separate_login_failure_from_refusal() {
    // 300 and 399 are inside Bugzilla's documented authentication band, so the
    // original 410 stands. 299 and 400 are outside it and are relayed as
    // themselves — 400 in particular, because it is a status code, not a
    // Bugzilla error code.
    for (retried_code, expected) in [(299, 299), (300, 410), (399, 410), (400, 400)] {
        let mock = MockServer::start().await;
        mount_auth_fallback(
            &mock,
            login_required(),
            ResponseTemplate::new(401).set_body_json(bugzilla_error(retried_code, "refused")),
        )
        .await;

        let client = test_client(&mock.uri());
        let err = client.get_json_value("bug/1").await.unwrap_err();
        match err {
            BzrError::Api { code, .. } => {
                assert_eq!(code, expected, "retried code {retried_code}");
            }
            other => panic!("retried code {retried_code}: expected Api, got {other:?}"),
        }
    }
}

/// A minimal `bug view` payload — enough fields for the view formatter.
fn view_ok_bug_body(id: u64, summary: &str) -> serde_json::Value {
    serde_json::json!({
        "bugs": [{
            "id": id,
            "summary": summary,
            "status": "NEW",
            "resolution": "",
            "assigned_to": "nobody@test.com",
            "priority": "P1",
            "severity": "normal",
            "product": "TestProduct",
            "component": "General",
            "creation_time": "2025-01-01T00:00:00Z",
            "last_change_time": "2025-01-01T00:00:00Z"
        }]
    })
}

fn permissive_view_action(ids: &[&str]) -> crate::cli::BugAction {
    crate::cli::BugAction::View(crate::cli::ViewArgs {
        ids: ids.iter().map(|s| (*s).to_string()).collect(),
        permissive: true,
        web: false,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
    })
}

/// ADR 0057 Consequences: relaying the retry's `api_code` is the one place this
/// change moves a process exit code. `api_code` is control flow — code 102 is a
/// per-resource fault `--permissive` skips (`BzrError::is_permissive_bug_view_error`),
/// while the header attempt's 410 is not — so a batch that previously aborted
/// with exit 4 now completes with exit 0 and the bug listed as failed. Driving
/// the command is what makes that observable; asserting the predicate on the
/// error would never exercise the exit-0 path.
#[tokio::test]
async fn relayed_per_resource_refusal_makes_permissive_view_exit_zero_not_four() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(view_ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    mount_auth_fallback_on(
        &mock,
        "/rest/bug/2",
        login_required(),
        ResponseTemplate::new(401).set_body_json(bugzilla_error(
            102,
            "You are not authorized to access bug #2",
        )),
    )
    .await;

    let action = permissive_view_action(&["1", "2"]);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();
    assert!(
        result.is_ok(),
        "a relayed per-resource code must be suppressible: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["failed"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["failed"][0]["id"], "2");
}

/// The other direction of the same substitution, and the one that costs a user
/// something: when the ORIGINAL 401 carries a suppressible per-resource code
/// and the retry carries a non-suppressible one, a `--permissive` batch that
/// previously completed now aborts. Relaying the server's true code is the
/// decision (ADR 0057); this is its correct consequence, and it is recorded
/// rather than discovered.
#[tokio::test]
async fn relayed_non_suppressible_code_makes_permissive_view_exit_four_not_zero() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(view_ok_bug_body(1, "first")))
        .mount(&mock)
        .await;
    mount_auth_fallback_on(
        &mock,
        "/rest/bug/2",
        ResponseTemplate::new(401).set_body_json(bugzilla_error(
            102,
            "You are not authorized to access bug #2",
        )),
        ResponseTemplate::new(401).set_body_json(bugzilla_error(
            120,
            "you are not allowed to restrict bugs to this group",
        )),
    )
    .await;

    let action = permissive_view_action(&["1", "2"]);
    let mut io = crate::test_helpers::CapturedIo::new();
    let outcome = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(
            None,
            crate::types::OutputFormat::Json,
            None,
        ),
        &mut io.writers(),
    )
    .await;
    let Err(err) = outcome else {
        panic!("a relayed non-suppressible code must abort the batch");
    };
    assert_eq!(err.exit_code(), 4);
    match err {
        BzrError::Api { code, .. } => assert_eq!(code, 120),
        other => panic!("expected the relayed Api 120, got {other:?}"),
    }
}
