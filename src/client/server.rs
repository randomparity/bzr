use super::user::UserSearchResponse;
use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::{ServerExtensions, ServerInfoResponse, ServerVersion, WhoamiResponse};

impl BugzillaClient {
    pub async fn whoami(&self, email_hint: Option<&str>) -> Result<WhoamiResponse> {
        let req = self.apply_auth(self.http.get(self.url("whoami")));
        let resp = self.send(req).await;
        match resp {
            Ok(r) => self.parse_json(r).await,
            Err(BzrError::Api { code: 32614, .. } | BzrError::HttpStatus { status: 404, .. }) => {
                // /rest/whoami not available (Bugzilla < 5.1). May surface as
                // API error 32614 (JSON response) or raw HTTP 404 (non-JSON server).
                // Fall back to looking up the user by email if available.
                tracing::debug!("whoami endpoint not found, falling back to user lookup");
                if let Some(email) = email_hint {
                    self.whoami_via_user_lookup(email).await
                } else {
                    Err(BzrError::Api {
                        code: 32614,
                        message: "whoami not available on this server; add --email to your server config for Bugzilla 5.0 compatibility".into(),
                    })
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Fallback for Bugzilla < 5.1 which lacks `/rest/whoami`.
    async fn whoami_via_user_lookup(&self, email: &str) -> Result<WhoamiResponse> {
        let req = self.apply_auth(self.http.get(self.url("user")).query(&[("names", email)]));
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        data.users
            .into_iter()
            .next()
            .map(WhoamiResponse::from)
            .ok_or_else(|| BzrError::NotFound {
                resource: "user",
                id: email.to_string(),
            })
    }

    /// Fetch version and extensions from the server (two sequential requests).
    pub async fn server_info(&self) -> Result<ServerInfoResponse> {
        let version = self.server_version().await?;
        let extensions = self.server_extensions().await?;
        Ok(ServerInfoResponse {
            version,
            extensions,
        })
    }

    pub async fn server_version(&self) -> Result<ServerVersion> {
        let req = self.apply_auth(self.http.get(self.url("version")));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        let req = self.apply_auth(self.http.get(self.url("extensions")));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;

    #[tokio::test]
    async fn whoami_returns_user_info() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 42,
                "name": "alice@example.com",
                "real_name": "Alice",
                "login": "alice@example.com"
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let who = client.whoami(None).await.unwrap();
        assert_eq!(who.id, 42);
        assert_eq!(who.name, "alice@example.com");
        assert_eq!(who.real_name.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn server_version_returns_version() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let ver = client.server_version().await.unwrap();
        assert_eq!(ver.version, "5.0.4");
    }

    #[tokio::test]
    async fn server_extensions_returns_map() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/extensions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "extensions": {
                    "BmpConvert": {"version": "1.0"},
                    "InlineHistory": {"version": "2.1"}
                }
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let ext = client.server_extensions().await.unwrap();
        assert_eq!(ext.extensions.len(), 2);
        assert!(ext.extensions.contains_key("BmpConvert"));
    }
}
