use serde::{Deserialize, Serialize};

/// Represents the four valid flag status values in Bugzilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagStatus {
    /// `+` — flag granted
    Grant,
    /// `-` — flag denied
    Deny,
    /// `?` — flag requested
    Request,
    /// `X` — flag cleared/removed
    Clear,
}

impl FlagStatus {
    pub fn to_char(self) -> char {
        match self {
            FlagStatus::Grant => '+',
            FlagStatus::Deny => '-',
            FlagStatus::Request => '?',
            FlagStatus::Clear => 'X',
        }
    }
}

impl std::fmt::Display for FlagStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

impl Serialize for FlagStatus {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_char(self.to_char())
    }
}

impl<'de> Deserialize<'de> for FlagStatus {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "+" => Ok(FlagStatus::Grant),
            "-" => Ok(FlagStatus::Deny),
            "?" => Ok(FlagStatus::Request),
            "X" => Ok(FlagStatus::Clear),
            other => Err(serde::de::Error::custom(format!(
                "invalid flag status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FlagUpdate {
    pub name: String,
    pub status: FlagStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestee: Option<String>,
}

/// A flag as returned on a bug or attachment view response.
///
/// Distinct from the write-side [`FlagUpdate`]: `status` is the raw server
/// token (`"+"`, `"-"`, `"?"`) kept as a plain `String` so an unexpected value
/// cannot make a view fail to deserialize, and `setter` (who set the flag) is
/// surfaced. Every field defaults so a server that omits one still parses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Flag {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requestee: Option<String>,
}

impl Flag {
    /// Concise one-line rendering for table/detail output, symmetric with the
    /// `name?(user@example.com)` flag-input syntax: `name` + status token, with
    /// the requestee in parentheses when present (e.g. `review?`, `review+`,
    /// `review?(bob@example.com)`).
    pub fn render_inline(&self) -> String {
        match &self.requestee {
            Some(requestee) => format!("{}{}({requestee})", self.name, self.status),
            None => format!("{}{}", self.name, self.status),
        }
    }
}

#[cfg(test)]
#[path = "flag_tests.rs"]
mod tests;
