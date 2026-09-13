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
