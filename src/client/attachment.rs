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

fn extract_bugs_envelope(value: &serde_json::Value) -> Result<Vec<Attachment>> {
    let resp = AttachmentBugResponse::deserialize(value)
        .map_err(|e| BzrError::Deserialize(format!("attachments `bugs` envelope: {e}")))?;
    // Treat a structurally empty `bugs` map as a non-match so try_envelopes
    // falls through to the flat extractor. `bugs: {"42": []}` (bug acknowledged,
    // no attachments) is a legitimate empty result and still returns Ok(vec![]).
    resp.bugs.into_values().next().ok_or_else(|| {
        BzrError::Deserialize("attachments `bugs` envelope: empty top-level map".into())
    })
}

fn extract_flat_envelope(value: &serde_json::Value) -> Result<Vec<Attachment>> {
    let resp = FlatAttachmentsResponse::deserialize(value)
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
#[path = "attachment_tests.rs"]
mod tests;
