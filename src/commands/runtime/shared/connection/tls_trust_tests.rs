#![expect(clippy::unwrap_used, clippy::panic)]

use crate::error::BzrError;
use crate::tls::TlsConfig;

use super::super::test_helpers::{connect_context, write_config};

#[test]
fn should_offer_tofu_false_when_insecure() {
    let tls = TlsConfig {
        insecure: true,
        ..Default::default()
    };
    let err = BzrError::Config("test".into());
    assert!(!super::should_offer_tofu(&err, &tls));
}

#[test]
fn should_offer_tofu_false_when_pin_configured() {
    let tls = TlsConfig {
        pin_sha256: Some("sha256//test".into()),
        ..Default::default()
    };
    let err = BzrError::Config("test".into());
    assert!(!super::should_offer_tofu(&err, &tls));
}

#[test]
fn should_offer_tofu_false_when_ca_configured() {
    let tls = TlsConfig {
        ca_cert_path: Some("/path".into()),
        ..Default::default()
    };
    let err = BzrError::Config("test".into());
    assert!(!super::should_offer_tofu(&err, &tls));
}

#[test]
fn should_offer_tofu_false_for_non_http_error() {
    let tls = TlsConfig::default();
    let err = BzrError::Config("not an HTTP error".into());
    assert!(!super::should_offer_tofu(&err, &tls));
}

/// `handle_tofu` calls `probe_server_cert`, which must hit a real TLS
/// endpoint. Pointing it at an unreachable port exercises the early
/// failure path (`probe_server_cert` returns `Err`), covering the entry
/// of `handle_tofu`.
#[tokio::test]
async fn handle_tofu_returns_error_when_probe_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Use an unreachable HTTPS URL so probe_server_cert fails fast.
    let config_path = write_config(&tmp, "https://127.0.0.1:1", "");

    let ctx = connect_context("test", "https://127.0.0.1:1", None, Some(config_path));
    let result = super::handle_tofu(&ctx).await;
    assert!(
        result.is_err(),
        "handle_tofu should propagate probe failure"
    );
}

/// `handle_pin_rotation` prompts the user; in non-interactive tests
/// `prompt_rotation` returns `false`, so the function must return a
/// "rotation rejected" config error covering lines 168-174.
#[tokio::test]
async fn handle_pin_rotation_rejects_in_noninteractive() {
    let tmp = tempfile::TempDir::new().unwrap();
    // `prompt_rotation` returns false non-interactively, so this errors before
    // any network/DNS — no name resolution, hence no ENV_LOCK.
    let config_path = write_config(
        &tmp,
        "https://example.test",
        "tls_pin_sha256 = \"sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\"\n\
         tls_pin_issuer = \"CN=Old\"",
    );

    let ctx = connect_context("test", "https://example.test", None, Some(config_path));
    let result = super::handle_pin_rotation(
        &ctx,
        "sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "sha256//BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=",
        "CN=New",
    )
    .await;
    match result {
        Err(BzrError::Config(msg)) => {
            assert!(
                msg.contains("rotation rejected"),
                "should be rotation-rejected error: {msg}"
            );
        }
        Err(other) => panic!("expected Config error, got {other:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// `should_offer_tofu` returns false for an `Http` error that does not
/// look like a TLS cert error. Construct a real reqwest error by
/// connecting plain HTTP to a wiremock URL with `connect_timeout` set
/// to a tiny value, then asserting on the predicate.
#[tokio::test]
async fn should_offer_tofu_false_for_non_tls_http_error() {
    // Build a real reqwest::Error by failing to connect to an
    // unreachable address. This is not a TLS error (plain HTTP), so
    // is_tls_cert_error should be false.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_millis(50))
        .build()
        .unwrap();
    let err = client
        .get("http://127.0.0.1:1/unreachable")
        .send()
        .await
        .unwrap_err();
    let bzr_err = BzrError::Http(err);
    let tls = TlsConfig::default();
    assert!(!super::should_offer_tofu(&bzr_err, &tls));
}

#[test]
fn tls_uses_default_trust_true_for_default_config() {
    assert!(super::tls_uses_default_trust(&TlsConfig::default()));
}

#[test]
fn tls_uses_default_trust_false_when_insecure() {
    let tls = TlsConfig {
        insecure: true,
        ..Default::default()
    };
    assert!(!super::tls_uses_default_trust(&tls));
}

#[test]
fn tls_uses_default_trust_false_when_ca_cert_set() {
    let tls = TlsConfig {
        ca_cert_path: Some("/path/to/ca.pem".into()),
        ..Default::default()
    };
    assert!(!super::tls_uses_default_trust(&tls));
}

#[test]
fn tls_uses_default_trust_false_when_pin_set() {
    let tls = TlsConfig {
        pin_sha256: Some("sha256//AAAA".into()),
        ..Default::default()
    };
    assert!(!super::tls_uses_default_trust(&tls));
}

/// `classify_and_handle_tls_failure` must silently pass through non-TLS
/// transport errors so the cached-path probe doesn't block on transient
/// network issues. The actual command will surface the same error with
/// full request context.
#[tokio::test]
async fn classify_and_handle_tls_failure_returns_none_for_non_tls_error() {
    // Build a real reqwest::Error from a connection failure — error
    // chain contains no TLS markers, so all three predicates miss.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_millis(50))
        .build()
        .unwrap();
    let err = client
        .get("http://127.0.0.1:1/unreachable")
        .send()
        .await
        .unwrap_err();
    let bzr_err = BzrError::Http(err);

    let tls_config = TlsConfig::default();
    let ctx = connect_context("test", "http://127.0.0.1:1/unreachable", None, None);
    let result = super::classify_and_handle_tls_failure(&bzr_err, &ctx, &tls_config).await;
    match result {
        Ok(None) => {}
        Ok(Some(_)) => panic!("expected Ok(None) for non-TLS error, got Some(client)"),
        Err(e) => panic!("expected Ok(None) for non-TLS error, got Err: {e}"),
    }
}

/// `probe_tls` must return `Err` (wrapped in `BzrError::Http`) when the
/// underlying request fails on transport. The cached-path branch then
/// delegates to `classify_and_handle_tls_failure`, which for non-TLS
/// errors returns `Ok(None)` and the cached values flow through.
#[tokio::test]
async fn probe_tls_returns_err_on_unreachable_address() {
    let tls_config = TlsConfig::default();
    let result = super::probe_tls(
        "http://127.0.0.1:1/unreachable",
        &tls_config,
        crate::http::REQUEST_TIMEOUT,
    )
    .await;
    match result {
        Err(BzrError::Http(_)) => {}
        Err(other) => panic!("expected Http error, got {other:?}"),
        Ok(()) => panic!("expected probe to fail against unreachable address"),
    }
}
