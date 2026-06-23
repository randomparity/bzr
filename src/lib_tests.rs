#![expect(clippy::unwrap_used)]

use clap::Parser;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn dispatch_whoami_defaults_to_show_action() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "name": "admin@example.com",
            "real_name": "Admin User"
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "test", "--json", "whoami"]).unwrap();
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut __io.writers()).await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "dispatch failed: {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "admin@example.com");
}

#[tokio::test]
async fn dispatch_routes_local_query_commands() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--json",
        "query",
        "save",
        "firefox-new",
        "--product",
        "Firefox",
    ])
    .unwrap();
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut __io2.writers()).await;
    let output = __io2.out_str().to_string();
    assert!(result.is_ok(), "dispatch failed: {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "firefox-new");
    assert_eq!(parsed["action"], "saved");
}

#[tokio::test]
async fn dispatch_applies_config_flag_without_global_override() {
    let _lock = ENV_LOCK.lock().await;
    crate::config::Config::set_path_override(None);
    let tmp = tempfile::TempDir::new().unwrap();
    let xdg_home = tmp.path().join("xdg");
    let explicit_dir = tmp.path().join("explicit");
    std::fs::create_dir_all(&explicit_dir).unwrap();
    let explicit_config = explicit_dir.join("config.toml");
    std::fs::write(
        &explicit_config,
        r#"
default_server = "alternate"

[servers.alternate]
url = "https://bugzilla.example.test"
api_key = "secret"
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&explicit_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&explicit_config, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    // SAFETY: tests that mutate process environment hold ENV_LOCK.
    unsafe {
        std::env::remove_var("BZR_CONFIG");
        std::env::set_var("XDG_CONFIG_HOME", xdg_home);
    };

    let config_arg = explicit_config.to_string_lossy().into_owned();
    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--config",
        config_arg.as_str(),
        "--json",
        "config",
        "show",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dispatch failed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["config_file"], explicit_config.display().to_string());
    assert_eq!(parsed["default_server"], "alternate");
    assert_eq!(
        parsed["servers"]["alternate"]["url"],
        "https://bugzilla.example.test"
    );
}

#[tokio::test]
async fn dispatch_rejects_dry_run_on_unsupported_command() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // --dry-run on a non-mutation command is rejected before any network I/O,
    // so a silently-ignored preview can never turn into a real write.
    let cli = cli::Cli::try_parse_from(["bzr", "--server", "test", "--dry-run", "whoami"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::InputValidation(ref msg)) if msg.contains("--dry-run")
    ));
}

#[tokio::test]
async fn dispatch_allows_dry_run_on_bug_update() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server",
        "test",
        "--json",
        "--dry-run",
        "bug",
        "update",
        "5",
        "--status",
        "RESOLVED",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run dispatch failed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([5]));
}

#[tokio::test]
async fn dispatch_allows_dry_run_on_admin_create() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server",
        "test",
        "--json",
        "--dry-run",
        "product",
        "create",
        "--name",
        "DryRun",
        "--description",
        "Preview",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run dispatch failed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["resource"], "product");
    assert_eq!(parsed["action"], "dry-run");
}

#[tokio::test]
async fn dispatch_rejects_dry_run_on_group_membership_mutation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server",
        "test",
        "--dry-run",
        "group",
        "add-user",
        "--group",
        "editbugs",
        "--user",
        "alice@example.com",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::InputValidation(ref msg)) if msg.contains("product create")
            && msg.contains("group create/update")
    ));
}

#[tokio::test]
async fn dispatch_rejects_write_without_credentials_before_network() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr", "--server", "public", "comment", "add", "1", "--body", "test",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("comment add") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_rejects_bug_my_without_credentials() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "bug", "my"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("bug my") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_rejects_whoami_without_credentials() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "whoami"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("whoami") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_uses_current_invocation_context_for_validation_errors() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "whoami"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("whoami") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_rejects_inline_write_without_api_key_env_before_network() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server-url",
        mock.uri().as_str(),
        "comment",
        "add",
        "1",
        "--body",
        "test",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("comment add") && msg.contains("--server-api-key-env")
    ));
}

#[tokio::test]
async fn dispatch_allows_public_server_info_without_credentials() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"extensions": {}})),
        )
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "--json", "server", "info"])
        .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(result.is_ok(), "public server info failed: {result:?}");
}

#[tokio::test]
async fn dispatch_inline_tls_insecure_connects_to_self_signed_server_without_config() {
    assert_self_signed_inline_server_info_succeeds(3, |_server, _tmp| {
        vec!["--server-tls-insecure".into()]
    })
    .await;
}

