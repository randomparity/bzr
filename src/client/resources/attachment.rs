use std::fmt;

use base64::Engine;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
use crate::types::deserialization::u64_from_number_or_string;
use crate::types::transport::ApiMode;

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

/// Select an attachment from the by-ID response envelopes returned by
/// different Bugzilla versions.
fn select_attachment(value: &serde_json::Value, attachment_id: u64) -> Result<Attachment> {
    let attachments = value.get("attachments").ok_or_else(|| {
        BzrError::Deserialize("attachment by-ID response: missing `attachments` member".into())
    })?;
    let not_found = || BzrError::NotFound {
        resource: "attachment",
        id: attachment_id.to_string(),
    };

    match attachments {
        serde_json::Value::Object(attachments) => {
            let mut attachments = std::collections::HashMap::<String, Attachment>::deserialize(
                attachments,
            )
            .map_err(|error| {
                BzrError::Deserialize(format!(
                    "attachment by-ID `attachments` object entry: {error}"
                ))
            })?;
            let attachment = attachments
                .remove(&attachment_id.to_string())
                .ok_or_else(not_found)?;
            if attachment.id != attachment_id {
                return Err(BzrError::Deserialize(format!(
                    "attachment by-ID `attachments` object key {attachment_id} contains ID {}",
                    attachment.id
                )));
            }
            Ok(attachment)
        }
        serde_json::Value::Array(attachments) => {
            let attachments = attachments
                .iter()
                .map(Attachment::deserialize)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| {
                    BzrError::Deserialize(format!("attachment by-ID `attachments` array: {error}"))
                })?;
            attachments
                .into_iter()
                .find(|attachment| attachment.id == attachment_id)
                .ok_or_else(not_found)
        }
        _ => Err(BzrError::Deserialize(
            "attachment by-ID response: `attachments` must be an object or array".into(),
        )),
    }
}

#[derive(Deserialize)]
struct AttachmentCreateResponse {
    #[serde(deserialize_with = "deserialize_attachment_ids")]
    ids: Vec<u64>,
}

fn deserialize_attachment_ids<'de, D>(deserializer: D) -> std::result::Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct AttachmentIdsVisitor;

    impl<'de> Visitor<'de> for AttachmentIdsVisitor {
        type Value = Vec<u64>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a sequence of attachment IDs")
        }

        fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            struct AttachmentId(u64);

            impl<'de> Deserialize<'de> for AttachmentId {
                fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    u64_from_number_or_string(
                        deserializer,
                        "an attachment ID as a number or string",
                        "attachment ID must be a non-negative integer or decimal string",
                    )
                    .map(Self)
                }
            }

            let mut ids = Vec::new();
            while let Some(AttachmentId(id)) = sequence.next_element()? {
                ids.push(id);
            }
            Ok(ids)
        }
    }

    deserializer.deserialize_seq(AttachmentIdsVisitor)
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
    /// In Hybrid mode, attachments are fetched via XML-RPC
    /// `Bug.attachments` rather than REST. Private attachments come
    /// back over either protocol whenever the server honoured the
    /// credential; what varies is whether it did. Bugzilla 5.0 and 5.2
    /// ignore the `X-BUGZILLA-API-KEY` header on REST and answer
    /// anonymously with `200` and the private attachments removed
    /// (issues #133, #714). XML-RPC carries the key in the request
    /// body, so it is unaffected. REST is the fallback when the server
    /// doesn't expose `xmlrpc.cgi`. The REST arm also omits `data` by
    /// design (`exclude_fields`) — a payload optimisation, not
    /// filtering. ADR-0059 records the per-version measurement.
    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<Attachment>> {
        self.dispatch_xmlrpc_first(
            &format!("attachment list (bug {bug_id})"),
            || self.get_attachments_rest(bug_id),
            || async { self.xmlrpc_client().get_attachments(bug_id).await },
        )
        .await
    }

    async fn get_attachments_rest(&self, bug_id: u64) -> Result<Vec<Attachment>> {
        let value = self
            .get_json_query(
                &format!("bug/{bug_id}/attachment"),
                &[("exclude_fields", "data")],
            )
            .await?;
        Self::try_envelopes(
            &value,
            &[
                ("bugs", extract_bugs_envelope),
                ("attachments", extract_flat_envelope),
            ],
        )
    }

    /// Like `get_attachments`, dispatches on `api_mode`. Unlike the
    /// list read, `GET /rest/bug/attachment/<id>` answers `401` rather
    /// than a filtered `200` when the request is unauthenticated, so
    /// the transport's auth-method fallback already recovers a private
    /// attachment from a server that ignores the configured auth
    /// method (issues #133, #714).
    ///
    /// That last sentence rests on ADR-0059's measurement, not on a
    /// functional case: the harness pins `query_param` auth, so no
    /// `401` occurs there and the fallback never fires. A change to
    /// `retry_with_alternate_auth` could falsify it with nothing
    /// turning red.
    pub async fn get_attachment(&self, attachment_id: u64) -> Result<Attachment> {
        self.dispatch_xmlrpc_first(
            &format!("attachment fetch (id {attachment_id})"),
            || self.get_attachment_rest(attachment_id),
            || async {
                self.xmlrpc_client()
                    .get_attachment_by_id(attachment_id)
                    .await
            },
        )
        .await
    }

    async fn get_attachment_rest(&self, attachment_id: u64) -> Result<Attachment> {
        let value = self
            .get_json_value(&format!("bug/attachment/{attachment_id}"))
            .await?;
        select_attachment(&value, attachment_id)
    }

    /// Fetch a single attachment's metadata without its (base64) bytes.
    ///
    /// On REST the `data` field is excluded server-side via `exclude_fields`
    /// so the bytes never cross the wire. XML-RPC has no cheap field
    /// exclusion, so the full record is fetched. Either way the bytes are
    /// dropped locally before returning, so the metadata-only guarantee holds
    /// even if a server ignores `exclude_fields`.
    pub async fn get_attachment_metadata(&self, attachment_id: u64) -> Result<Attachment> {
        let mut attachment = match self.api_mode {
            ApiMode::Rest => self.get_attachment_metadata_rest(attachment_id).await?,
            ApiMode::XmlRpc | ApiMode::Hybrid => self.get_attachment(attachment_id).await?,
        };
        attachment.data = None;
        Ok(attachment)
    }

    async fn get_attachment_metadata_rest(&self, attachment_id: u64) -> Result<Attachment> {
        let value = self
            .get_json_query(
                &format!("bug/attachment/{attachment_id}"),
                &[("exclude_fields", "data")],
            )
            .await?;
        select_attachment(&value, attachment_id)
    }

    pub async fn download_attachment(&self, attachment_id: u64) -> Result<(String, Vec<u8>)> {
        let attachment = self.get_attachment(attachment_id).await?;
        let file_name = attachment.file_name.ok_or_else(|| {
            BzrError::DataIntegrity(format!("attachment #{attachment_id} has no file_name"))
        })?;
        let data = attachment
            .data
            .ok_or_else(|| BzrError::DataIntegrity("attachment has no data".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| BzrError::DataIntegrity(format!("failed to decode attachment: {e}")))?;
        Ok((file_name, bytes))
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
