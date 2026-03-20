use crate::cli::GroupAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateGroupParams, UpdateGroupParams};

pub async fn execute(
    action: &GroupAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        GroupAction::AddUser { group, user } => {
            client.add_user_to_group(user, group).await?;
            output::print_result(
                &serde_json::json!({
                    "user": user,
                    "group": group,
                    "resource": "group_membership",
                    "action": "added",
                }),
                &format!("Added {user} to group '{group}'"),
                format,
            );
        }
        GroupAction::RemoveUser { group, user } => {
            client.remove_user_from_group(user, group).await?;
            output::print_result(
                &serde_json::json!({
                    "user": user,
                    "group": group,
                    "resource": "group_membership",
                    "action": "removed",
                }),
                &format!("Removed {user} from group '{group}'"),
                format,
            );
        }
        GroupAction::ListUsers { group, details } => {
            let users = client.get_group_members(group, *details).await?;
            output::print_users(&users, *details, format);
        }
        GroupAction::View { group } => {
            let info = client.get_group(group).await?;
            output::print_group_info(&info, format);
        }
        GroupAction::Create {
            name,
            description,
            is_active,
        } => {
            let params = CreateGroupParams {
                name: name.clone(),
                description: description.clone(),
                is_active: *is_active,
            };
            let id = client.create_group(&params).await?;
            output::print_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Group),
                &format!("Created group #{id} '{name}'"),
                format,
            );
        }
        GroupAction::Update {
            group,
            description,
            is_active,
        } => {
            let params = UpdateGroupParams {
                description: description.clone(),
                is_active: *is_active,
            };
            client.update_group(group, &params).await?;
            output::print_result(
                &ActionResult::updated_named(group.as_str(), ResourceKind::Group),
                &format!("Updated group '{group}'"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::await_holding_lock)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::cli::GroupAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn group_view_returns_info() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "groups": [{
                    "id": 1,
                    "name": "admin",
                    "description": "Admin group",
                    "is_active": true,
                    "membership": []
                }]
            })))
            .mount(&mock)
            .await;

        let action = GroupAction::View {
            group: "admin".to_string(),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok(), "group_view failed: {result:?}");
    }

    #[tokio::test]
    async fn group_create_sends_post() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("POST"))
            .and(path("/rest/group"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let action = GroupAction::Create {
            name: "new-group".into(),
            description: "A test group".into(),
            is_active: true,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok(), "group create failed: {result:?}");
    }

    #[tokio::test]
    async fn group_update_sends_put() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("PUT"))
            .and(path("/rest/group/admin"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"groups": [{"changes": {}}]})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let action = GroupAction::Update {
            group: "admin".into(),
            description: Some("Updated description".into()),
            is_active: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok(), "group update failed: {result:?}");
    }

    #[tokio::test]
    async fn group_add_user_sends_put() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        // add_user_to_group sends PUT /rest/user/{user} with group membership body
        Mock::given(method("PUT"))
            .and(path("/rest/user/alice%40test%2Ecom"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"users": [{"id": 1, "changes": {}}]})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let action = GroupAction::AddUser {
            group: "admin".into(),
            user: "alice@test.com".into(),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok(), "group add_user failed: {result:?}");
    }
}
