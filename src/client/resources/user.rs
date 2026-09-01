use serde::Serialize;

use crate::client::encode_path;
use crate::client::{BugzillaClient, UserDetailLevel, UserSearchResponse};
use crate::error::{BzrError, Result};
use crate::types::transport::ApiMode;
use crate::types::user::{BugzillaUser, CreateUserParams, UpdateUserParams, WhoamiResponse};

#[derive(Serialize)]
struct UpdateUserRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    names: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    login_denied_text: Option<&'a str>,
}

impl<'a> From<&'a UpdateUserParams> for UpdateUserRequest<'a> {
    fn from(updates: &'a UpdateUserParams) -> Self {
        Self {
            names: updates.names.as_deref(),
            full_name: updates.real_name.as_deref(),
            email: updates.email.as_deref(),
            login_denied_text: updates.login_denied_text.as_deref(),
        }
    }
}

impl BugzillaClient {
    pub async fn whoami(&self) -> Result<WhoamiResponse> {
        let req = self.apply_auth(self.http.get(self.url("whoami")));
        let resp = self.send(req).await;
        match resp {
            Ok(r) => self.parse_json(r).await,
            Err(BzrError::Api { code: 32614, .. } | BzrError::HttpStatus { status: 404, .. }) => {
                // /rest/whoami is absent on Bugzilla 5.0/5.2. It may surface as
                // API error 32614 (JSON response) or raw HTTP 404 (non-JSON server).
                // Fall back to looking up the user by email if available.
                tracing::debug!("whoami endpoint not found, falling back to user lookup");
                if let Some(email) = &self.email_hint {
                    self.whoami_via_user_lookup(email).await
                } else {
                    Err(BzrError::Api {
                        code: 32614,
                        message: "Bugzilla 5.3+/BMO-derived servers use native whoami; this Bugzilla 5.0/5.2 server needs an email-backed fallback. Configure a named server with `bzr config set-server --email`, or add `--server-email` to an inline `--server-url` invocation".into(),
                    })
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Fallback for Bugzilla 5.0/5.2, which lack `/rest/whoami`.
    async fn whoami_via_user_lookup(&self, email: &str) -> Result<WhoamiResponse> {
        let data: UserSearchResponse = self.get_json_query("user", &[("names", email)]).await?;
        data.users
            .into_iter()
            .next()
            .map(WhoamiResponse::from)
            .ok_or_else(|| BzrError::NotFound {
                resource: "user",
                id: email.to_string(),
            })
    }

    pub async fn search_users(
        &self,
        query: &str,
        detail_level: UserDetailLevel,
    ) -> Result<Vec<BugzillaUser>> {
        let fields = detail_level.include_fields();
        let data: UserSearchResponse = self
            .get_json_query("user", &[("match", query), ("include_fields", fields)])
            .await?;
        Ok(data.users)
    }

    pub async fn create_user(&self, params: &CreateUserParams) -> Result<u64> {
        match self.api_mode {
            ApiMode::Rest => self.post_json_id("user", params).await,
            ApiMode::XmlRpc => self.xmlrpc_client().create_user(params).await,
            ApiMode::Hybrid if params.login.is_some() => {
                self.xmlrpc_client().create_user(params).await
            }
            ApiMode::Hybrid => match self.post_json_id("user", params).await {
                Ok(id) => Ok(id),
                Err(e) if e.is_transport_failure() => {
                    tracing::info!("REST user creation failed ({e}), retrying via XML-RPC");
                    self.xmlrpc_client().create_user(params).await
                }
                Err(e) => Err(e),
            },
        }
    }

    /// Update a user's profile fields.
    pub async fn update_user(&self, user: &str, updates: &UpdateUserParams) -> Result<()> {
        let request = UpdateUserRequest::from(updates);
        self.put_json(&format!("user/{}", encode_path(user)), &request)
            .await
    }
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
