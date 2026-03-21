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
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bugzilla_user_deserializes_full() {
        let json = serde_json::json!({
            "id": 123,
            "name": "alice",
            "real_name": "Alice Smith",
            "email": "alice@example.com",
            "can_login": true,
            "groups": [
                {"id": 1, "name": "admin", "description": "Admins"}
            ]
        });
        let user: BugzillaUser = serde_json::from_value(json).unwrap();
        assert_eq!(user.id, 123);
        assert_eq!(user.name, "alice");
        assert_eq!(user.real_name.as_deref(), Some("Alice Smith"));
        assert_eq!(user.email.as_deref(), Some("alice@example.com"));
        assert_eq!(user.can_login, Some(true));
        assert_eq!(user.groups.len(), 1);
        assert_eq!(user.groups[0].name, "admin");
    }

    #[test]
    fn bugzilla_user_deserializes_minimal() {
        let json = serde_json::json!({"id": 1});
        let user: BugzillaUser = serde_json::from_value(json).unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "");
        assert!(user.real_name.is_none());
        assert!(user.email.is_none());
        assert!(user.can_login.is_none());
        assert!(user.groups.is_empty());
    }

    #[test]
    fn whoami_response_deserializes() {
        let json = serde_json::json!({
            "id": 42,
            "name": "bob",
            "real_name": "Bob Jones",
            "login": "bob@example.com"
        });
        let whoami: WhoamiResponse = serde_json::from_value(json).unwrap();
        assert_eq!(whoami.id, 42);
        assert_eq!(whoami.name, "bob");
        assert_eq!(whoami.real_name.as_deref(), Some("Bob Jones"));
        assert_eq!(whoami.login.as_deref(), Some("bob@example.com"));
    }

    #[test]
    fn whoami_from_bugzilla_user() {
        let user = BugzillaUser {
            id: 99,
            name: "carol".to_string(),
            real_name: Some("Carol White".to_string()),
            email: Some("carol@example.com".to_string()),
            groups: vec![],
            can_login: Some(true),
        };
        let whoami = WhoamiResponse::from(user);
        assert_eq!(whoami.id, 99);
        assert_eq!(whoami.name, "carol");
        assert_eq!(whoami.real_name.as_deref(), Some("Carol White"));
        assert_eq!(whoami.login.as_deref(), Some("carol@example.com"));
    }

    #[test]
    fn whoami_from_user_maps_email_to_login() {
        let user = BugzillaUser {
            id: 1,
            name: "test".to_string(),
            real_name: None,
            email: None,
            groups: vec![],
            can_login: None,
        };
        let whoami = WhoamiResponse::from(user);
        assert!(whoami.login.is_none());
    }
}
