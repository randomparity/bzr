use base64::Engine;
use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};

#[derive(Deserialize)]
struct AttachmentBugResponse {
    bugs: std::collections::HashMap<String, Vec<Attachment>>,
}

#[derive(Deserialize)]
struct AttachmentByIdResponse {
    attachments: std::collections::HashMap<String, Attachment>,
}

#[derive(Deserialize)]
struct AttachmentCreateResponse {
    ids: Vec<u64>,
}

impl BugzillaClient {
    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<Attachment>> {
        let req = self.apply_auth(self.http.get(self.url(&format!("bug/{bug_id}/attachment"))));
        let resp = self.send(req).await?;
        let data: AttachmentBugResponse = self.parse_json(resp).await?;
        let attachments = data.bugs.into_values().next().unwrap_or_default();
        Ok(attachments)
    }

    pub async fn get_attachment(&self, attachment_id: u64) -> Result<Attachment> {
        let req = self.apply_auth(
            self.http
                .get(self.url(&format!("bug/attachment/{attachment_id}"))),
        );
        let resp = self.send(req).await?;
        let data: AttachmentByIdResponse = self.parse_json(resp).await?;
        data.attachments
            .into_values()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "attachment",
                id: attachment_id.to_string(),
            })
    }

    pub async fn download_attachment(&self, attachment_id: u64) -> Result<(String, Vec<u8>)> {
        let attachment = self.get_attachment(attachment_id).await?;
        let data = attachment
            .data
            .ok_or_else(|| BzrError::DataIntegrity("attachment has no data".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| BzrError::DataIntegrity(format!("failed to decode attachment: {e}")))?;
        Ok((attachment.file_name, bytes))
    }

    pub async fn upload_attachment(&self, params: &UploadAttachmentParams) -> Result<u64> {
        let req = self.apply_auth(
            self.http
                .post(self.url(&format!("bug/{}/attachment", params.bug_id)))
                .json(params),
        );
        let resp = self.send(req).await?;
        let data: AttachmentCreateResponse = self.parse_json(resp).await?;
        data.ids
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::DataIntegrity("no attachment ID returned".into()))
    }

    pub async fn update_attachment(&self, id: u64, updates: &UpdateAttachmentParams) -> Result<()> {
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("bug/attachment/{id}")))
                .json(updates),
        );
        self.send(req).await?;
        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;
    use crate::types::{FlagStatus, FlagUpdate, UpdateAttachmentParams, UploadAttachmentParams};

    #[tokio::test]
    async fn update_attachment_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/bug/attachment/100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "attachments": [{"id": 100, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateAttachmentParams {
            is_obsolete: Some(true),
            summary: Some("Updated patch".into()),
            ..Default::default()
        };
        client.update_attachment(100, &params).await.unwrap();
    }

    #[tokio::test]
    async fn upload_attachment_with_flags_sends_flags() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/bug/1/attachment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let flags = vec![FlagUpdate {
            name: "review".into(),
            status: FlagStatus::Request,
            requestee: Some("alice@example.com".into()),
        }];
        let id = client
            .upload_attachment(&UploadAttachmentParams {
                bug_id: 1,
                file_name: "test.txt".into(),
                summary: "test".into(),
                content_type: "text/plain".into(),
                data: b"hello".to_vec(),
                flags,
            })
            .await
            .unwrap();
        assert_eq!(id, 200);
    }
}
