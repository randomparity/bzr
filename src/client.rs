use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::error::{BzrError, Result};

pub struct BugzillaClient {
    http: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bug {
    pub id: u64,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub assigned_to: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub last_change_time: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub whiteboard: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<u64>,
    #[serde(default)]
    pub depends_on: Vec<u64>,
    #[serde(default)]
    pub cc: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    #[serde(default)]
    pub bug_id: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub is_private: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct SearchParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quicksearch: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateBugParams {
    pub product: String,
    pub component: String,
    pub summary: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct UpdateBugParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
}

// API response wrappers
#[derive(Deserialize)]
struct BugListResponse {
    bugs: Vec<Bug>,
}

#[derive(Deserialize)]
struct BugCreateResponse {
    id: u64,
}

#[derive(Deserialize)]
struct CommentResponse {
    bugs: std::collections::HashMap<String, CommentBugEntry>,
}

#[derive(Deserialize)]
struct CommentBugEntry {
    comments: Vec<Comment>,
}

#[derive(Deserialize)]
struct CommentCreateResponse {
    id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: u64,
    #[serde(default)]
    pub bug_id: u64,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creation_time: Option<String>,
    #[serde(default)]
    pub last_change_time: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub is_obsolete: bool,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub data: Option<String>,
}

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

#[derive(Deserialize)]
struct ErrorResponse {
    #[serde(default)]
    error: bool,
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: Option<String>,
}

impl BugzillaClient {
    pub fn new(base_url: &str, api_key: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let key_val = HeaderValue::from_str(api_key)
            .map_err(|_| BzrError::config("invalid API key characters"))?;
        headers.insert("X-BUGZILLA-API-KEY", key_val);

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/rest/{}", self.base_url, path.trim_start_matches('/'))
    }

    async fn check_error(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                if err.error {
                    return Err(BzrError::Api {
                        code: err.code,
                        message: err.message.unwrap_or_else(|| status.to_string()),
                    });
                }
            }
            return Err(BzrError::Other(format!("HTTP {}: {}", status, body)));
        }
        Ok(response)
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let resp = self.http.get(self.url("bug")).query(params).send().await?;
        let resp = self.check_error(resp).await?;
        let data: BugListResponse = resp.json().await?;
        Ok(data.bugs)
    }

    pub async fn get_bug(&self, id: u64) -> Result<Bug> {
        let resp = self.http.get(self.url(&format!("bug/{}", id))).send().await?;
        let resp = self.check_error(resp).await?;
        let data: BugListResponse = resp.json().await?;
        data.bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::Other(format!("bug {} not found", id)))
    }

    pub async fn create_bug(&self, params: &CreateBugParams) -> Result<u64> {
        let resp = self.http.post(self.url("bug")).json(params).send().await?;
        let resp = self.check_error(resp).await?;
        let data: BugCreateResponse = resp.json().await?;
        Ok(data.id)
    }

    pub async fn update_bug(&self, id: u64, params: &UpdateBugParams) -> Result<()> {
        let resp = self
            .http
            .put(self.url(&format!("bug/{}", id)))
            .json(params)
            .send()
            .await?;
        self.check_error(resp).await?;
        Ok(())
    }

    pub async fn get_comments(&self, bug_id: u64) -> Result<Vec<Comment>> {
        let resp = self
            .http
            .get(self.url(&format!("bug/{}/comment", bug_id)))
            .send()
            .await?;
        let resp = self.check_error(resp).await?;
        let data: CommentResponse = resp.json().await?;
        let comments = data
            .bugs
            .into_values()
            .next()
            .map(|e| e.comments)
            .unwrap_or_default();
        Ok(comments)
    }

    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<Attachment>> {
        let resp = self
            .http
            .get(self.url(&format!("bug/{}/attachment", bug_id)))
            .send()
            .await?;
        let resp = self.check_error(resp).await?;
        let data: AttachmentBugResponse = resp.json().await?;
        let attachments = data
            .bugs
            .into_values()
            .next()
            .unwrap_or_default();
        Ok(attachments)
    }

    pub async fn get_attachment(&self, attachment_id: u64) -> Result<Attachment> {
        let resp = self
            .http
            .get(self.url(&format!("bug/attachment/{}", attachment_id)))
            .send()
            .await?;
        let resp = self.check_error(resp).await?;
        let data: AttachmentByIdResponse = resp.json().await?;
        data.attachments
            .into_values()
            .next()
            .ok_or_else(|| BzrError::Other(format!("attachment {} not found", attachment_id)))
    }

    pub async fn download_attachment(&self, attachment_id: u64) -> Result<(String, Vec<u8>)> {
        let attachment = self.get_attachment(attachment_id).await?;
        let data = attachment
            .data
            .ok_or_else(|| BzrError::Other("attachment has no data".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| BzrError::Other(format!("failed to decode attachment: {}", e)))?;
        Ok((attachment.file_name, bytes))
    }

    pub async fn upload_attachment(
        &self,
        bug_id: u64,
        file_name: &str,
        summary: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<u64> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let body = serde_json::json!({
            "ids": [bug_id],
            "file_name": file_name,
            "summary": summary,
            "content_type": content_type,
            "data": encoded,
        });
        let resp = self
            .http
            .post(self.url(&format!("bug/{}/attachment", bug_id)))
            .json(&body)
            .send()
            .await?;
        let resp = self.check_error(resp).await?;
        let data: AttachmentCreateResponse = resp.json().await?;
        data.ids
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::Other("no attachment ID returned".into()))
    }

    pub async fn add_comment(&self, bug_id: u64, text: &str) -> Result<u64> {
        let body = serde_json::json!({ "comment": text });
        let resp = self
            .http
            .post(self.url(&format!("bug/{}/comment", bug_id)))
            .json(&body)
            .send()
            .await?;
        let resp = self.check_error(resp).await?;
        let data: CommentCreateResponse = resp.json().await?;
        Ok(data.id)
    }
}
