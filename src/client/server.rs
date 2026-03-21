use super::BugzillaClient;
use crate::error::Result;
use crate::types::{ServerExtensions, ServerInfoResponse, ServerVersion};

impl BugzillaClient {
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
        self.get_json("version").await
    }

    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        self.get_json("extensions").await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;

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
