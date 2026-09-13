use crate::config::ServerConfig;

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
