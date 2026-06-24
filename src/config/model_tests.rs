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
