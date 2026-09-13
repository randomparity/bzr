#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::cli::AuthAction;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::{Config, ServerConfig};
use crate::test_helpers::{write_config_to, CapturedIo};
use crate::types::OutputFormat;

fn context(config_path: std::path::PathBuf) -> CommandContext {
    CommandContext::new(Some("test"), OutputFormat::Json, None)
        .with_config_path_override(Some(config_path))
}

fn config_path(tmp: &tempfile::TempDir, url: &str, token: Option<&str>) -> std::path::PathBuf {
    let token = token.map_or_else(String::new, |token| format!("token = \"{token}\"\n"));
    write_config_to(
        tmp,
        &format!(
            "default_server = \"test\"\n\n[servers.test]\nurl = \"{url}\"\napi_mode = \"rest\"\n{token}"
        ),
    )
}

#[test]
fn api_key_source_blocks_token_login() {
    let server = ServerConfig {
        api_key: Some("key".into()),
        ..ServerConfig::default()
    };
    assert!(super::ensure_token_slot(&server, "test").is_err());
}

#[test]
fn empty_server_allows_token_login() {
    assert!(super::ensure_token_slot(&ServerConfig::default(), "test").is_ok());
}

#[test]
fn logout_keeps_a_token_replaced_by_a_concurrent_login() {
    let mut server = ServerConfig {
        token: Some("new-token".into()),
        ..ServerConfig::default()
    };

    super::clear_token_if_matches(&mut server, "old-token");

    assert_eq!(server.token.as_deref(), Some("new-token"));
    super::clear_token_if_matches(&mut server, "new-token");
    assert!(server.token.is_none());
}

#[tokio::test]
async fn login_persists_the_returned_token_and_reports_json() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), None);
    Mock::given(method("GET"))
        .and(path("/rest/login"))
        .and(query_param("login", "alice@example.test"))
        .and(query_param("password", "secret"))
        .and(query_param("restrict_login", "1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "saved"})),
        )
        .mount(&mock)
        .await;
    let action = AuthAction::Login {
        email: "alice@example.test".into(),
        password: Some("secret".into()),
        restrict_login: true,
    };
    let mut io = CapturedIo::new();

    super::execute(&action, &context(config_path.clone()), &mut io.writers())
        .await
        .unwrap();

    assert_eq!(
        Config::load_at(Some(&config_path)).unwrap().servers["test"]
            .token
            .as_deref(),
        Some("saved")
    );
    assert_eq!(
        Config::load_at(Some(&config_path)).unwrap().servers["test"]
            .email
            .as_deref(),
        Some("alice@example.test")
    );
    assert_eq!(
        crate::test_helpers::json_envelope_data(io.out_str())["action"],
        "logged-in"
    );
}

#[tokio::test]
async fn login_replaces_a_stale_saved_email() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), Some("old-token"));
    Config::update_locked_at(Some(&config_path), |config| {
        config.servers.get_mut("test").unwrap().email = Some("old@example.test".into());
        Ok(())
    })
    .unwrap();
    Mock::given(method("GET"))
        .and(path("/rest/login"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "saved"})),
        )
        .mount(&mock)
        .await;
    let action = AuthAction::Login {
        email: "alice@example.test".into(),
        password: Some("secret".into()),
        restrict_login: false,
    };
    let mut io = CapturedIo::new();

    super::execute(&action, &context(config_path.clone()), &mut io.writers())
        .await
        .unwrap();

    let saved = Config::load_at(Some(&config_path)).unwrap();
    assert_eq!(saved.servers["test"].token.as_deref(), Some("saved"));
    assert_eq!(
        saved.servers["test"].email.as_deref(),
        Some("alice@example.test")
    );
}

