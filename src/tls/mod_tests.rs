#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn build_tls_client_default_succeeds() {
    let client = build_tls_client(&TlsConfig::default(), crate::http::REQUEST_TIMEOUT);
    assert!(client.is_ok());
}

#[test]
fn build_tls_client_insecure_succeeds() {
    let config = TlsConfig {
        insecure: true,
        ..Default::default()
    };
    assert!(build_tls_client(&config, crate::http::REQUEST_TIMEOUT).is_ok());
}

#[test]
fn build_tls_client_pinned_succeeds() {
    let config = TlsConfig {
        pin_sha256: Some(crate::tls::fingerprint::compute_fingerprint(b"test")),
        server_name: Some("test".into()),
        ..Default::default()
    };
    assert!(build_tls_client(&config, crate::http::REQUEST_TIMEOUT).is_ok());
}

#[test]
fn build_tls_client_bad_pin_fails() {
    let config = TlsConfig {
        pin_sha256: Some("not-a-valid-pin".into()),
        ..Default::default()
    };
    assert!(build_tls_client(&config, crate::http::REQUEST_TIMEOUT).is_err());
}

#[test]
fn build_tls_client_missing_ca_cert_fails() {
    let config = TlsConfig {
        ca_cert_path: Some("/nonexistent/ca.pem".into()),
        ..Default::default()
    };
    let err = build_tls_client(&config, crate::http::REQUEST_TIMEOUT).unwrap_err();
    assert!(
        err.to_string().contains("failed to read"),
        "should report missing file: {err}"
    );
}

/// Stand up two loopback mock servers where `origin` issues a `301` to a
/// different host (`127.0.0.1` vs `localhost`) so a followed redirect is
/// genuinely cross-host. Returns `(origin, target)`; `target` records every
/// request that reaches it.
async fn cross_host_redirect_pair() -> (wiremock::MockServer, wiremock::MockServer) {
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let target = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .mount(&target)
        .await;
    let target_port = target.address().port();

    let origin = MockServer::start().await;
    let location = format!("http://localhost:{target_port}/landed");
    Mock::given(any())
        .respond_with(ResponseTemplate::new(301).insert_header("location", location.as_str()))
        .mount(&origin)
        .await;

    (origin, target)
}

#[tokio::test]
async fn api_key_header_not_forwarded_across_cross_host_redirect() {
    let (origin, target) = cross_host_redirect_pair().await;
    let client = build_tls_client(&TlsConfig::default(), crate::http::REQUEST_TIMEOUT).unwrap();

    // Mirror the real client's header-auth attach.
    let _ = client
        .get(format!("{}/start", origin.uri()))
        .header(crate::bugzilla_auth::AUTH_HEADER_NAME, "SECRET-API-KEY")
        .send()
        .await;

    let leaked = target.received_requests().await.unwrap().iter().any(|r| {
        r.headers
            .contains_key(crate::bugzilla_auth::AUTH_HEADER_NAME)
    });
    assert!(
        !leaked,
        "API key header must not be forwarded to a different host across a redirect"
    );
}

#[tokio::test]
async fn api_key_query_param_not_forwarded_across_cross_host_redirect() {
    let (origin, target) = cross_host_redirect_pair().await;
    let client = build_tls_client(&TlsConfig::default(), crate::http::REQUEST_TIMEOUT).unwrap();

    let _ = client
        .get(format!("{}/start", origin.uri()))
        .query(&[(crate::bugzilla_auth::AUTH_QUERY_PARAM, "SECRET-QP-KEY")])
        .send()
        .await;

    let leaked = target.received_requests().await.unwrap().iter().any(|r| {
        r.url
            .query()
            .is_some_and(|q| q.contains(crate::bugzilla_auth::AUTH_QUERY_PARAM))
    });
    assert!(
        !leaked,
        "API key query param must not be forwarded to a different host across a redirect"
    );
}

#[tokio::test]
async fn same_host_redirect_is_followed_with_credentials() {
    use wiremock::matchers::{any, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // A single server that 301s /start -> /landed on itself: a genuinely
    // same-host (same host AND port) redirect, which the policy must follow
    // and over which the credential is legitimately retained.
    let server = MockServer::start().await;
    Mock::given(path("/start"))
        .respond_with(ResponseTemplate::new(301).insert_header("location", "/landed"))
        .mount(&server)
        .await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = build_tls_client(&TlsConfig::default(), crate::http::REQUEST_TIMEOUT).unwrap();
    let resp = client
        .get(format!("{}/start", server.uri()))
        .header(crate::bugzilla_auth::AUTH_HEADER_NAME, "SECRET-API-KEY")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "same-host redirect should be followed");

    let landed_with_key = server.received_requests().await.unwrap().iter().any(|r| {
        r.url.path() == "/landed"
            && r.headers
                .contains_key(crate::bugzilla_auth::AUTH_HEADER_NAME)
    });
    assert!(
        landed_with_key,
        "same-host redirect should reach /landed carrying the API key header"
    );
}
