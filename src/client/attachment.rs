use base64::Engine;
use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};

#[derive(Deserialize)]
struct AttachmentBugResponse {
    bugs: std::collections::HashMap<String, Vec<Attachment>>,
}

/// Flat envelope variant: `{"attachments": [...]}` at the root of the
/// response. Observed on some Bugzilla 5.0.x deployments (issue #135).
#[derive(Deserialize)]
struct FlatAttachmentsResponse {
    attachments: Vec<Attachment>,
}

#[derive(Deserialize)]
struct AttachmentByIdResponse {
    attachments: std::collections::HashMap<String, Attachment>,
}

#[derive(Deserialize)]
struct AttachmentCreateResponse {
    ids: Vec<u64>,
}

fn extract_bugs_envelope(value: serde_json::Value) -> Result<Vec<Attachment>> {
    let resp: AttachmentBugResponse = serde_json::from_value(value)
        .map_err(|e| BzrError::Deserialize(format!("attachments `bugs` envelope: {e}")))?;
    // Treat a structurally empty `bugs` map as a non-match so try_envelopes
    // falls through to the flat extractor. `bugs: {"42": []}` (bug acknowledged,
    // no attachments) is a legitimate empty result and still returns Ok(vec![]).
    resp.bugs.into_values().next().ok_or_else(|| {
        BzrError::Deserialize("attachments `bugs` envelope: empty top-level map".into())
    })
}

fn extract_flat_envelope(value: serde_json::Value) -> Result<Vec<Attachment>> {
    let resp: FlatAttachmentsResponse = serde_json::from_value(value)
        .map_err(|e| BzrError::Deserialize(format!("attachments flat envelope: {e}")))?;
    Ok(resp.attachments)
}

impl BugzillaClient {
    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<Attachment>> {
        let value = self
            .get_json_value(&format!("bug/{bug_id}/attachment"))
            .await?;
        Self::try_envelopes(
            &value,
            &[
                ("bugs", extract_bugs_envelope),
                ("attachments", extract_flat_envelope),
            ],
        )
    }

    pub async fn get_attachment(&self, attachment_id: u64) -> Result<Attachment> {
        let data: AttachmentByIdResponse = self
            .get_json(&format!("bug/attachment/{attachment_id}"))
            .await?;
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
        self.put_json(&format!("bug/attachment/{id}"), updates)
            .await
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

    #[tokio::test]
    async fn get_attachments_accepts_bugs_envelope() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "42": [
                        {
                            "id": 100,
                            "bug_id": 42,
                            "file_name": "patch.diff",
                            "summary": "test patch",
                            "content_type": "text/plain",
                            "creator": "alice@example.com",
                            "creation_time": "2026-01-01T00:00:00Z",
                            "last_change_time": "2026-01-01T00:00:00Z",
                            "size": 100,
                            "is_obsolete": false,
                            "is_patch": true,
                            "is_private": false
                        }
                    ]
                },
                "attachments": {}
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let attachments = client.get_attachments(42).await.unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].id, 100);
        assert_eq!(attachments[0].file_name, "patch.diff");
    }

    #[tokio::test]
    async fn get_attachments_accepts_flat_attachments_envelope() {
        // Some Bugzilla 5.0.x deployments (e.g. IBM LTC) return only an
        // `attachments` array, no `bugs` key. Issue #135.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "attachments": [
                    {
                        "id": 200,
                        "bug_id": 42,
                        "file_name": "alt.diff",
                        "summary": "alt-envelope patch",
                        "content_type": "text/plain",
                        "creator": "bob@example.com",
                        "creation_time": "2026-01-01T00:00:00Z",
                        "last_change_time": "2026-01-01T00:00:00Z",
                        "size": 50,
                        "is_obsolete": false,
                        "is_patch": true,
                        "is_private": false
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let attachments = client.get_attachments(42).await.unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].id, 200);
        assert_eq!(attachments[0].file_name, "alt.diff");
    }

    #[tokio::test]
    async fn get_attachments_prefers_bugs_when_both_envelopes_populated() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "42": [
                        {
                            "id": 100,
                            "bug_id": 42,
                            "file_name": "from_bugs.diff",
                            "summary": "from bugs",
                            "content_type": "text/plain",
                            "creator": "alice@example.com",
                            "creation_time": "2026-01-01T00:00:00Z",
                            "last_change_time": "2026-01-01T00:00:00Z",
                            "size": 1,
                            "is_obsolete": false,
                            "is_patch": true,
                            "is_private": false
                        }
                    ]
                },
                "attachments": [
                    {
                        "id": 200,
                        "bug_id": 42,
                        "file_name": "from_attachments.diff",
                        "summary": "from attachments",
                        "content_type": "text/plain",
                        "creator": "bob@example.com",
                        "creation_time": "2026-01-01T00:00:00Z",
                        "last_change_time": "2026-01-01T00:00:00Z",
                        "size": 1,
                        "is_obsolete": false,
                        "is_patch": true,
                        "is_private": false
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let attachments = client.get_attachments(42).await.unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments[0].file_name, "from_bugs.diff",
            "should prefer bugs-keyed envelope when both present"
        );
    }

    #[tokio::test]
    async fn get_attachments_falls_through_when_bugs_map_empty_and_flat_populated() {
        // Regression: an empty top-level `bugs` map (no bug ID acknowledged)
        // alongside a populated flat `attachments` array used to silently
        // return [] because the bugs extractor swallowed the empty case.
        // The bugs extractor now returns Err on an empty top-level map so
        // try_envelopes falls through to the flat extractor.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {},
                "attachments": [
                    {
                        "id": 300,
                        "bug_id": 42,
                        "file_name": "from_flat.diff",
                        "summary": "from flat envelope",
                        "content_type": "text/plain",
                        "creator": "alice@example.com",
                        "creation_time": "2026-01-01T00:00:00Z",
                        "last_change_time": "2026-01-01T00:00:00Z",
                        "size": 1,
                        "is_obsolete": false,
                        "is_patch": true,
                        "is_private": false
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let attachments = client.get_attachments(42).await.unwrap();
        assert_eq!(
            attachments.len(),
            1,
            "should fall through to flat envelope, not return empty"
        );
        assert_eq!(attachments[0].file_name, "from_flat.diff");
    }

    #[tokio::test]
    async fn get_attachments_returns_empty_when_bug_acknowledged_with_no_attachments() {
        // Legitimate empty case: server acknowledges bug 42 but reports no
        // attachments. Must return Ok([]) — NOT fall through to flat (where
        // the empty {} would fail to deserialize as Vec) and NOT error.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {"42": []},
                "attachments": {}
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let attachments = client.get_attachments(42).await.unwrap();
        assert!(
            attachments.is_empty(),
            "no attachments expected: {attachments:?}"
        );
    }
}
