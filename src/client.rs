use base64::Engine;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::header::HeaderValue;
use reqwest::RequestBuilder;
use serde::Deserialize;

use crate::config::{ApiMode, AuthMethod};
use crate::error::{BzrError, Result};
use crate::http::{build_http_client, AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::{
    Attachment, Bug, BugzillaUser, Classification, Comment, CreateBugParams, CreateComponentParams,
    CreateGroupParams, CreateProductParams, CreateUserParams, FieldValue, GroupInfo, HistoryEntry,
    Product, ProductListType, SearchParams, ServerExtensions, ServerVersion,
    UpdateAttachmentParams, UpdateBugParams, UpdateComponentParams, UpdateGroupParams,
    UpdateProductParams, UpdateUserParams, UploadAttachmentParams, WhoamiResponse,
};
use crate::xmlrpc::client::XmlRpcClient;

fn encode_path(segment: &str) -> String {
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
}

/// Default fields requested for Bug queries. Matches the fields in [`Bug`] and
/// avoids requesting server-side fields we don't use — some Bugzilla extensions
/// crash when serializing certain fields (e.g. group visibility) via the REST API.
const BUG_DEFAULT_FIELDS: &str = "id,summary,status,resolution,product,component,\
    assigned_to,priority,severity,creation_time,last_change_time,creator,\
    url,whiteboard,keywords,blocks,depends_on,cc";

enum AuthConfig {
    Header(HeaderValue),
    QueryParam(String),
}

enum GroupOp {
    Add,
    Remove,
}

pub struct BugzillaClient {
    http: reqwest::Client,
    base_url: String,
    auth: AuthConfig,
    api_key: String,
    api_mode: ApiMode,
    xmlrpc: Option<XmlRpcClient>,
}

// API response wrappers
#[derive(Deserialize)]
struct BugListResponse {
    bugs: Vec<Bug>,
}

/// Generic response for endpoints that return a single `id` field.
/// Used by bug creation, comment creation, product/component/user/group creation.
#[derive(Deserialize)]
struct IdResponse {
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
struct ClassificationResponse {
    classifications: Vec<Classification>,
}

#[derive(Deserialize)]
struct GroupResponse {
    groups: Vec<GroupInfo>,
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

/// Bugzilla response keys that indicate real data is present alongside
/// an error payload. When any of these exist, the error is a non-fatal
/// server-side warning (e.g. from a Bugzilla extension) and the data
/// should be used.
const DATA_KEYS: &[&str] = &[
    "bugs",
    "comments",
    "attachments",
    "products",
    "groups",
    "users",
    "fields",
    "extensions",
    "classifications",
    "ids",
];

impl BugzillaClient {
    /// Check if a JSON response body contains known Bugzilla data keys,
    /// indicating the response has real data alongside any error fields.
    fn has_data_fields(body: &str) -> bool {
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(body) else {
            return false;
        };
        let Some(map) = obj.as_object() else {
            return false;
        };
        DATA_KEYS.iter().any(|key| map.contains_key(*key))
    }

    pub fn new(
        base_url: &str,
        api_key: &str,
        auth_method: AuthMethod,
        api_mode: ApiMode,
    ) -> Result<Self> {
        let auth = match auth_method {
            AuthMethod::Header => {
                let value = HeaderValue::from_str(api_key)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                AuthConfig::Header(value)
            }
            AuthMethod::QueryParam => AuthConfig::QueryParam(api_key.to_string()),
        };

        let http = build_http_client().map_err(BzrError::Http)?;

        let xmlrpc = match api_mode {
            ApiMode::Rest => None,
            ApiMode::XmlRpc | ApiMode::Hybrid => {
                Some(XmlRpcClient::new(http.clone(), base_url, api_key))
            }
        };

        tracing::debug!(base_url, %auth_method, %api_mode, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            api_key: api_key.to_string(),
            api_mode,
            xmlrpc,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/rest/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn xmlrpc_client(&self) -> Result<&XmlRpcClient> {
        self.xmlrpc.as_ref().ok_or_else(|| {
            BzrError::Config(
                "XML-RPC client not initialized — set api_mode to 'xmlrpc' or 'hybrid'".into(),
            )
        })
    }

    fn apply_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            AuthConfig::Header(value) => builder.header(AUTH_HEADER_NAME, value.clone()),
            AuthConfig::QueryParam(key) => builder.query(&[(AUTH_QUERY_PARAM, key)]),
        }
    }

    async fn send(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
        let retry_builder = builder.try_clone();
        let resp = builder.send().await?;
        tracing::debug!(
            url = Self::safe_url(resp.url()),
            status = %resp.status(),
            "API response"
        );
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(clone) = retry_builder {
                tracing::debug!("401 received, retrying with alternate auth method");
                let retried = self.apply_alternate_auth(clone)?.send().await?;
                tracing::debug!(
                    url = Self::safe_url(retried.url()),
                    status = %retried.status(),
                    "auth fallback response"
                );
                if !retried.status().is_client_error() && !retried.status().is_server_error() {
                    return Ok(retried);
                }
                tracing::debug!("auth fallback also failed, returning original 401");
            }
        }
        self.check_error(resp).await
    }

    fn apply_alternate_auth(&self, builder: RequestBuilder) -> Result<RequestBuilder> {
        match &self.auth {
            AuthConfig::Header(_) => Ok(builder.query(&[(AUTH_QUERY_PARAM, &self.api_key)])),
            AuthConfig::QueryParam(_) => {
                let value = HeaderValue::from_str(&self.api_key).map_err(|e| {
                    BzrError::Config(format!("API key contains invalid header characters: {e}"))
                })?;
                Ok(builder.header(AUTH_HEADER_NAME, value))
            }
        }
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

        tracing::trace!(
            url = safe_url,
            body = &body[..body.len().min(2048)],
            "response body"
        );

        // Check for Bugzilla error responses that arrive with 200 status.
        // Some servers (e.g. IBM LTC Bugzilla) include error fields alongside
        // valid data — only treat the error as fatal when the response doesn't
        // also contain real data (indicated by common Bugzilla result keys).
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
            if err.error {
                let has_data = Self::has_data_fields(&body);
                tracing::debug!(
                    url = safe_url,
                    code = err.code,
                    message = err.message.as_deref().unwrap_or("unknown"),
                    has_data,
                    "error payload in 200 response"
                );
                if !has_data {
                    return Err(BzrError::Api {
                        code: err.code,
                        message: err.message.unwrap_or_else(|| "unknown API error".into()),
                    });
                }
                tracing::warn!(
                    url = safe_url,
                    "server returned error alongside data; using data"
                );
            }
        }

        serde_json::from_str(&body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = &body[..body.len().min(512)],
                "JSON deserialization failed"
            );
            BzrError::Deserialize(format!("failed to parse response from {safe_url}: {e}"))
        })
    }

    async fn check_error(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::debug!(
                %status,
                body = &body[..body.len().min(512)],
                "API error response"
            );
            if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                if err.error {
                    return Err(BzrError::Api {
                        code: err.code,
                        message: err.message.unwrap_or_else(|| status.to_string()),
                    });
                }
            }
            return Err(BzrError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response)
    }

    pub async fn get_bug_history_since(
        &self,
        id: u64,
        since: Option<&str>,
    ) -> Result<Vec<HistoryEntry>> {
        let mut req_builder = self.http.get(self.url(&format!("bug/{id}/history")));
        if let Some(since) = since {
            req_builder = req_builder.query(&[("new_since", since)]);
        }
        let req = self.apply_auth(req_builder);
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
        tracing::debug!(?params, %self.api_mode, "search parameters");
        match self.api_mode {
            ApiMode::Rest => self.search_bugs_rest(params).await,
            ApiMode::XmlRpc => self.xmlrpc_client()?.search_bugs(params).await,
            ApiMode::Hybrid => {
                // Hybrid search only retries on empty results with active filters,
                // not on REST errors. Unlike get_bug (which retries on HTTP/parse
                // errors), search results are less critical and REST errors likely
                // indicate a server issue that XML-RPC won't solve either.
                let rest_result = self.search_bugs_rest(params).await;
                match rest_result {
                    Ok(ref bugs) if !bugs.is_empty() => rest_result,
                    Ok(_) if params.has_filters() => {
                        tracing::info!(
                            "REST search returned empty with active filters, \
                             retrying via XML-RPC"
                        );
                        self.xmlrpc_client()?.search_bugs(params).await
                    }
                    other => other,
                }
            }
        }
    }

    async fn search_bugs_rest(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let mut req_builder = self.http.get(self.url("bug")).query(params);
        if params.include_fields.is_none() {
            req_builder = req_builder.query(&[("include_fields", BUG_DEFAULT_FIELDS)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        Ok(data.bugs)
    }

    pub async fn get_bug(
        &self,
        id: &str,
        include_fields: Option<&str>,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        match self.api_mode {
            ApiMode::XmlRpc => self.xmlrpc_client()?.get_bug(id).await,
            ApiMode::Hybrid => {
                let rest_result = self.get_bug_rest(id, include_fields, exclude_fields).await;
                match &rest_result {
                    Err(
                        BzrError::Http(_)
                        | BzrError::HttpStatus { .. }
                        | BzrError::Deserialize(_)
                        | BzrError::XmlRpc(_),
                    ) => {
                        tracing::info!("REST bug lookup failed, retrying via XML-RPC");
                        self.xmlrpc_client()?.get_bug(id).await
                    }
                    Err(BzrError::Api { code: 100_500, .. }) => {
                        tracing::info!(
                            "REST bug lookup returned 100500, \
                             retrying via XML-RPC"
                        );
                        self.xmlrpc_client()?.get_bug(id).await
                    }
                    _ => rest_result,
                }
            }
            ApiMode::Rest => self.get_bug_rest(id, include_fields, exclude_fields).await,
        }
    }

    async fn get_bug_rest(
        &self,
        id: &str,
        include_fields: Option<&str>,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        let fields = include_fields.unwrap_or(BUG_DEFAULT_FIELDS);
        let mut req_builder = self
            .http
            .get(self.url(&format!("bug/{id}")))
            .query(&[("include_fields", fields)]);
        if let Some(fields) = exclude_fields {
            req_builder = req_builder.query(&[("exclude_fields", fields)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let result: Result<BugListResponse> = self.parse_json(resp).await;

        // If the direct endpoint fails with a server internal error (100500),
        // retry via the search endpoint (/rest/bug?id=X). Some Bugzilla
        // extensions only hook into the direct lookup path and crash there.
        if let Err(BzrError::Api { code: 100_500, .. }) = &result {
            tracing::debug!("direct bug lookup returned 100500, retrying via search endpoint");
            return self.get_bug_via_search(id, fields, exclude_fields).await;
        }

        result?
            .bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            })
    }

    async fn get_bug_via_search(
        &self,
        id: &str,
        include_fields: &str,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        let mut req_builder = self
            .http
            .get(self.url("bug"))
            .query(&[("id", id), ("include_fields", include_fields)]);
        if let Some(fields) = exclude_fields {
            req_builder = req_builder.query(&[("exclude_fields", fields)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        data.bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            })
    }

    pub async fn create_bug(&self, params: &CreateBugParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("bug")).json(params));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_bug(&self, id: u64, params: &UpdateBugParams) -> Result<()> {
        let req = self.apply_auth(self.http.put(self.url(&format!("bug/{id}"))).json(params));
        self.send(req).await?;
        Ok(())
    }

    pub async fn get_comments_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        let mut req_builder = self.http.get(self.url(&format!("bug/{bug_id}/comment")));
        if let Some(since) = since {
            req_builder = req_builder.query(&[("new_since", since)]);
        }
        let req = self.apply_auth(req_builder);
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

    pub async fn update_comment_tags(
        &self,
        comment_id: u64,
        add: &[String],
        remove: &[String],
    ) -> Result<Vec<String>> {
        let body = serde_json::json!({
            "comment_id": comment_id,
            "add": add,
            "remove": remove,
        });
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("bug/comment/{comment_id}/tags")))
                .json(&body),
        );
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn search_comment_tags(&self, query: &str) -> Result<Vec<String>> {
        let req = self.apply_auth(
            self.http
                .get(self.url(&format!("bug/comment/tags/{}", encode_path(query)))),
        );
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

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
        let encoded = base64::engine::general_purpose::STANDARD.encode(&params.data);
        let body = serde_json::json!({
            "ids": [params.bug_id],
            "file_name": params.file_name,
            "summary": params.summary,
            "content_type": params.content_type,
            "data": encoded,
            "flags": params.flags,
        });
        let req = self.apply_auth(
            self.http
                .post(self.url(&format!("bug/{}/attachment", params.bug_id)))
                .json(&body),
        );
        let resp = self.send(req).await?;
        let data: AttachmentCreateResponse = self.parse_json(resp).await?;
        data.ids
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::DataIntegrity("no attachment ID returned".into()))
    }

    pub async fn add_comment(&self, bug_id: u64, text: &str) -> Result<u64> {
        let body = serde_json::json!({ "comment": text });
        let req = self.apply_auth(
            self.http
                .post(self.url(&format!("bug/{bug_id}/comment")))
                .json(&body),
        );
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn list_products_by_type(
        &self,
        product_type: ProductListType,
    ) -> Result<Vec<Product>> {
        let endpoint = product_type.as_endpoint();
        let req = self.apply_auth(self.http.get(self.url(endpoint)));
        let resp = self.send(req).await?;
        let accessible: ProductAccessibleResponse = self.parse_json(resp).await?;

        if accessible.ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_products = Vec::new();
        for chunk in accessible.ids.chunks(50) {
            let id_params: Vec<(&str, String)> =
                chunk.iter().map(|id| ("ids", id.to_string())).collect();
            let req = self.apply_auth(self.http.get(self.url("product")).query(&id_params));
            let resp = self.send(req).await?;
            let data: ProductResponse = self.parse_json(resp).await?;
            all_products.extend(data.products);
        }
        Ok(all_products)
    }

    pub async fn create_product(&self, params: &CreateProductParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("product")).json(params));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_product(&self, name: &str, updates: &UpdateProductParams) -> Result<()> {
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("product/{}", encode_path(name))))
                .json(updates),
        );
        self.send(req).await?;
        Ok(())
    }

    /// Fetch a product by name. Note: components, versions, and milestones
    /// may require `include_fields` on some Bugzilla versions to be populated.
    pub async fn get_product(&self, name: &str) -> Result<Product> {
        let req = self.apply_auth(self.http.get(self.url("product")).query(&[("names", name)]));
        let resp = self.send(req).await?;
        let data: ProductResponse = self.parse_json(resp).await?;
        data.products
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "product",
                id: name.to_string(),
            })
    }

    /// Fetch legal values for a bug field.
    ///
    /// Returns `NotFound` when the server does not recognize the field name
    /// (empty `fields` array). An empty `Vec` means the field exists but has
    /// no legal values.
    pub async fn get_field_values(&self, field_name: &str) -> Result<Vec<FieldValue>> {
        let req = self.apply_auth(self.http.get(self.url(&format!("field/bug/{field_name}"))));
        let resp = self.send(req).await?;
        let data: FieldBugResponse = self.parse_json(resp).await?;
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

    pub async fn search_users(
        &self,
        query: &str,
        include_details: bool,
    ) -> Result<Vec<BugzillaUser>> {
        let mut req_builder = self.http.get(self.url("user")).query(&[("match", query)]);
        if include_details {
            req_builder = req_builder
                .query(&[("include_fields", "id,name,real_name,email,can_login,groups")]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        Ok(data.users)
    }

    pub async fn get_group_members(
        &self,
        group_name: &str,
        include_details: bool,
    ) -> Result<Vec<BugzillaUser>> {
        // Always send include_fields to cap the response payload — group
        // queries can return many members. Unlike search_users (which omits
        // include_fields to get Bugzilla's default set), we explicitly
        // constrain both paths.
        let fields = if include_details {
            "id,name,real_name,email,can_login,groups"
        } else {
            "id,name,real_name,email"
        };
        let req = self.apply_auth(
            self.http
                .get(self.url("user"))
                .query(&[("group", group_name), ("include_fields", fields)]),
        );
        let resp = self.send(req).await?;
        let data: UserSearchResponse = self.parse_json(resp).await?;
        Ok(data.users)
    }

    pub async fn add_user_to_group(&self, user: &str, group: &str) -> Result<()> {
        self.modify_group_membership(user, group, GroupOp::Add)
            .await
    }

    pub async fn remove_user_from_group(&self, user: &str, group: &str) -> Result<()> {
        self.modify_group_membership(user, group, GroupOp::Remove)
            .await
    }

    async fn modify_group_membership(
        &self,
        user: &str,
        group: &str,
        operation: GroupOp,
    ) -> Result<()> {
        let key = match operation {
            GroupOp::Add => "add",
            GroupOp::Remove => "remove",
        };
        let body = serde_json::json!({
            "groups": {
                key: [group]
            }
        });
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("user/{}", encode_path(user))))
                .json(&body),
        );
        self.send(req).await?;
        Ok(())
    }

    pub async fn whoami(&self) -> Result<WhoamiResponse> {
        let req = self.apply_auth(self.http.get(self.url("whoami")));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn server_version(&self) -> Result<ServerVersion> {
        let req = self.apply_auth(self.http.get(self.url("version")));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        let req = self.apply_auth(self.http.get(self.url("extensions")));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn update_attachment(&self, id: u64, params: &UpdateAttachmentParams) -> Result<()> {
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("bug/attachment/{id}")))
                .json(params),
        );
        self.send(req).await?;
        Ok(())
    }

    pub async fn get_classification(&self, name: &str) -> Result<Classification> {
        let req = self.apply_auth(
            self.http
                .get(self.url(&format!("classification/{}", encode_path(name)))),
        );
        let resp = self.send(req).await?;
        let data: ClassificationResponse = self.parse_json(resp).await?;
        data.classifications
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "classification",
                id: name.to_string(),
            })
    }

    pub async fn create_component(&self, params: &CreateComponentParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("component")).json(params));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_component(&self, id: u64, updates: &UpdateComponentParams) -> Result<()> {
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("component/{id}")))
                .json(updates),
        );
        self.send(req).await?;
        Ok(())
    }

    pub async fn create_user(&self, params: &CreateUserParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("user")).json(params));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_user(&self, user: &str, updates: &UpdateUserParams) -> Result<()> {
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("user/{}", encode_path(user))))
                .json(updates),
        );
        self.send(req).await?;
        Ok(())
    }

    pub async fn get_group(&self, group: &str) -> Result<GroupInfo> {
        let req = self.apply_auth(
            self.http
                .get(self.url("group"))
                .query(&[("names", group), ("membership", "1")]),
        );
        let resp = self.send(req).await?;
        let data: GroupResponse = self.parse_json(resp).await?;
        data.groups
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "group",
                id: group.to_string(),
            })
    }

    pub async fn create_group(&self, params: &CreateGroupParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("group")).json(params));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_group(&self, group: &str, updates: &UpdateGroupParams) -> Result<()> {
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("group/{}", encode_path(group))))
                .json(updates),
        );
        self.send(req).await?;
        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::config::AuthMethod;
    use crate::types::{FlagStatus, FlagUpdate};

    #[test]
    fn safe_url_strips_query_params() {
        let url = reqwest::Url::parse(&format!(
            "https://bugzilla.example.com/rest/bug/1?{}=secret",
            crate::http::AUTH_QUERY_PARAM
        ))
        .unwrap();
        let safe = BugzillaClient::safe_url(&url);
        assert!(
            !safe.contains("secret"),
            "API key should be stripped: {safe}"
        );
        assert!(
            safe.contains("/rest/bug/1"),
            "path should be preserved: {safe}"
        );
    }

    #[test]
    fn safe_url_preserves_path() {
        let url = reqwest::Url::parse("https://bugzilla.example.com/rest/bug/42").unwrap();
        let safe = BugzillaClient::safe_url(&url);
        assert_eq!(safe, "https://bugzilla.example.com/rest/bug/42");
    }

    fn test_client(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::Header, ApiMode::Rest).unwrap()
    }

    fn test_client_hybrid(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::Header, ApiMode::Hybrid).unwrap()
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
        let products = client
            .list_products_by_type(ProductListType::Accessible)
            .await
            .unwrap();
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
        let products = client
            .list_products_by_type(ProductListType::Accessible)
            .await
            .unwrap();
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
        let users = client.search_users("example", false).await.unwrap();
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
        let users = client.search_users("nobody", false).await.unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn search_users_details_sends_include_fields() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("match", "alice"))
            .and(query_param(
                "include_fields",
                "id,name,real_name,email,can_login,groups",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{
                    "id": 1,
                    "name": "alice@example.com",
                    "real_name": "Alice",
                    "email": "alice@example.com",
                    "can_login": true,
                    "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("alice", true).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].can_login, Some(true));
        assert_eq!(users[0].groups.len(), 1);
        assert_eq!(users[0].groups[0].name, "admin");
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
    async fn api_error_with_200_and_data_returns_data() {
        // Some servers (e.g. IBM LTC) return error fields alongside real
        // data. The data should be used and the error logged as a warning.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 100_500,
                "message": "MirrorTool internal error",
                "bugs": [{"id": 42, "summary": "test bug", "status": "NEW"}]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let bug = client.get_bug("42", None, None).await.unwrap();
        assert_eq!(bug.id, 42);
        assert_eq!(bug.summary, "test bug");
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
        let err = client.search_users("anyone", false).await.unwrap_err();
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
        let history = client.get_bug_history_since(42, None).await.unwrap();
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
        let history = client.get_bug_history_since(99, None).await.unwrap();
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
        let history = client.get_bug_history_since(10, None).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].changes[0].attachment_id, Some(555));
    }

    #[tokio::test]
    async fn get_group_members_returns_users() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "admin"))
            .and(query_param("include_fields", "id,name,real_name,email"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "id": 1,
                        "name": "alice@example.com",
                        "real_name": "Alice",
                        "email": "alice@example.com"
                    },
                    {
                        "id": 2,
                        "name": "bob@example.com",
                        "real_name": "Bob",
                        "email": "bob@example.com"
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("admin", false).await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "alice@example.com");
    }

    #[tokio::test]
    async fn get_group_members_details_sends_include_fields() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "admin"))
            .and(query_param(
                "include_fields",
                "id,name,real_name,email,can_login,groups",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "id": 1,
                        "name": "alice@example.com",
                        "real_name": "Alice",
                        "email": "alice@example.com",
                        "can_login": true,
                        "groups": [{"id": 10, "name": "admin", "description": "Admins"}]
                    }
                ]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.get_group_members("admin", true).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "alice@example.com");
        assert_eq!(users[0].groups.len(), 1);
        assert_eq!(users[0].groups[0].name, "admin");
        assert_eq!(users[0].can_login, Some(true));
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
        let users = client.get_group_members("nobody", false).await.unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn get_group_members_api_error() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param("group", "nonexistent"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "There is no group named 'nonexistent'."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client
            .get_group_members("nonexistent", false)
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "Expected error to mention group name, got: {msg}"
        );
    }

    #[tokio::test]
    async fn add_user_to_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("alice@example.com")
            )))
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
            .and(path(format!(
                "/rest/user/{}",
                encode_path("bob@example.com")
            )))
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
    async fn update_comment_tags_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/bug/comment/42/tags"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!(["needinfo", "reviewed"])),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let tags = client
            .update_comment_tags(42, &["needinfo".into()], &[])
            .await
            .unwrap();
        assert_eq!(tags, vec!["needinfo", "reviewed"]);
    }

    #[tokio::test]
    async fn search_comment_tags_returns_matches() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/comment/tags/need"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!(["needinfo", "needreview"])),
            )
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let tags = client.search_comment_tags("need").await.unwrap();
        assert_eq!(tags, vec!["needinfo", "needreview"]);
    }

    #[tokio::test]
    async fn get_comments_since_filters_by_date() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/1/comment"))
            .and(query_param("new_since", "2025-01-01T00:00:00Z"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "1": {
                        "comments": [
                            {"id": 5, "bug_id": 1, "text": "new comment", "count": 3}
                        ]
                    }
                }
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let comments = client
            .get_comments_since(1, Some("2025-01-01T00:00:00Z"))
            .await
            .unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "new comment");
    }

    #[tokio::test]
    async fn get_bug_history_since_filters_by_date() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/history"))
            .and(query_param("new_since", "2025-06-01T00:00:00Z"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "alias": [],
                    "history": [{
                        "who": "alice@example.com",
                        "when": "2025-06-15T10:00:00Z",
                        "changes": [{"field_name": "status", "removed": "NEW", "added": "ASSIGNED"}]
                    }]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client
            .get_bug_history_since(42, Some("2025-06-01T00:00:00Z"))
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn get_bug_passes_params() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/1"))
            .and(query_param("include_fields", "id,summary"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
            ))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let bug = client.get_bug("1", Some("id,summary"), None).await.unwrap();
        assert_eq!(bug.id, 1);
    }

    #[tokio::test]
    async fn get_bug_falls_back_on_100500() {
        let mock = MockServer::start().await;

        // Direct endpoint returns 100500 (server extension crash)
        Mock::given(method("GET"))
            .and(path("/rest/bug/99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 100_500,
                "message": "Extension crash"
            })))
            .mount(&mock)
            .await;

        // Search endpoint returns the bug successfully
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("id", "99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"bugs": [{"id": 99, "summary": "fallback bug", "status": "NEW"}]}),
            ))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let bug = client.get_bug("99", None, None).await.unwrap();
        assert_eq!(bug.id, 99);
        assert_eq!(bug.summary, "fallback bug");
    }

    #[tokio::test]
    async fn get_classification_returns_data() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/classification/Unclassified"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "classifications": [{
                    "id": 1,
                    "name": "Unclassified",
                    "description": "Default",
                    "sort_key": 0,
                    "products": [
                        {"id": 10, "name": "Widget", "description": "A widget"}
                    ]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let cls = client.get_classification("Unclassified").await.unwrap();
        assert_eq!(cls.name, "Unclassified");
        assert_eq!(cls.products.len(), 1);
        assert_eq!(cls.products[0].name, "Widget");
    }

    // Admin API tests: create/update product, component, user, group

    #[tokio::test]
    async fn create_product_returns_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/product"))
            .and(body_json(serde_json::json!({
                "name": "NewProduct",
                "description": "A new product",
                "version": "1.0",
                "is_open": true,
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 42})))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = CreateProductParams {
            name: "NewProduct".into(),
            description: "A new product".into(),
            version: "1.0".into(),
            is_open: true,
        };
        let id = client.create_product(&params).await.unwrap();
        assert_eq!(id, 42);
    }

    #[tokio::test]
    async fn create_product_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/product"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized to create products."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = CreateProductParams {
            name: "X".into(),
            description: "X".into(),
            version: "1.0".into(),
            is_open: true,
        };
        let err = client.create_product(&params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn update_product_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/product/Widget"))
            .and(body_json(serde_json::json!({
                "description": "Updated desc",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "products": [{"id": 1, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateProductParams {
            description: Some("Updated desc".into()),
            ..Default::default()
        };
        client.update_product("Widget", &params).await.unwrap();
    }

    #[tokio::test]
    async fn update_product_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/product/Widget"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateProductParams::default();
        let err = client.update_product("Widget", &params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

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

    #[tokio::test]
    async fn create_user_returns_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/user"))
            .and(body_json(serde_json::json!({
                "email": "new@example.com",
                "full_name": "New User",
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = CreateUserParams {
            email: "new@example.com".into(),
            full_name: Some("New User".into()),
            password: None,
        };
        let id = client.create_user(&params).await.unwrap();
        assert_eq!(id, 99);
    }

    #[tokio::test]
    async fn create_user_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = CreateUserParams {
            email: "x@x.com".into(),
            full_name: None,
            password: None,
        };
        let err = client.create_user(&params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn update_user_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("alice@example.com")
            )))
            .and(body_json(serde_json::json!({
                "real_name": "Alice Smith",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 1, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateUserParams {
            real_name: Some("Alice Smith".into()),
            ..Default::default()
        };
        client
            .update_user("alice@example.com", &params)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn update_user_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path(format!(
                "/rest/user/{}",
                encode_path("alice@example.com")
            )))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateUserParams::default();
        let err = client
            .update_user("alice@example.com", &params)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn get_group_returns_info() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .and(query_param("names", "admin"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "groups": [{
                    "id": 1,
                    "name": "admin",
                    "description": "Administrators",
                    "is_active": true,
                    "membership": []
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let info = client.get_group("admin").await.unwrap();
        assert_eq!(info.name, "admin");
        assert!(info.is_active);
    }

    #[tokio::test]
    async fn get_group_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_group("secret").await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn create_group_returns_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/group"))
            .and(body_json(serde_json::json!({
                "name": "testers",
                "description": "Test team",
                "is_active": true,
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 5})))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let id = client
            .create_group(&CreateGroupParams {
                name: "testers".into(),
                description: "Test team".into(),
                is_active: true,
            })
            .await
            .unwrap();
        assert_eq!(id, 5);
    }

    #[tokio::test]
    async fn create_group_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client
            .create_group(&CreateGroupParams {
                name: "x".into(),
                description: "x".into(),
                is_active: true,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[tokio::test]
    async fn update_group_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/group/testers"))
            .and(body_json(serde_json::json!({
                "description": "Updated testers",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "groups": [{"id": 5, "changes": {}}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateGroupParams {
            description: Some("Updated testers".into()),
            ..Default::default()
        };
        client.update_group("testers", &params).await.unwrap();
    }

    #[tokio::test]
    async fn update_group_forbidden() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/group/testers"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = UpdateGroupParams::default();
        let err = client.update_group("testers", &params).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    fn test_client_query_param(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::QueryParam, ApiMode::Rest).unwrap()
    }

    #[tokio::test]
    async fn auth_fallback_header_to_query_param_on_401() {
        let mock = MockServer::start().await;
        // Success response requires query param auth (registered first)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 1, "name": "alice@example.com"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;
        // First request returns 401 (registered second, checked first by LIFO)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 410,
                "message": "You must log in."
            })))
            .up_to_n_times(1)
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("alice", false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "alice@example.com");
    }

    #[tokio::test]
    async fn auth_fallback_query_param_to_header_on_401() {
        let mock = MockServer::start().await;
        // Success response requires header auth (registered first)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(wiremock::matchers::header(
                crate::http::AUTH_HEADER_NAME,
                "test-key",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 2, "name": "bob@example.com"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;
        // First request returns 401 (registered second, checked first by LIFO)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 410,
                "message": "You must log in."
            })))
            .up_to_n_times(1)
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_query_param(&mock.uri());
        let users = client.search_users("bob", false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "bob@example.com");
    }

    #[tokio::test]
    async fn auth_fallback_both_fail_returns_original_error() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 410,
                "message": "You must log in."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.search_users("anyone", false).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("410") || msg.contains("log in"),
            "expected auth error: {msg}"
        );
    }

    #[tokio::test]
    async fn non_401_errors_do_not_trigger_fallback() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.search_users("anyone", false).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }

    #[test]
    fn search_params_serialization_product_only() {
        let params = SearchParams {
            product: Some("Product".into()),
            limit: Some(50),
            ..Default::default()
        };
        let qs = serde_urlencoded::to_string(&params).unwrap();
        assert!(qs.contains("product=Product"));
        assert!(qs.contains("limit=50"));
    }

    #[tokio::test]
    async fn search_bugs_sends_product_filter() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("product", "Product"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 217_630,
                    "summary": "Test bug",
                    "status": "WORKING",
                    "product": "Product",
                    "component": "Triage",
                    "assigned_to": "test@example.com",
                    "priority": "P1",
                    "severity": "high",
                    "creation_time": "2026-03-09T09:33:08Z",
                    "last_change_time": "2026-03-18T05:49:05Z"
                }]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = SearchParams {
            product: Some("Product".into()),
            limit: Some(50),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, 217_630);
    }

    fn xmlrpc_bug_response(id: i64, summary: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse><params><param><value><struct>
              <member><name>bugs</name><value><array><data>
                <value><struct>
                  <member><name>id</name><value><int>{id}</int></value></member>
                  <member><name>summary</name><value><string>{summary}</string></value></member>
                  <member><name>status</name><value><string>NEW</string></value></member>
                  <member><name>keywords</name><value><array><data></data></array></value></member>
                  <member><name>blocks</name><value><array><data></data></array></value></member>
                  <member><name>depends_on</name><value><array><data></data></array></value></member>
                  <member><name>cc</name><value><array><data></data></array></value></member>
                </struct></value>
              </data></array></value></member>
            </struct></value></param></params></methodResponse>"#
        )
    }

    #[tokio::test]
    async fn hybrid_search_rest_has_results_no_xmlrpc_call() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{"id": 1, "summary": "REST bug", "status": "NEW"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let params = SearchParams {
            product: Some("P".into()),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].summary, "REST bug");
    }

    #[tokio::test]
    async fn hybrid_search_rest_empty_with_filters_falls_back_to_xmlrpc() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(99, "XML-RPC bug")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let params = SearchParams {
            product: Some("P".into()),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, 99);
        assert_eq!(bugs[0].summary, "XML-RPC bug");
    }

    #[tokio::test]
    async fn hybrid_search_rest_empty_without_filters_no_fallback() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let params = SearchParams::default();
        let bugs = client.search_bugs(&params).await.unwrap();
        assert!(bugs.is_empty());
    }

    #[tokio::test]
    async fn hybrid_get_bug_rest_500_falls_back_to_xmlrpc() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(500).set_body_string("error"))
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(xmlrpc_bug_response(42, "XML-RPC result")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let bug = client.get_bug("42", None, None).await.unwrap();
        assert_eq!(bug.id, 42);
        assert_eq!(bug.summary, "XML-RPC result");
    }

    #[tokio::test]
    async fn hybrid_get_bug_rest_401_does_not_fall_back() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 102,
                "message": "Invalid API key"
            })))
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let err = client.get_bug("42", None, None).await.unwrap_err();
        assert!(
            err.to_string().contains("Invalid API key"),
            "should propagate auth error, got: {err}"
        );
    }

    #[test]
    fn search_params_has_filters() {
        let empty = SearchParams::default();
        assert!(!empty.has_filters());

        let with_product = SearchParams {
            product: Some("P".into()),
            ..Default::default()
        };
        assert!(with_product.has_filters());

        let with_quicksearch = SearchParams {
            quicksearch: Some("crash".into()),
            ..Default::default()
        };
        assert!(with_quicksearch.has_filters());
    }
}
