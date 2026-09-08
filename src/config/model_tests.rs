#![expect(clippy::expect_used)]

use super::*;

#[test]
fn from_url_with_env_key_preserves_email() {
    let cfg = ServerConfig::from_url_with_env_key(
        "https://bugs.example.com".to_string(),
        "BUGZILLA_API_KEY".to_string(),
        Some("alice@example.com".to_string()),
    );
    assert_eq!(cfg.email.as_deref(), Some("alice@example.com"));
}

#[test]
fn from_url_with_env_key_none_email_is_none() {
    let cfg = ServerConfig::from_url_with_env_key(
        "https://bugs.example.com".to_string(),
        "BUGZILLA_API_KEY".to_string(),
        None,
    );
    assert!(cfg.email.is_none());
}

// ── auth_method provenance stamp (ADR-0066) ───────────────────────

#[test]
fn auth_method_is_trusted_only_for_known_markers() {
    let stamped = |source: Option<&str>| ServerConfig {
        url: "https://bugs.example.com".to_string(),
        auth_method: Some(AuthMethod::Header),
        auth_method_source: source.map(str::to_owned),
        ..ServerConfig::default()
    };

    // An unstamped value predates the stamp and may predate the differential
    // probe, so it must not be trusted.
    assert!(!stamped(None).auth_method_is_trusted());
    // A marker written by a newer bzr means nothing to this build.
    assert!(!stamped(Some("something-else")).auth_method_is_trusted());
    // An empty stamp is not a marker either.
    assert!(!stamped(Some("")).auth_method_is_trusted());

    assert!(stamped(Some(AUTH_METHOD_SOURCE_DETECTED)).auth_method_is_trusted());
    assert!(stamped(Some(AUTH_METHOD_SOURCE_PINNED)).auth_method_is_trusted());
}

#[test]
fn auth_method_source_markers_are_distinct() {
    // The two markers arbitrate different behaviours (re-detectable vs pinned),
    // so collapsing them to one value would silently make pins re-detectable.
    assert_ne!(AUTH_METHOD_SOURCE_DETECTED, AUTH_METHOD_SOURCE_PINNED);
}

#[test]
fn unknown_auth_method_source_deserializes_instead_of_failing_the_load() {
    // The whole point of `Option<String>` over an enum: a marker written by a
    // future bzr must not make the config file unreadable to every command.
    let config: Config = toml::from_str(
        r#"
default_server = "test"

[servers.test]
url = "https://bugs.example.com"
auth_method = "header"
auth_method_source = "from-the-future"
"#,
    )
    .expect("an unrecognised auth_method_source must still deserialize");

    let srv = &config.servers["test"];
    assert_eq!(srv.auth_method_source.as_deref(), Some("from-the-future"));
    assert!(
        !srv.auth_method_is_trusted(),
        "an unknown marker is a cache miss, not a trusted value"
    );
}

#[test]
fn absent_auth_method_source_round_trips_without_emitting_the_key() {
    let config: Config = toml::from_str(
        r#"
default_server = "test"

[servers.test]
url = "https://bugs.example.com"
auth_method = "header"
"#,
    )
    .expect("a config predating the stamp must load");
    assert_eq!(config.servers["test"].auth_method_source, None);

    let rendered = toml::to_string(&config).expect("config must serialize");
    assert!(
        !rendered.contains("auth_method_source"),
        "an unset stamp must not be written back into user config files"
    );
}

#[test]
fn from_url_with_env_key_preserves_url_and_env_var() {
    let cfg = ServerConfig::from_url_with_env_key(
        "https://bugs.example.com".to_string(),
        "MY_API_KEY_VAR".to_string(),
        Some("user@example.com".to_string()),
    );
    assert_eq!(cfg.url, "https://bugs.example.com");
    assert_eq!(cfg.api_key_env.as_deref(), Some("MY_API_KEY_VAR"));
}
