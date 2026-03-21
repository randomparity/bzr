use serde::Deserialize;

use super::encode_path;
use super::user::{UserSearchResponse, USER_FIELDS_BASIC, USER_FIELDS_DETAILED};
use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::{BugzillaUser, CreateGroupParams, GroupInfo, UpdateGroupParams};

#[derive(Deserialize)]
struct GroupResponse {
    groups: Vec<GroupInfo>,
}

enum GroupOp {
    Add,
    Remove,
}

impl BugzillaClient {
    pub async fn get_group_members(
        &self,
        group_name: &str,
        detailed: bool,
    ) -> Result<Vec<BugzillaUser>> {
        let fields = if detailed {
            USER_FIELDS_DETAILED
        } else {
            USER_FIELDS_BASIC
        };
        // Bugzilla 5.0 requires at least one of ids/names/match alongside
        // the group filter. Use a broad match pattern to list all members.
        let req = self.apply_auth(self.http.get(self.url("user")).query(&[
            ("group", group_name),
            ("include_fields", fields),
            ("match", "*"),
        ]));
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        Ok(data.users)
    }

    pub async fn add_user_to_group(&self, user: &str, group: &str) -> Result<()> {
        self.modify_group_membership(user, group, GroupOp::Add)
            .await
    }

    pub async fn remove_user_from_group(&self, user: &str, group: &str) -> Result<()> {
        self.modify_group_membership(user, group, GroupOp::Remove)
            .await
    }

    async fn modify_group_membership(
        &self,
        user: &str,
        group: &str,
        operation: GroupOp,
    ) -> Result<()> {
        let key = match operation {
            GroupOp::Add => "add",
            GroupOp::Remove => "remove",
        };
        let body = serde_json::json!({
            "groups": {
                key: [group]
            }
        });
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("user/{}", encode_path(user))))
                .json(&body),
        );
        self.send(req).await?;
        Ok(())
    }

    pub async fn get_group(&self, group: &str) -> Result<GroupInfo> {
        let req = self.apply_auth(
            self.http
                .get(self.url("group"))
                .query(&[("names", group), ("membership", "1")]),
        );
        let resp = self.send(req).await?;
        let data: GroupResponse = self.parse_json(resp).await?;
        data.groups
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "group",
                id: group.to_string(),
            })
    }

    pub async fn create_group(&self, params: &CreateGroupParams) -> Result<u64> {
        self.post_json_id("group", params).await
    }

    pub async fn update_group(&self, group: &str, updates: &UpdateGroupParams) -> Result<()> {
        self.put_json(&format!("group/{}", encode_path(group)), updates)
            .await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::encode_path;
    use super::super::user::USER_FIELDS_BASIC;
    use crate::client::test_helpers::test_client;
    use crate::types::{CreateGroupParams, UpdateGroupParams};

    #[tokio::test]
    async fn get_group_members_returns_users() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "admin"))
            .and(query_param("include_fields", USER_FIELDS_BASIC))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "id": 1,
                        "name": "alice@example.com",
                        "real_name": "Alice",
                        "email": "alice@example.com"
                    },
                    {
                        "id": 2,
                        "name": "bob@example.com",
                        "real_name": "Bob",
                        "email": "bob@example.com"
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("admin", false).await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "alice@example.com");
    }

    #[tokio::test]
    async fn get_group_members_details_sends_include_fields() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "admin"))
            .and(query_param(
                "include_fields",
                super::super::user::USER_FIELDS_DETAILED,
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "id": 1,
                        "name": "alice@example.com",
                        "real_name": "Alice",
                        "email": "alice@example.com",
                        "can_login": true,
                        "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("admin", true).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "alice@example.com");
        assert_eq!(users[0].groups.len(), 1);
        assert_eq!(users[0].groups[0].name, "admin");
        assert_eq!(users[0].can_login, Some(true));
    }

    #[tokio::test]
    async fn get_group_members_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "nobody"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"users": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("nobody", false).await.unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn get_group_members_api_error() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "nonexistent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "There is no group named 'nonexistent'."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client
            .get_group_members("nonexistent", false)
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "Expected error to mention group name, got: {msg}"
        );
    }

    #[tokio::test]
    async fn add_user_to_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("alice@example.com")
            )))
            .and(body_json(
                serde_json::json!({"groups": {"add": ["testers"]}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 1, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        client
            .add_user_to_group("alice@example.com", "testers")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remove_user_from_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("bob@example.com")
            )))
            .and(body_json(
                serde_json::json!({"groups": {"remove": ["testers"]}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 2, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        client
            .remove_user_from_group("bob@example.com", "testers")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_group_returns_info() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .and(query_param("names", "admin"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "groups": [{
                    "id": 1,
                    "name": "admin",
                    "description": "Administrators",
                    "is_active": true,
                    "membership": []
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let info = client.get_group("admin").await.unwrap();
        assert_eq!(info.name, "admin");
        assert!(info.is_active);
    }

    #[tokio::test]
    async fn get_group_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_group("secret").await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn create_group_returns_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/group"))
            .and(body_json(serde_json::json!({
                "name": "testers",
                "description": "Test team",
                "is_active": true,
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 5})))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let id = client
            .create_group(&CreateGroupParams {
                name: "testers".into(),
                description: "Test team".into(),
                is_active: true,
            })
            .await
            .unwrap();
        assert_eq!(id, 5);
    }

    #[tokio::test]
    async fn create_group_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client
            .create_group(&CreateGroupParams {
                name: "x".into(),
                description: "x".into(),
                is_active: true,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn update_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/group/testers"))
            .and(body_json(serde_json::json!({
                "description": "Updated testers",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "groups": [{"id": 5, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateGroupParams {
            description: Some("Updated testers".into()),
            ..Default::default()
        };
        client.update_group("testers", &params).await.unwrap();
    }

    #[tokio::test]
    async fn update_group_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/group/testers"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateGroupParams::default();
        let err = client.update_group("testers", &params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }
}
