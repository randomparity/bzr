#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn auth_method_from_str() {
    assert_eq!("header".parse::<AuthMethod>().unwrap(), AuthMethod::Header);
    assert_eq!(
        "query_param".parse::<AuthMethod>().unwrap(),
        AuthMethod::QueryParam
    );
    assert_eq!(
        "query-param".parse::<AuthMethod>().unwrap(),
        AuthMethod::QueryParam
    );
    assert!("bogus".parse::<AuthMethod>().is_err());
}

#[test]
fn api_mode_from_str() {
    assert_eq!("rest".parse::<ApiMode>().unwrap(), ApiMode::Rest);
    assert_eq!("xmlrpc".parse::<ApiMode>().unwrap(), ApiMode::XmlRpc);
    assert_eq!("hybrid".parse::<ApiMode>().unwrap(), ApiMode::Hybrid);
    assert!("grpc".parse::<ApiMode>().is_err());
}

#[test]
fn auth_mode_serializes_to_contract_strings() {
    assert_eq!(
        serde_json::to_value(AuthMode::ApiKey).unwrap(),
        serde_json::json!("api_key")
    );
    assert_eq!(
        serde_json::to_value(AuthMode::Anonymous).unwrap(),
        serde_json::json!("anonymous")
    );
}

#[test]
fn auth_mode_display_matches_wire_strings() {
    assert_eq!(AuthMode::ApiKey.to_string(), "api_key");
    assert_eq!(AuthMode::Anonymous.to_string(), "anonymous");
}
