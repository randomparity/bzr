#![expect(clippy::unwrap_used)]

use super::*;
use crate::client::PreparedAuth;
use crate::error::BzrError;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn strict_http_client() -> reqwest::Client {
    crate::tls::build_no_redirect_tls_client(
        &crate::tls::TlsConfig::default(),
        crate::http::REQUEST_TIMEOUT,
    )
    .unwrap()
}

/// Mount the three `rest/user` responses the header-auth probe distinguishes.
///
/// Mount order is load-bearing: wiremock sorts by priority (default 5 for every
/// mock) and breaks ties by insertion order, so the credential-matching mocks
/// must precede the catch-all that answers the anonymous leg.
async fn mount_user_legs(
    server: &MockServer,
    header_leg: ResponseTemplate,
    query_leg: ResponseTemplate,
    anonymous_leg: ResponseTemplate,
) {
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(header_leg)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(query_leg)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(anonymous_leg)
        .mount(server)
        .await;
}

/// The projection Bugzilla returns to an anonymous caller.
fn thin_user() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"users": [{"id": 1, "real_name": "T"}]}))
}

/// The projection Bugzilla returns to an authenticated caller.
fn rich_user() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"users": [{"id": 1, "real_name": "T", "groups": ["g"]}]}))
}

fn bugzilla_error(status: u16) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(
        serde_json::json!({"error": true, "code": 410, "message": "You must log in"}),
    )
}

async fn header_auth_confirmed(server: &MockServer) -> bool {
    verify_header_auth_via_rest(
        &strict_http_client(),
        &server.uri(),
        "test-key",
        &HeaderValue::from_static("test-key"),
        "user@example.com",
    )
    .await
}