#[tokio::test]
async fn failed_login_keeps_the_saved_credential_and_identity_pair() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), Some("old-token"));
    Config::update_locked_at(Some(&config_path), |config| {
        config.servers.get_mut("test").unwrap().email = Some("old@example.test".into());
        Ok(())
    })
    .unwrap();
    Mock::given(method("GET"))
        .and(path("/rest/login"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let action = AuthAction::Login {
        email: "alice@example.test".into(),
        password: Some("secret".into()),
        restrict_login: false,
    };
    let mut io = CapturedIo::new();

    assert!(
        super::execute(&action, &context(config_path.clone()), &mut io.writers())
            .await
            .is_err()
    );

    let saved = Config::load_at(Some(&config_path)).unwrap();
    assert_eq!(saved.servers["test"].token.as_deref(), Some("old-token"));
    assert_eq!(
        saved.servers["test"].email.as_deref(),
        Some("old@example.test")
    );
}

#[tokio::test]
async fn logout_revokes_then_removes_the_saved_token() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), Some("saved"));
    Mock::given(method("GET"))
        .and(path("/rest/logout"))
        .and(query_param("Bugzilla_token", "saved"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&mock)
        .await;
    let mut io = CapturedIo::new();

    super::execute(
        &AuthAction::Logout,
        &context(config_path.clone()),
        &mut io.writers(),
    )
    .await
    .unwrap();

    assert!(Config::load_at(Some(&config_path)).unwrap().servers["test"]
        .token
        .is_none());
    assert_eq!(
        crate::test_helpers::json_envelope_data(io.out_str())["action"],
        "logged-out"
    );
}

#[tokio::test]
async fn failed_logout_retains_the_saved_token() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), Some("saved"));
    Mock::given(method("GET"))
        .and(path("/rest/logout"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let mut io = CapturedIo::new();

    assert!(super::execute(
        &AuthAction::Logout,
        &context(config_path.clone()),
        &mut io.writers()
    )
    .await
    .is_err());

    assert_eq!(
        Config::load_at(Some(&config_path)).unwrap().servers["test"]
            .token
            .as_deref(),
        Some("saved")
    );
}

#[tokio::test]
async fn login_refuses_to_replace_an_api_key_source() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), None);
    Config::update_locked_at(Some(&config_path), |config| {
        config.servers.get_mut("test").unwrap().api_key = Some("key".into());
        Ok(())
    })
    .unwrap();
    let action = AuthAction::Login {
        email: "alice@example.test".into(),
        password: Some("secret".into()),
        restrict_login: false,
    };
    let mut io = CapturedIo::new();

    let error = super::execute(&action, &context(config_path), &mut io.writers())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("API-key credential source"));
}

#[tokio::test]
async fn logout_without_a_saved_token_fails_locally() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = config_path(&tmp, &mock.uri(), None);
    let mut io = CapturedIo::new();

    let error = super::execute(
        &AuthAction::Logout,
        &context(config_path),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("no saved login token"));
}

#[tokio::test]
async fn login_detects_rest_when_the_server_has_no_cached_api_mode() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config_to(
        &tmp,
        &format!(
            "default_server = \"test\"\n\n[servers.test]\nurl = \"{}\"\n",
            mock.uri()
        ),
    );
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.3.0"})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/login"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "saved"})),
        )
        .mount(&mock)
        .await;
    let action = AuthAction::Login {
        email: "alice@example.test".into(),
        password: Some("secret".into()),
        restrict_login: false,
    };
    let mut io = CapturedIo::new();

    super::execute(&action, &context(config_path), &mut io.writers())
        .await
        .unwrap();
}

#[tokio::test]
async fn auth_commands_reject_inline_servers_before_loading_config() {
    let inline = crate::commands::runtime::invocation::InlineServer {
        url: "https://example.test".into(),
        api_key_env: None,
        email: None,
        tls: crate::commands::runtime::invocation::InlineTlsOptions::default(),
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None).with_inline_server(Some(inline));
    let mut io = CapturedIo::new();

    let error = super::execute(&AuthAction::Logout, &ctx, &mut io.writers())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("require a named server"));
}
