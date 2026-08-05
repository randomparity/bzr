use std::collections::BTreeMap;

use crate::bugzilla_auth::AUTH_QUERY_PARAM;
use crate::error::{BzrError, Result};
use crate::xmlrpc::protocol::call::build_request;
use crate::xmlrpc::protocol::parsing::parse_response;
use crate::xmlrpc::protocol::Value;

pub struct XmlRpcClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl XmlRpcClient {
    pub fn new(http: reqwest::Client, base_url: &str, api_key: Option<&str>) -> Self {
        XmlRpcClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.map(String::from),
        }
    }

    // SECURITY: When configured, the request body contains Bugzilla_api_key in
    // plain text. Never log the request body. Redact response body diagnostics:
    // extensions and intermediary servers can echo request URLs or credentials.
    //
    // NOTE: XML-RPC transmits the API key in the request body (as a method
    // parameter), regardless of the REST AuthMethod detected for this server.
    // This is an inherent XML-RPC protocol constraint; there is no header-based
    // auth equivalent for XML-RPC calls.
    pub(crate) async fn call(
        &self,
        method: &str,
        mut params: BTreeMap<String, Value>,
    ) -> Result<Value> {
        if let Some(api_key) = self.api_key.as_deref() {
            params.insert(AUTH_QUERY_PARAM.into(), Value::from(api_key));
        }

        let body = build_request(method, params);
        let url = format!("{}/xmlrpc.cgi", self.base_url);

        tracing::debug!(
            method,
            url = %self.base_url,
            "XML-RPC call"
        );

        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = match resp.text().await {
                Ok(body) => body,
                Err(e) => format!("<failed to read response body: {e}>"),
            };
            trace_http_error(status, &body);
            return Err(BzrError::HttpStatus {
                status: status.as_u16(),
                body: crate::http::diagnostic_body_preview(&body),
            });
        }

        let body_text = resp.text().await?;
        tracing::trace!(body_len = body_text.len(), "XML-RPC response received");

        parse_response(&body_text)
    }
}

fn trace_http_error(status: reqwest::StatusCode, body: &str) {
    tracing::debug!(
        %status,
        body = crate::bugzilla_auth::redact_api_key(crate::http::utf8_prefix(
            body,
            crate::http::DIAGNOSTIC_BODY_PREVIEW_MAX_BYTES,
        )),
        "XML-RPC HTTP error"
    );
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
