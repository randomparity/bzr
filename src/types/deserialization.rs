use std::fmt;

use serde::de::{Error as _, Visitor};
use serde::{Deserialize, Deserializer};

pub(crate) fn u64_from_number_or_string<'de, D: Deserializer<'de>>(
    deserializer: D,
    expecting: &'static str,
    invalid: &'static str,
) -> Result<u64, D::Error> {
    deserializer.deserialize_any(UnsignedVisitor { expecting, invalid })
}

struct UnsignedVisitor {
    expecting: &'static str,
    invalid: &'static str,
}

impl Visitor<'_> for UnsignedVisitor {
    type Value = u64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(value)
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
        u64::try_from(value).map_err(|_| E::custom(self.invalid))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        value.parse().map_err(|_| E::custom(self.invalid))
    }
}

pub(crate) fn option_bool_from_int_or_bool<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Bool(value) => Ok(Some(value)),
        serde_json::Value::Number(number) => match number.as_u64() {
            Some(0) => Ok(Some(false)),
            Some(1) => Ok(Some(true)),
            _ => Err(D::Error::custom(format!(
                "expected bool or 0/1 integer, got {number}"
            ))),
        },
        other => Err(D::Error::custom(format!(
            "expected bool or integer, got {other}"
        ))),
    }
}

#[cfg(test)]
#[path = "deserialization_tests.rs"]
mod tests;
