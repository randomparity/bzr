use std::time::Duration;

use base64::Engine;
use reqwest::header::HeaderValue;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::config::AuthMethod;
use crate::error::{BzrError, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

enum AuthConfig {
    Header(HeaderValue),
    QueryParam(String),
}

pub struct BugzillaClient {
    http: reqwest::Client,
    base_url: String,
    auth: AuthConfig,
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
    pub fn new(base_url: &str, api_key: &str, auth_method: AuthMethod) -> Result<Self> {
        let auth = match auth_method {
            AuthMethod::Header => {
                let value = HeaderValue::from_str(api_key)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                AuthConfig::Header(value)
            }
            AuthMethod::QueryParam => AuthConfig::QueryParam(api_key.to_string()),
        };

        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| BzrError::Other(format!("failed to build HTTP client: {e}")))?;

        tracing::debug!(base_url, %auth_method, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/rest/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            AuthConfig::Header(value) => builder.header("X-BUGZILLA-API-KEY", value.clone()),
            AuthConfig::QueryParam(key) => builder.query(&[("Bugzilla_api_key", key)]),
        }
    }

    async fn send(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
        let resp = builder.send().await?;
        tracing::debug!(
            url = Self::safe_url(resp.url()),
            status = %resp.status(),
            "API response"
        );
        self.check_error(resp).await
    }

    fn safe_url(url: &reqwest::Url) -> String {
        format!("{}{}", url.origin().ascii_serialization(), url.path())
    }

    async fn parse_json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T> {
        let safe_url = Self::safe_url(resp.url());
        let body = resp.text().await?;

        // Check for Bugzilla error responses that arrive with 200 status
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            if err.error {
                return Err(BzrError::Api {
                    code: err.code,
                    message: err.message.unwrap_or_else(|| "unknown API error".into()),
                });
            }
        }

        serde_json::from_str(&body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = &body[..body.len().min(512)],
                "JSON deserialization failed"
            );
            BzrError::Other(format!("failed to parse response from {safe_url}: {e}"))
        })
    }

    async fn check_error(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let preview = if body.len() > 512 {
                &body[..512]
            } else {
                &body
            };
            tracing::debug!(%status, body = preview, "API error response");
            if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                if err.error {
                    return Err(BzrError::Api {
                        code: err.code,
                        message: err.message.unwrap_or_else(|| status.to_string()),
                    });
                }
            }
            return Err(BzrError::Other(format!("HTTP {status}: {body}")));
        }
        Ok(response)
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let req = self.auth(self.http.get(self.url("bug")).query(params));
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        Ok(data.bugs)
    }

    pub async fn get_bug(&self, id: u64) -> Result<Bug> {
        let req = self.auth(self.http.get(self.url(&format!("bug/{id}"))));
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        data.bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::Other(format!("bug {id} not found")))
    }

    pub async fn create_bug(&self, params: &CreateBugParams) -> Result<u64> {
        let req = self.auth(self.http.post(self.url("bug")).json(params));
        let resp = self.send(req).await?;
        let data: BugCreateResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_bug(&self, id: u64, params: &UpdateBugParams) -> Result<()> {
        let req = self.auth(self.http.put(self.url(&format!("bug/{id}"))).json(params));
        self.send(req).await?;
        Ok(())
    }

    pub async fn get_comments(&self, bug_id: u64) -> Result<Vec<Comment>> {
        let req = self.auth(self.http.get(self.url(&format!("bug/{bug_id}/comment"))));
        let resp = self.send(req).await?;
        let data: CommentResponse = self.parse_json(resp).await?;
        let comments = data
            .bugs
            .into_values()
            .next()
            .map(|e| e.comments)
            .unwrap_or_default();
        Ok(comments)
    }

    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<Attachment>> {
        let req = self.auth(self.http.get(self.url(&format!("bug/{bug_id}/attachment"))));
        let resp = self.send(req).await?;
        let data: AttachmentBugResponse = self.parse_json(resp).await?;
        let attachments = data.bugs.into_values().next().unwrap_or_default();
        Ok(attachments)
    }

    pub async fn get_attachment(&self, attachment_id: u64) -> Result<Attachment> {
        let req = self.auth(
            self.http
                .get(self.url(&format!("bug/attachment/{attachment_id}"))),
        );
        let resp = self.send(req).await?;
        let data: AttachmentByIdResponse = self.parse_json(resp).await?;
        data.attachments
            .into_values()
            .next()
            .ok_or_else(|| BzrError::Other(format!("attachment {attachment_id} not found")))
    }

    pub async fn download_attachment(&self, attachment_id: u64) -> Result<(String, Vec<u8>)> {
        let attachment = self.get_attachment(attachment_id).await?;
        let data = attachment
            .data
            .ok_or_else(|| BzrError::Other("attachment has no data".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| BzrError::Other(format!("failed to decode attachment: {e}")))?;
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
        let req = self.auth(
            self.http
                .post(self.url(&format!("bug/{bug_id}/attachment")))
                .json(&body),
        );
        let resp = self.send(req).await?;
        let data: AttachmentCreateResponse = self.parse_json(resp).await?;
        data.ids
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::Other("no attachment ID returned".into()))
    }

    pub async fn add_comment(&self, bug_id: u64, text: &str) -> Result<u64> {
        let body = serde_json::json!({ "comment": text });
        let req = self.auth(
            self.http
                .post(self.url(&format!("bug/{bug_id}/comment")))
                .json(&body),
        );
        let resp = self.send(req).await?;
        let data: CommentCreateResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::layer::SubscriberExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::config::AuthMethod;

    /// Tracing layer that captures formatted log output into a shared buffer.
    struct CapturingWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CapturingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturingWriter {
        type Writer = CapturingWriter;

        fn make_writer(&'a self) -> Self::Writer {
            CapturingWriter(Arc::clone(&self.0))
        }
    }

    #[tokio::test]
    async fn send_does_not_log_query_param_api_key() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
            ))
            .mount(&mock)
            .await;

        let secret = "super-secret-api-key-12345";
        let client = BugzillaClient::new(&mock.uri(), secret, AuthMethod::QueryParam).unwrap();

        let log_buf = Arc::new(Mutex::new(Vec::new()));
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(CapturingWriter(Arc::clone(&log_buf)))
            .with_ansi(false);
        let subscriber = tracing_subscriber::registry().with(fmt_layer);

        let _guard = tracing::subscriber::set_default(subscriber);

        let _bug = client.get_bug(1).await.unwrap();

        let output = String::from_utf8(log_buf.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains(secret),
            "API key leaked in log output: {output}"
        );
    }

    #[tokio::test]
    async fn logged_url_contains_path() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"bugs": [{"id": 42, "summary": "test", "status": "NEW"}]}),
            ))
            .mount(&mock)
            .await;

        let client = BugzillaClient::new(&mock.uri(), "key", AuthMethod::Header).unwrap();

        let log_buf = Arc::new(Mutex::new(Vec::new()));
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(CapturingWriter(Arc::clone(&log_buf)))
            .with_ansi(false);
        let subscriber = tracing_subscriber::registry().with(fmt_layer);

        let _guard = tracing::subscriber::set_default(subscriber);

        let _bug = client.get_bug(42).await.unwrap();

        let output = String::from_utf8(log_buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("/rest/bug/42"),
            "logged URL should contain path: {output}"
        );
    }
}
