use serde::{Deserialize, Serialize};

use super::encode_path;
use super::{BugzillaClient, UserDetailLevel, UserSearchResponse};
use crate::error::{BzrError, Result};
use crate::types::common::ApiMode;
use crate::types::group::{CreateGroupParams, GroupInfo, UpdateGroupParams};
use crate::types::user::BugzillaUser;

#[derive(Serialize)]
struct GroupMembershipBody {
    groups: GroupMembershipAction,
}

#[derive(Serialize)]
struct GroupMembershipAction {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    remove: Vec<String>,
}
#[derive(Deserialize)]
struct GroupResponse {
    groups: Vec<GroupInfo>,
}

impl BugzillaClient {
    pub async fn get_group_members(
        &self,
        group_name: &str,
        detail_level: UserDetailLevel,
    ) -> Result<Vec<BugzillaUser>> {
        let fields = detail_level.include_fields();
        // Bugzilla 5.0 requires at least one of ids/names/match alongside
        // the group filter. Use a broad match pattern to list all members.
        let data: UserSearchResponse = self
            .get_json_query(
                "user",
                &[
                    ("group", group_name),
                    ("include_fields", fields),
                    ("match", "*"),
                ],
            )
            .await?;
        Ok(data.users)
    }

    pub async fn add_user_to_group(&self, user: &str, group: &str) -> Result<()> {
        let body = GroupMembershipBody {
            groups: GroupMembershipAction {
                add: vec![group.to_string()],
                remove: Vec::new(),
            },
        };
        self.put_json(&format!("user/{}", encode_path(user)), &body)
            .await
    }

    pub async fn remove_user_from_group(&self, user: &str, group: &str) -> Result<()> {
        let body = GroupMembershipBody {
            groups: GroupMembershipAction {
                add: Vec::new(),
                remove: vec![group.to_string()],
            },
        };
        self.put_json(&format!("user/{}", encode_path(user)), &body)
            .await
    }

    pub async fn get_group(&self, group: &str) -> Result<GroupInfo> {
        match self.api_mode {
            ApiMode::XmlRpc => return self.xmlrpc_client().get_group(group).await,
            ApiMode::Rest | ApiMode::Hybrid => {}
        }

        // Try REST first, fall back to XML-RPC on specific errors.
        // Bugzilla 5.3+ blocks GET for Group.get with error 32610, and
        // POST maps to Group.create, so REST is unusable for this
        // endpoint regardless of the configured API mode.
        match self.get_group_rest(group).await {
            Ok(info) => Ok(info),
            Err(BzrError::Api { code: 32610, .. }) => {
                tracing::info!(
                    "REST Group.get blocked (32610), \
                     falling back to XML-RPC"
                );
                self.xmlrpc_client().get_group(group).await
            }
            Err(e) if self.api_mode == ApiMode::Hybrid && e.is_transport_failure() => {
                tracing::info!(
                    "REST group lookup failed ({e}), \
                     retrying via XML-RPC"
                );
                self.xmlrpc_client().get_group(group).await
            }
            Err(e) => Err(e),
        }
    }

    async fn get_group_rest(&self, group: &str) -> Result<GroupInfo> {
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
#[path = "group_tests.rs"]
mod tests;
