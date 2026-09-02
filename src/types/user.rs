use serde::{Deserialize, Deserializer, Serialize};

use crate::types::deserialization::{option_bool_from_int_or_bool, u64_from_number_or_string};
use crate::types::transport::AuthMode;

fn deserialize_user_or_group_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    u64_from_number_or_string(
        deserializer,
        "an unsigned integer or decimal numeric string user/group ID",
        "expected an unsigned integer user/group ID",
    )
}

#[derive(Deserialize)]
struct UserGroupId(#[serde(deserialize_with = "deserialize_user_or_group_id")] u64);

fn deserialize_optional_user_group_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    Option::<UserGroupId>::deserialize(deserializer).map(|id| id.map(|id| id.0))
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BugzillaUser {
    #[serde(deserialize_with = "deserialize_user_or_group_id")]
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub groups: Vec<UserGroup>,
    #[serde(default, deserialize_with = "option_bool_from_int_or_bool")]
    pub can_login: Option<bool>,
}

/// Serde JSON keys of [`BugzillaUser`], for `--fields` / `--exclude-fields`
/// validation on `user search` and `group list-users`.
pub const BUGZILLA_USER_FIELDS: &[&str] =
    &["id", "name", "real_name", "email", "groups", "can_login"];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserGroup {
    #[serde(default, deserialize_with = "deserialize_optional_user_group_id")]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WhoamiResponse {
    #[serde(deserialize_with = "deserialize_user_or_group_id")]
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
}

/// The `whoami` output payload: the server-provided identity
/// ([`WhoamiResponse`]) flattened together with the connection metadata `bzr`
/// resolved locally (`server_name`, `auth_mode`). Flattening keeps the JSON a
/// single flat object so the identity fields stay at the top level (additive,
/// per the JSON output stability policy). See ADR 0009.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct WhoamiOutput {
    #[serde(flatten)]
    pub identity: WhoamiResponse,
    pub server_name: String,
    pub auth_mode: AuthMode,
}

impl From<BugzillaUser> for WhoamiResponse {
    fn from(user: BugzillaUser) -> Self {
        Self {
            id: user.id,
            name: user.name,
            real_name: user.real_name,
            login: user.email,
        }
    }
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CreateUserParams {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct UpdateUserParams {
    /// Bugzilla 5.0 requires `names` in the request body to identify the user.
    /// Newer versions accept the user in the URL path alone, but including
    /// `names` ensures cross-version compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_denied_text: Option<String>,
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