#[tokio::test]
async fn dispatch_inline_tls_pin_sha256_connects_to_self_signed_server_without_config() {
    assert_self_signed_inline_server_info_succeeds(3, |server, _tmp| {
        vec!["--server-tls-pin-sha256".into(), server.pin_sha256.clone()]
    })
    .await;
}

#[tokio::test]
async fn dispatch_inline_tls_pin_now_connects_to_self_signed_server_without_config() {
    assert_self_signed_inline_server_info_succeeds(4, |_server, _tmp| {
        vec!["--server-tls-pin-now".into()]
    })
    .await;
}

#[tokio::test]
async fn dispatch_inline_tls_ca_cert_connects_to_self_signed_server_without_config() {
    assert_self_signed_inline_server_info_succeeds(3, |server, tmp| {
        let ca_path = tmp.path().join("ca.pem");
        std::fs::write(&ca_path, &server.ca_pem).unwrap();
        vec!["--server-tls-ca-cert".into(), ca_path.display().to_string()]
    })
    .await;
}

async fn assert_self_signed_inline_server_info_succeeds(
    expected_requests: usize,
    build_tls_args: impl FnOnce(&SelfSignedBugzillaServer, &tempfile::TempDir) -> Vec<String>,
) {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: tests are serialized via ENV_LOCK.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let server = spawn_self_signed_bugzilla_server(expected_requests);
    let mut argv = vec!["bzr".into(), "--server-url".into(), server.url.clone()];
    argv.extend(build_tls_args(&server, &tmp));
    argv.extend(["--json".into(), "server".into(), "info".into()]);
    let argv = argv.iter().map(String::as_str).collect::<Vec<_>>();
    let cli = cli::Cli::try_parse_from(argv).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;
    server.handle.join().unwrap();

    assert!(
        result.is_ok(),
        "ad-hoc TLS settings should connect to a self-signed server: {result:?}"
    );
    assert!(
        !tmp.path().join("bzr").join("config.toml").exists(),
        "an inline TLS connect must not create or write the config file"
    );
}

fn write_public_config(tmp: &tempfile::TempDir, server_url: &str) {
    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_content = format!(
        r#"
default_server = "public"

[servers.public]
url = "{server_url}"
"#,
    );
    std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(
            config_dir.join("config.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    // SAFETY: Tests are serialized via ENV_LOCK.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
}

struct SelfSignedBugzillaServer {
    url: String,
    pin_sha256: String,
    ca_pem: String,
    handle: std::thread::JoinHandle<()>,
}

fn spawn_self_signed_bugzilla_server(expected_requests: usize) -> SelfSignedBugzillaServer {
    let params = rcgen::CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    let cert_der_bytes = cert.der().to_vec();
    let pin_sha256 = crate::tls::fingerprint::compute_fingerprint(&cert_der_bytes);
    let ca_pem = cert.pem();
    let cert_der = rustls::pki_types::CertificateDer::from(cert_der_bytes);
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
    );
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let config = Arc::new(config);
        let deadline = Instant::now() + Duration::from_secs(5);
        for _ in 0..expected_requests {
            let Ok(tcp) = accept_until(&listener, deadline) else {
                return;
            };
            let Ok(connection) = rustls::ServerConnection::new(Arc::clone(&config)) else {
                return;
            };
            let mut stream = rustls::StreamOwned::new(connection, tcp);
            let Ok(request) = read_http_request(&mut stream) else {
                return;
            };
            if write_bugzilla_response(&mut stream, &request).is_err() {
                return;
            }
        }
    });

    SelfSignedBugzillaServer {
        url: format!("https://localhost:{port}"),
        pin_sha256,
        ca_pem,
        handle,
    }
}

fn accept_until(listener: &TcpListener, deadline: Instant) -> std::io::Result<std::net::TcpStream> {
    loop {
        match listener.accept() {
            Ok((tcp, _addr)) => {
                tcp.set_nonblocking(false)?;
                return Ok(tcp);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
}

fn read_http_request(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>,
) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut buf = [0_u8; 512];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn write_bugzilla_response(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>,
    request: &str,
) -> std::io::Result<()> {
    let mut parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let body = match path {
        "/rest/version" => r#"{"version":"5.1.2"}"#,
        "/rest/extensions" => r#"{"extensions":{}}"#,
        _ => "{}",
    };
    let body = if method == "HEAD" { "" } else { body };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
