use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::FieldValue;

#[derive(Deserialize)]
struct FieldBugResponse {
    fields: Vec<FieldEntry>,
}

#[derive(Deserialize)]
struct FieldEntry {
    values: Vec<FieldValue>,
}

impl BugzillaClient {
    /// Fetch legal values for a bug field.
    ///
    /// Returns `NotFound` when the server does not recognize the field name
    /// (empty `fields` array). An empty `Vec` means the field exists but has
    /// no legal values.
    pub async fn get_field_values(&self, field_name: &str) -> Result<Vec<FieldValue>> {
        let data: FieldBugResponse = self.get_json(&format!("field/bug/{field_name}")).await?;
        let field = data
            .fields
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "field",
                id: field_name.to_string(),
            })?;
        Ok(field.values)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;
    use crate::error::BzrError;

    #[tokio::test]
    async fn get_field_values_returns_values() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/field/bug/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "fields": [{
                    "values": [
                        {"name": "NEW", "sort_key": 100, "is_active": true, "can_change_to": [{"name": "ASSIGNED"}, {"name": "RESOLVED"}]},
                        {"name": "RESOLVED", "sort_key": 500, "is_active": true}
                    ]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let values = client.get_field_values("status").await.unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].name, "NEW");
        let transitions = values[0].can_change_to.as_ref().unwrap();
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].name, "ASSIGNED");
    }

    #[tokio::test]
    async fn get_field_values_unrecognized_field_returns_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/field/bug/nonexistent"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"fields": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_field_values("nonexistent").await.unwrap_err();
        assert!(
            matches!(
                err,
                BzrError::NotFound {
                    resource: "field",
                    ..
                }
            ),
            "expected NotFound, got: {err}"
        );
    }
}
