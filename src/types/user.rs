use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BugzillaUser {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub groups: Vec<UserGroup>,
    #[serde(default)]
    pub can_login: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserGroup {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WhoamiResponse {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
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