async fn requests_received(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

#[tokio::test]
async fn header_ignored_is_not_confirmed() {
    // The #713 defect: a header-unaware server answers the header leg exactly as
    // it answers an anonymous caller.
    let server = MockServer::start().await;
    mount_user_legs(&server, thin_user(), rich_user(), thin_user()).await;

    assert!(!header_auth_confirmed(&server).await);
    assert_eq!(
        requests_received(&server).await,
        2,
        "probe should stop after the anonymous leg matched the header leg"
    );
}

#[tokio::test]
async fn header_honoured_is_confirmed() {
    let server = MockServer::start().await;
    mount_user_legs(&server, rich_user(), rich_user(), thin_user()).await;

    assert!(header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn anonymous_refusal_is_confirmed() {
    // A `requirelogin` server refuses the anonymous caller with a status *and* a
    // Bugzilla error body together. That refusal is the discrimination the probe
    // is looking for, so the anonymous leg is exempt from the credentialed-leg
    // error test. A bodiless 401 would pass either way and prove nothing.
    let server = MockServer::start().await;
    mount_user_legs(&server, rich_user(), rich_user(), bugzilla_error(401)).await;

    assert!(header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn non_discriminating_endpoint_is_not_confirmed() {
    let server = MockServer::start().await;
    mount_user_legs(&server, thin_user(), thin_user(), thin_user()).await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn header_leg_non_success_is_not_confirmed() {
    // The header body deliberately equals the query-param body, so the
    // credential-accepted guard is the only thing preventing a confirmation.
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        ResponseTemplate::new(401).set_body_json(
            serde_json::json!({"users": [{"id": 1, "real_name": "T", "groups": ["g"]}]}),
        ),
        rich_user(),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
    assert_eq!(
        requests_received(&server).await,
        1,
        "a refused header leg should end the probe before the anonymous leg"
    );
}

#[tokio::test]
async fn query_leg_non_success_is_not_confirmed() {
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        rich_user(),
        ResponseTemplate::new(403).set_body_json(
            serde_json::json!({"users": [{"id": 1, "real_name": "T", "groups": ["g"]}]}),
        ),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn anonymous_leg_failure_is_not_confirmed() {
    // A transient anonymous failure differs from the header response for a
    // reason unrelated to auth; without the conclusive-status rule the matching
    // query-param leg would then confirm on the strength of an error.
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        thin_user(),
        thin_user(),
        ResponseTemplate::new(503),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn error_body_leg_is_not_confirmed() {
    // Bugzilla delivers some errors inside an HTTP 200. Two credentialed legs
    // carrying the same 200 error must not read as agreement.
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        bugzilla_error(200),
        bugzilla_error(200),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn differing_key_order_still_confirms() {
    // Bugzilla randomises JSON object key order per response -- measured on the
    // project's bz50 image, where three identical authenticated requests returned
    // three different byte sequences and one identical value. This is the bite
    // check for comparing parsed values rather than bytes: under a byte
    // comparison the credentialed legs never match and the probe is permanently
    // negative. Raw strings, not `set_body_json`, so the key order survives.
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        ResponseTemplate::new(200)
            .set_body_string(r#"{"users":[{"id":1,"real_name":"T","groups":["g"]}]}"#),
        ResponseTemplate::new(200)
            .set_body_string(r#"{"users":[{"groups":["g"],"real_name":"T","id":1}]}"#),
        ResponseTemplate::new(200).set_body_string(r#"{"users":[{"id":1,"real_name":"T"}]}"#),
    )
    .await;

    assert!(header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn explicit_error_false_is_not_an_error_body() {
    // `carries_error` must reject only a *truthy* error key. A server that spells
    // "no error" explicitly still carries real data, and rejecting it would make
    // the credentialed legs unusable on that server.
    let server = MockServer::start().await;
    let ok_with_error_false = || {
        ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"error": false, "users": [{"id": 1, "real_name": "T", "groups": ["g"]}]}),
        )
    };
    mount_user_legs(
        &server,
        ok_with_error_false(),
        ok_with_error_false(),
        thin_user(),
    )
    .await;

    assert!(header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn non_json_header_body_is_not_confirmed() {
    // A parsed body never equals an unparsed one, so a server answering the
    // header leg with something that is not JSON cannot confirm.
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        ResponseTemplate::new(200).set_body_string("not json at all"),
        rich_user(),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn transient_anonymous_refusal_is_not_confirmed() {
    // The anonymous refusal is what the differential rests on when that leg is
    // non-2xx, so it must repeat. Here the server ignores the header and does not
    // discriminate (header and query-param bodies are equal), and a single
    // transient 401 on the anonymous leg would otherwise confirm header auth.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(thin_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(thin_user())
        .mount(&server)
        .await;
    // The anonymous leg is refused once, then answers normally on the re-check.
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(thin_user())
        .mount(&server)
        .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn header_leg_inconclusive_status_is_not_confirmed() {
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        ResponseTemplate::new(500),
        rich_user(),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
    assert_eq!(
        requests_received(&server).await,
        1,
        "an inconclusive header leg should end the probe immediately"
    );
}

#[tokio::test]
async fn query_leg_inconclusive_status_is_not_confirmed() {
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        rich_user(),
        ResponseTemplate::new(500),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn anonymous_recheck_inconclusive_status_is_not_confirmed() {
    // The re-check is a leg like any other: an inconclusive answer there means
    // the anonymous observation could not be reconfirmed, so nothing is proved.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(rich_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(rich_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(thin_user())
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn credential_challenging_middlebox_is_not_confirmed() {
    // A middlebox that challenges any credential-bearing request with a stable
    // 200 interstitial, and passes anonymous requests to the origin, answers both
    // credentialed legs identically while differing from the anonymous leg --
    // satisfying every comparison, on a server that honoured neither credential.
    // Requiring a credentialed leg's body to have parsed as JSON closes it.
    let server = MockServer::start().await;
    let interstitial =
        || ResponseTemplate::new(200).set_body_string("<html>please solve this challenge</html>");
    mount_user_legs(&server, interstitial(), interstitial(), thin_user()).await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn transient_two_hundred_anomaly_is_not_confirmed() {
    // The anomaly that makes the anonymous leg differ need not be a refusal: a
    // rate limiter or WAF can answer the second request of a burst with a 200
    // interstitial. On a server that ignores the header and does not discriminate
    // here, that alone would otherwise confirm header auth.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(thin_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(thin_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>rate limited</html>"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(thin_user())
        .mount(&server)
        .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn anonymous_refusal_with_a_changed_body_is_not_confirmed() {
    // The re-check compares the body as well as the status. A refusal that
    // repeats with *different* content is not the same observation, so it is not
    // the stable discrimination the confirmation rests on.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(rich_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(rich_user())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(bugzilla_error(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            serde_json::json!({"error": true, "code": 505, "message": "different refusal"}),
        ))
        .mount(&server)
        .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn null_error_key_is_not_an_error_body() {
    // `carries_error` rejects only a truthy error key: `null` is not one.
    let server = MockServer::start().await;
    let ok_with_null_error = || {
        ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"error": null, "users": [{"id": 1, "real_name": "T", "groups": ["g"]}]}),
        )
    };
    mount_user_legs(
        &server,
        ok_with_null_error(),
        ok_with_null_error(),
        thin_user(),
    )
    .await;

    assert!(header_auth_confirmed(&server).await);
}

#[tokio::test]
async fn repeated_anonymous_refusal_is_confirmed() {
    // The mirror of the case above: a `requirelogin` server refuses the anonymous
    // caller every time, so the refusal is policy and the confirmation stands.
    let server = MockServer::start().await;
    mount_user_legs(&server, rich_user(), rich_user(), bugzilla_error(401)).await;

    assert!(header_auth_confirmed(&server).await);
    assert_eq!(
        requests_received(&server).await,
        4,
        "a load-bearing anonymous refusal costs one extra re-check request"
    );
}

#[tokio::test]
async fn header_matching_neither_peer_is_not_confirmed() {
    let server = MockServer::start().await;
    mount_user_legs(
        &server,
        rich_user(),
        ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"users": [{"id": 1, "real_name": "T", "groups": ["h"]}]}),
        ),
        thin_user(),
    )
    .await;

    assert!(!header_auth_confirmed(&server).await);
}

#[test]
fn valid_login_result_from_bool_true() {
    let v: ValidLoginResult = serde_json::Value::Bool(true).try_into().unwrap();
    assert!(v.is_valid());
}

#[test]
fn valid_login_result_from_bool_false() {
    let v: ValidLoginResult = serde_json::Value::Bool(false).try_into().unwrap();
    assert!(!v.is_valid());
}

#[test]
fn valid_login_result_from_integer_1() {
    let v: ValidLoginResult = serde_json::json!(1).try_into().unwrap();
    assert!(v.is_valid());
}

#[test]
fn valid_login_result_from_integer_0() {
    let v: ValidLoginResult = serde_json::json!(0).try_into().unwrap();
    assert!(!v.is_valid());
}

#[test]
fn valid_login_result_from_string_errors() {
    let result: Result<ValidLoginResult, _> = serde_json::json!("yes").try_into();
    assert!(result.is_err());
}

#[test]
fn valid_login_response_deserializes() {
    let json = r#"{"result": true}"#;
    let resp: ValidLoginResponse = serde_json::from_str(json).unwrap();
    assert!(resp.result.is_valid());
}

#[test]
fn valid_login_response_integer_result() {
    let json = r#"{"result": 1}"#;
    let resp: ValidLoginResponse = serde_json::from_str(json).unwrap();
    assert!(resp.result.is_valid());
}

#[test]
fn valid_login_response_missing_result_errors() {
    let json = r"{}";
    let result = serde_json::from_str::<ValidLoginResponse>(json);
    assert!(result.is_err(), "missing result should fail to deserialize");
    let err = result.err().unwrap();
    assert!(
        err.to_string().contains("missing field `result`"),
        "unexpected error: {err}",
    );
}

#[tokio::test]
async fn current_header_proof_uses_only_the_configured_method() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .expect(0)
        .mount(&server)
        .await;

    prove_valid_login_current_method(
        &strict_http_client(),
        &server.uri(),
        "user@example.com",
        &PreparedAuth::Header(HeaderValue::from_static("test-key")),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn current_query_proof_uses_only_the_configured_method() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": 1})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .expect(0)
        .mount(&server)
        .await;

    prove_valid_login_current_method(
        &strict_http_client(),
        &server.uri(),
        "user@example.com",
        &PreparedAuth::QueryParam("test-key".to_owned()),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn current_proof_rejects_false_malformed_and_redirected_responses() {
    for (response, expected) in [
        (
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            "did not confirm",
        ),
        (
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"error": true})),
            "invalid response",
        ),
        (
            ResponseTemplate::new(302).insert_header("location", "/landed"),
            "unexpected HTTP status",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/landed"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .expect(0)
            .mount(&server)
            .await;

        let result = prove_valid_login_current_method(
            &strict_http_client(),
            &server.uri(),
            "user@example.com",
            &PreparedAuth::Header(HeaderValue::from_static("test-key")),
        )
        .await;

        assert!(matches!(result, Err(BzrError::Auth(_))));
        assert!(result.unwrap_err().to_string().contains(expected));
    }
}

#[tokio::test]
async fn current_proof_rejects_top_level_error_even_with_true_result() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"error": true, "result": true})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = prove_valid_login_current_method(
        &strict_http_client(),
        &server.uri(),
        "user@example.com",
        &PreparedAuth::Header(HeaderValue::from_static("test-key")),
    )
    .await;

    assert!(matches!(result, Err(BzrError::Auth(_))));
}
