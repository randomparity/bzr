use super::BugzillaClient;
use crate::error::Result;
use crate::types::{CreateComponentParams, UpdateComponentParams};

impl BugzillaClient {
    pub async fn create_component(&self, params: &CreateComponentParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("component")).json(params));
        let resp = self.send(req).await?;
        let data: super::IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    /// Updates a component by numeric ID.
    ///
    /// Unlike peer update methods (`update_product`, `update_group`) which accept string names,
    /// the Bugzilla REST API's component endpoint only accepts numeric IDs.
    pub async fn update_component(&self, id: u64, updates: &UpdateComponentParams) -> Result<()> {
        self.put_json(&format!("component/{id}"), updates).await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;
    use crate::types::{CreateComponentParams, UpdateComponentParams};

    #[tokio::test]
    async fn create_component_returns_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/component"))
            .and(body_json(serde_json::json!({
                "product": "Widget",
                "name": "Backend",
                "description": "Backend component",
                "default_assignee": "dev@example.com",
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 10})))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let id = client
            .create_component(&CreateComponentParams {
                product: "Widget".into(),
                name: "Backend".into(),
                description: "Backend component".into(),
                default_assignee: "dev@example.com".into(),
            })
            .await
            .unwrap();
        assert_eq!(id, 10);
    }

    #[tokio::test]
    async fn create_component_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/component"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client
            .create_component(&CreateComponentParams {
                product: "X".into(),
                name: "Y".into(),
                description: "Z".into(),
                default_assignee: "a@b.com".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn update_component_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/component/10"))
            .and(body_json(serde_json::json!({
                "description": "New desc",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "components": [{"id": 10, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateComponentParams {
            description: Some("New desc".into()),
            ..Default::default()
        };
        client.update_component(10, &params).await.unwrap();
    }

    #[tokio::test]
    async fn update_component_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/component/10"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateComponentParams::default();
        let err = client.update_component(10, &params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }
}
