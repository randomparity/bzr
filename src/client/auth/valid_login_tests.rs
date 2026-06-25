#![expect(clippy::unwrap_used)]

use super::*;

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
