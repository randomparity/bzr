use serde::{Deserialize, Deserializer, Serialize, Serializer};

const MIN: i128 = i64::MIN as i128;
const MAX: i128 = u64::MAX as i128;

pub(crate) fn deserialize_optional<'de, D>(deserializer: D) -> Result<Option<i128>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<i128>::deserialize(deserializer)?;
    validate(value).map_err(serde::de::Error::custom)
}

#[expect(
    clippy::ref_option,
    reason = "serde serialize_with requires a reference to the field's exact type"
)]
pub(crate) fn serialize_optional<S>(value: &Option<i128>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    validate(*value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

fn validate(value: Option<i128>) -> Result<Option<i128>, &'static str> {
    match value {
        Some(value) if !(MIN..=MAX).contains(&value) => {
            Err("sort_key must be between i64::MIN and u64::MAX")
        }
        value => Ok(value),
    }
}
