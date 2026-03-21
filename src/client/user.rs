use super::encode_path;
use super::{BugzillaClient, UserSearchResponse, USER_FIELDS_BASIC, USER_FIELDS_DETAILED};
use crate::error::Result;
use crate::types::{BugzillaUser, CreateUserParams, UpdateUserParams};

impl BugzillaClient {
    pub async fn search_users(&self, query: &str, detailed: bool) -> Result<Vec<BugzillaUser>> {
        let fields = if detailed {
            USER_FIELDS_DETAILED
        } else {
            USER_FIELDS_BASIC
        };
        let req_builder = self
            .http
            .get(self.url("user"))
            .query(&[("match", query)])
            .query(&[("include_fields", fields)]);
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        Ok(data.users)
    }

    pub async fn create_user(&self, params: &CreateUserParams) -> Result<u64> {
        self.post_json_id("user", params).await
    }

    /// Update a user's profile fields.
    pub async fn update_user(&self, user: &str, updates: &UpdateUserParams) -> Result<()> {
        self.put_json(&format!("user/{}", encode_path(user)), updates)
            .await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::encode_path;
    use crate::client::test_helpers::test_client;
    use crate::types::{CreateUserParams, UpdateUserParams};

    #[tokio::test]
    async fn search_users_returns_matches() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {"id": 1, "name": "alice@example.com", "real_name": "Alice", "email": "alice@example.com"},
                    {"id": 2, "name": "bob@example.com", "real_name": "Bob", "email": "bob@example.com"}
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("example", false).await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "alice@example.com");
        assert_eq!(users[1].real_name.as_deref(), Some("Bob"));
    }

    #[tokio::test]
    async fn search_users_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"users": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("nobody", false).await.unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn search_users_details_sends_include_fields() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("match", "alice"))
            .and(query_param("include_fields", super::USER_FIELDS_DETAILED))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{
                    "id": 1,
                    "name": "alice@example.com",
                    "real_name": "Alice",
                    "email": "alice@example.com",
                    "can_login": true,
                    "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("alice", true).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].can_login, Some(true));
        assert_eq!(users[0].groups.len(), 1);
        assert_eq!(users[0].groups[0].name, "admin");
    }

    #[tokio::test]
    async fn create_user_returns_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/user"))
            .and(body_json(serde_json::json!({
                "email": "new@example.com",
                "full_name": "New User",
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = CreateUserParams {
            email: "new@example.com".into(),
            full_name: Some("New User".into()),
            password: None,
        };
        let id = client.create_user(&params).await.unwrap();
        assert_eq!(id, 99);
    }

    #[tokio::test]
    async fn create_user_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = CreateUserParams {
            email: "x@x.com".into(),
            full_name: None,
            password: None,
        };
        let err = client.create_user(&params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn update_user_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("alice@example.com")
            )))
            .and(body_json(serde_json::json!({
                "real_name": "Alice Smith",
                "names": ["alice@example.com"],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 1, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateUserParams {
            names: Some(vec!["alice@example.com".into()]),
            real_name: Some("Alice Smith".into()),
            ..Default::default()
        };
        client
            .update_user("alice@example.com", &params)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn update_user_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("alice@example.com")
            )))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateUserParams::default();
        let err = client
            .update_user("alice@example.com", &params)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }
}
