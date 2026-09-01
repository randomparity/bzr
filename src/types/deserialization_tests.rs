#![expect(clippy::unwrap_used)]

use serde::Deserialize;

use super::{option_bool_from_int_or_bool, u64_from_number_or_string};

#[derive(Debug, PartialEq, Eq)]
struct Unsigned(u64);

impl<'de> Deserialize<'de> for Unsigned {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        u64_from_number_or_string(
            deserializer,
            "an unsigned integer or decimal numeric string",
            "expected an unsigned integer",
        )
        .map(Self)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct OptionalBool {
    #[serde(default, deserialize_with = "option_bool_from_int_or_bool")]
    value: Option<bool>,
}

#[test]
fn unsigned_accepts_number_and_decimal_string() {
    assert_eq!(serde_json::from_str::<Unsigned>("7").unwrap(), Unsigned(7));
    assert_eq!(
        serde_json::from_str::<Unsigned>(r#""7""#).unwrap(),
        Unsigned(7)
    );
}

#[test]
fn unsigned_rejects_other_json_shapes_and_invalid_values() {
    for input in [
        "-1",
        r#""-1""#,
        "1.5",
        "true",
        "null",
        "[]",
        r#""seven""#,
        "18446744073709551616",
    ] {
        assert!(serde_json::from_str::<Unsigned>(input).is_err(), "{input}");
    }
}

#[test]
fn optional_bool_accepts_boolean_binary_integer_null_and_absence() {
    for (input, expected) in [
        (r#"{"value":true}"#, Some(true)),
        (r#"{"value":false}"#, Some(false)),
        (r#"{"value":1}"#, Some(true)),
        (r#"{"value":0}"#, Some(false)),
        (r#"{"value":null}"#, None),
        ("{}", None),
    ] {
        assert_eq!(
            serde_json::from_str::<OptionalBool>(input).unwrap(),
            OptionalBool { value: expected },
            "{input}"
        );
    }
}

#[test]
fn optional_bool_rejects_other_json_shapes_and_values() {
    for input in [
        r#"{"value":2}"#,
        r#"{"value":-1}"#,
        r#"{"value":1.5}"#,
        r#"{"value":"1"}"#,
        r#"{"value":[]}"#,
    ] {
        assert!(
            serde_json::from_str::<OptionalBool>(input).is_err(),
            "{input}"
        );
    }
}
