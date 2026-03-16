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

// Product / field / user types

#[derive(Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub versions: Vec<Version>,
    #[serde(default)]
    pub milestones: Vec<Milestone>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub default_assignee: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Milestone {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldValue {
    pub name: String,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub can_change_to: Option<Vec<StatusTransition>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusTransition {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BugzillaUser {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub groups: Vec<UserGroup>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserGroup {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

// Whoami type

#[derive(Debug, Serialize, Deserialize)]
pub struct WhoamiResponse {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub real_name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
}

// Bug history types

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub who: String,
    pub when: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldChange {
    pub field_name: String,
    #[serde(default)]
    pub removed: String,
    #[serde(default)]
    pub added: String,
    #[serde(default)]
    pub attachment_id: Option<u64>,
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
struct ProductAccessibleResponse {
    ids: Vec<u64>,
}

#[derive(Deserialize)]
struct ProductResponse {
    products: Vec<Product>,
}

#[derive(Deserialize)]
struct FieldBugResponse {
    fields: Vec<FieldEntry>,
}

#[derive(Deserialize)]
struct FieldEntry {
    values: Vec<FieldValue>,
}

#[derive(Deserialize)]
struct UserSearchResponse {
    users: Vec<BugzillaUser>,
}

#[derive(Deserialize)]
struct HistoryResponse {
    bugs: Vec<HistoryBugEntry>,
}

#[derive(Deserialize)]
struct HistoryBugEntry {
    history: Vec<HistoryEntry>,
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

    pub async fn get_bug_history(&self, id: u64) -> Result<Vec<HistoryEntry>> {
        let req = self.auth(self.http.get(self.url(&format!("bug/{id}/history"))));
        let resp = self.send(req).await?;
        let data: HistoryResponse = self.parse_json(resp).await?;
        Ok(data
            .bugs
            .into_iter()
            .next()
            .map(|b| b.history)
            .unwrap_or_default())
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

    pub async fn list_products(&self) -> Result<Vec<Product>> {
        let req = self.auth(self.http.get(self.url("product_accessible")));
        let resp = self.send(req).await?;
        let accessible: ProductAccessibleResponse = self.parse_json(resp).await?;

        if accessible.ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_products = Vec::new();
        for chunk in accessible.ids.chunks(50) {
            let id_params: Vec<(&str, String)> =
                chunk.iter().map(|id| ("ids", id.to_string())).collect();
            let req = self.auth(self.http.get(self.url("product")).query(&id_params));
            let resp = self.send(req).await?;
            let data: ProductResponse = self.parse_json(resp).await?;
            all_products.extend(data.products);
        }
        Ok(all_products)
    }

    /// Fetch a product by name. Note: components, versions, and milestones
    /// may require `include_fields` on some Bugzilla versions to be populated.
    pub async fn get_product(&self, name: &str) -> Result<Product> {
        let req = self.auth(self.http.get(self.url("product")).query(&[("names", name)]));
        let resp = self.send(req).await?;
        let data: ProductResponse = self.parse_json(resp).await?;
        data.products
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::Other(format!("product '{name}' not found")))
    }

    /// Fetch legal values for a bug field. An unrecognized field name returns
    /// an empty list, indistinguishable from a field with no values.
    pub async fn get_field_values(&self, field_name: &str) -> Result<Vec<FieldValue>> {
        let req = self.auth(self.http.get(self.url(&format!("field/bug/{field_name}"))));
        let resp = self.send(req).await?;
        let data: FieldBugResponse = self.parse_json(resp).await?;
        Ok(data
            .fields
            .into_iter()
            .next()
            .map(|f| f.values)
            .unwrap_or_default())
    }

    pub async fn search_users(&self, query: &str) -> Result<Vec<BugzillaUser>> {
        let req = self.auth(self.http.get(self.url("user")).query(&[("match", query)]));
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        Ok(data.users)
    }

    pub async fn get_group_members(&self, group_name: &str) -> Result<Vec<BugzillaUser>> {
        let req = self.auth(self.http.get(self.url("user")).query(&[
            ("group", group_name),
            ("include_fields", "id,name,real_name,email,groups"),
        ]));
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        Ok(data.users)
    }

    pub async fn add_user_to_group(&self, user: &str, group: &str) -> Result<()> {
        let body = serde_json::json!({
            "groups": {
                "add": [group]
            }
        });
        let req = self.auth(self.http.put(self.url(&format!("user/{user}"))).json(&body));
        self.send(req).await?;
        Ok(())
    }

    pub async fn remove_user_from_group(&self, user: &str, group: &str) -> Result<()> {
        let body = serde_json::json!({
            "groups": {
                "remove": [group]
            }
        });
        let req = self.auth(self.http.put(self.url(&format!("user/{user}"))).json(&body));
        self.send(req).await?;
        Ok(())
    }

    pub async fn whoami(&self) -> Result<WhoamiResponse> {
        let req = self.auth(self.http.get(self.url("whoami")));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::EnvFilter;
    use wiremock::matchers::{body_json, method, path, query_param};
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
        let filter = EnvFilter::new("bzr=debug");
        let subscriber = tracing_subscriber::registry().with(fmt_layer).with(filter);

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
        let filter = EnvFilter::new("bzr=debug");
        let subscriber = tracing_subscriber::registry().with(fmt_layer).with(filter);

        let _guard = tracing::subscriber::set_default(subscriber);

        let _bug = client.get_bug(42).await.unwrap();

        let output = String::from_utf8(log_buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("/rest/bug/42"),
            "logged URL should contain path: {output}"
        );
    }

    fn test_client(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::Header).unwrap()
    }

    #[tokio::test]
    async fn list_products_returns_products() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product_accessible"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, 2]})),
            )
            .mount(&mock)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/product"))
            .and(query_param("ids", "1"))
            .and(query_param("ids", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "products": [
                    {"id": 1, "name": "Widget", "description": "A widget", "is_active": true, "components": [], "versions": [], "milestones": []},
                    {"id": 2, "name": "Gadget", "description": "A gadget", "is_active": true, "components": [], "versions": [], "milestones": []}
                ]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let products = client.list_products().await.unwrap();
        assert_eq!(products.len(), 2);
        assert_eq!(products[0].name, "Widget");
        assert_eq!(products[1].name, "Gadget");
    }

    #[tokio::test]
    async fn list_products_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product_accessible"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": []})))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let products = client.list_products().await.unwrap();
        assert!(products.is_empty());
    }

    #[tokio::test]
    async fn get_product_by_name() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "products": [{
                    "id": 1,
                    "name": "Widget",
                    "description": "A widget",
                    "is_active": true,
                    "components": [{"id": 10, "name": "Backend", "description": "", "is_active": true}],
                    "versions": [{"id": 20, "name": "1.0", "sort_key": 0, "is_active": true}],
                    "milestones": [{"id": 30, "name": "M1", "sort_key": 0, "is_active": true}]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let product = client.get_product("Widget").await.unwrap();
        assert_eq!(product.name, "Widget");
        assert_eq!(product.components.len(), 1);
        assert_eq!(product.components[0].name, "Backend");
        assert_eq!(product.versions.len(), 1);
        assert_eq!(product.milestones.len(), 1);
    }

    #[tokio::test]
    async fn get_product_not_found() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"products": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_product("NoSuch").await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

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
    async fn get_field_values_empty_fields() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/field/bug/priority"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"fields": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let values = client.get_field_values("priority").await.unwrap();
        assert!(values.is_empty());
    }

    #[tokio::test]
    async fn search_users_returns_matches() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {"id": 1, "name": "alice@example.com", "real_name": "Alice", "email": "alice@example.com"},
                    {"id": 2, "name": "bob@example.com", "real_name": "Bob", "email": "bob@example.com"}
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("example").await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "alice@example.com");
        assert_eq!(users[1].real_name.as_deref(), Some("Bob"));
    }

    #[tokio::test]
    async fn search_users_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"users": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("nobody").await.unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn api_error_with_200_status() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 301,
                "message": "You are not authorized to access that product."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_product("Secret").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("301"), "expected error code 301: {msg}");
        assert!(
            msg.contains("not authorized"),
            "expected auth error message: {msg}"
        );
    }

    #[tokio::test]
    async fn http_500_returns_error() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.search_users("anyone").await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("500") || msg.contains("Internal Server Error"),
            "expected 500 error: {msg}"
        );
    }

    #[tokio::test]
    async fn get_bug_history_returns_entries() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "alias": [],
                    "history": [
                        {
                            "who": "alice@example.com",
                            "when": "2025-01-15T10:30:00Z",
                            "changes": [
                                {
                                    "field_name": "status",
                                    "removed": "NEW",
                                    "added": "ASSIGNED"
                                },
                                {
                                    "field_name": "assigned_to",
                                    "removed": "",
                                    "added": "alice@example.com"
                                }
                            ]
                        },
                        {
                            "who": "bob@example.com",
                            "when": "2025-01-16T14:00:00Z",
                            "changes": [
                                {
                                    "field_name": "status",
                                    "removed": "ASSIGNED",
                                    "added": "RESOLVED"
                                }
                            ]
                        }
                    ]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client.get_bug_history(42).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].who, "alice@example.com");
        assert_eq!(history[0].changes.len(), 2);
        assert_eq!(history[0].changes[0].field_name, "status");
        assert_eq!(history[0].changes[0].removed, "NEW");
        assert_eq!(history[0].changes[0].added, "ASSIGNED");
        assert_eq!(history[1].changes.len(), 1);
    }

    #[tokio::test]
    async fn get_bug_history_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/99/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{"id": 99, "alias": [], "history": []}]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client.get_bug_history(99).await.unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn get_bug_history_with_attachment_id() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/10/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 10,
                    "alias": [],
                    "history": [{
                        "who": "carol@example.com",
                        "when": "2025-02-01T09:00:00Z",
                        "changes": [{
                            "field_name": "attachments.isobsolete",
                            "removed": "0",
                            "added": "1",
                            "attachment_id": 555
                        }]
                    }]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client.get_bug_history(10).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].changes[0].attachment_id, Some(555));
    }

    #[tokio::test]
    async fn get_group_members_returns_users() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "admin"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "id": 1,
                        "name": "alice@example.com",
                        "real_name": "Alice",
                        "email": "alice@example.com",
                        "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                    },
                    {
                        "id": 2,
                        "name": "bob@example.com",
                        "real_name": "Bob",
                        "email": "bob@example.com",
                        "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("admin").await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "alice@example.com");
        assert_eq!(users[0].groups.len(), 1);
        assert_eq!(users[0].groups[0].name, "admin");
    }

    #[tokio::test]
    async fn get_group_members_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "nobody"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"users": []})),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("nobody").await.unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn add_user_to_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/user/alice@example.com"))
            .and(body_json(
                serde_json::json!({"groups": {"add": ["testers"]}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 1, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        client
            .add_user_to_group("alice@example.com", "testers")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remove_user_from_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/user/bob@example.com"))
            .and(body_json(
                serde_json::json!({"groups": {"remove": ["testers"]}}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 2, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        client
            .remove_user_from_group("bob@example.com", "testers")
            .await
            .unwrap();
    }

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
        let who = client.whoami().await.unwrap();
        assert_eq!(who.id, 42);
        assert_eq!(who.name, "alice@example.com");
        assert_eq!(who.real_name.as_deref(), Some("Alice"));
    }
}
