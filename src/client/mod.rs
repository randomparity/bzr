pub(crate) mod auth;
pub(crate) use auth::{
    detect_server_settings, detect_server_settings_without_auth, DetectedServerSettings,
};
mod request;
mod resources;
mod response;
mod transport;
mod version;

use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::error::{BzrError, Result};
use crate::types::transport::{ApiMode, AuthMethod};
use crate::types::user::BugzillaUser;
use crate::xmlrpc::protocol::XmlRpcClient;

/// Default fields for user queries (basic info).
pub(super) const USER_FIELDS_BASIC: &str = "id,name,real_name,email,groups";
/// Extended fields for detailed user queries.
pub(super) const USER_FIELDS_DETAILED: &str = "id,name,real_name,email,can_login,groups";

/// Field detail level for APIs that return users.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserDetailLevel {
    Basic,
    Detailed,
}

impl UserDetailLevel {
    const fn include_fields(self) -> &'static str {
        match self {
            Self::Basic => USER_FIELDS_BASIC,
            Self::Detailed => USER_FIELDS_DETAILED,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct UserSearchResponse {
    pub(super) users: Vec<BugzillaUser>,
}

pub(super) fn encode_path(segment: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
}

pub(crate) fn parse_adjacency_numeric(requested: &str) -> Option<i64> {
    (!requested.is_empty() && requested.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| requested.parse::<i64>().ok())
        .flatten()
}

enum PreparedAuth {
    Header(HeaderValue),
    QueryParam(String),
}

/// Bugzilla API client for REST, XML-RPC, and Hybrid transport modes.
///
/// The client owns the shared HTTP stack, authentication material, API-mode
/// preference, and XML-RPC adapter used by resource methods. Hybrid behavior is
/// operation-specific: some reads are REST-first with XML-RPC fallback, while
/// comments and attachments use XML-RPC first to preserve private-data behavior
/// that REST responses cannot reliably distinguish.
///
/// Update methods use the identifier type that the Bugzilla REST API accepts:
/// - `u64` for resources identified only by numeric ID (e.g. `update_component`)
/// - `&str` for resources that accept name-based addressing (e.g. `update_product`, `update_user`)
pub struct BugzillaClient {
    pub(super) http: reqwest::Client,
    pub(super) strict_http: reqwest::Client,
    pub(super) base_url: String,
    auth: Option<PreparedAuth>,
    pub(super) api_key: Option<String>,
    pub(super) api_mode: ApiMode,
    pub(super) xmlrpc: XmlRpcClient,
    pub(super) strict_xmlrpc: Box<XmlRpcClient>,
    /// Email hint for Bugzilla 5.0 compatibility (whoami fallback via user lookup).
    email_hint: Option<String>,
    /// The configured/inline server name this client resolved against, surfaced
    /// by `whoami` so a single call reports which server the identity belongs to.
    server_name: String,
    /// Transient-retry budget (429 / 5xx / timeout). 0 disables retries.
    retry_max: u32,
}

/// Configuration needed to construct a [`BugzillaClient`].
#[non_exhaustive]
#[derive(Clone, Copy)]
pub struct BugzillaClientConfig<'a> {
    pub base_url: &'a str,
    pub credential: Option<&'a str>,
    pub auth_method: Option<AuthMethod>,
    pub api_mode: ApiMode,
    pub email_hint: Option<&'a str>,
    pub server_name: &'a str,
    pub tls_config: &'a crate::tls::TlsConfig,
    pub request_timeout: std::time::Duration,
    pub retry_max: u32,
}

/// Generic response for endpoints that return a single `id` field.
/// Used by bug creation, comment creation, product/component/user/group creation.
#[derive(Deserialize)]
pub(super) struct IdResponse {
    pub id: u64,
}

impl BugzillaClient {
    pub fn new(config: BugzillaClientConfig<'_>) -> Result<Self> {
        let BugzillaClientConfig {
            base_url,
            credential,
            auth_method,
            api_mode,
            email_hint,
            server_name,
            tls_config,
            request_timeout,
            retry_max,
        } = config;

        let auth = match (credential, auth_method) {
            (Some(key), Some(AuthMethod::Header)) => {
                let value = HeaderValue::from_str(key)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                Some(PreparedAuth::Header(value))
            }
            (Some(key), Some(AuthMethod::QueryParam)) => {
                Some(PreparedAuth::QueryParam(key.to_string()))
            }
            (None, None) => None,
            (Some(_), None) => {
                return Err(BzrError::config(
                    "internal: credential provided without detected auth method",
                ));
            }
            (None, Some(_)) => {
                return Err(BzrError::config(
                    "internal: auth method provided without credential",
                ));
            }
        };

        let http = crate::tls::build_tls_client(tls_config, request_timeout)?;
        let strict_http = crate::tls::build_no_redirect_tls_client(tls_config, request_timeout)?;

        // Always construct the XML-RPC client — even in REST mode, some
        // methods (e.g. Group.get on Bugzilla 5.3+) require XML-RPC fallback
        // because the REST endpoint is broken for them.
        if api_mode != ApiMode::Rest && auth_method == Some(AuthMethod::Header) {
            tracing::info!(
                "XML-RPC always sends API key in request body, \
                 overriding configured header auth for XML-RPC calls"
            );
        }
        let xmlrpc = XmlRpcClient::new(http.clone(), base_url, credential);
        let strict_xmlrpc = Box::new(XmlRpcClient::new(strict_http.clone(), base_url, credential));

        tracing::debug!(base_url, ?auth_method, %api_mode, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            strict_http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            api_key: credential.map(String::from),
            api_mode,
            xmlrpc,
            strict_xmlrpc,
            email_hint: email_hint.map(String::from),
            server_name: server_name.to_string(),
            retry_max,
        })
    }

    /// The configured/inline server name this client resolved against.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// How this connection is authenticated: [`crate::types::AuthMode::ApiKey`]
    /// when a credential was supplied, [`crate::types::AuthMode::Anonymous`]
    /// otherwise.
    pub fn auth_mode(&self) -> crate::types::AuthMode {
        if self.api_key.is_some() {
            crate::types::AuthMode::ApiKey
        } else {
            crate::types::AuthMode::Anonymous
        }
    }

    /// Prove the configured credentials through `rest/valid_login` once.
    ///
    /// The proof uses the current auth method only and disables redirects via
    /// the strict client. It is intentionally not an auth-detection fallback.
    pub async fn prove_current_credentials(&self) -> Result<()> {
        let login = self
            .email_hint
            .as_deref()
            .filter(|login| !login.is_empty())
            .ok_or_else(|| {
                BzrError::Auth(
                    "current credential proof requires a configured email for rest/valid_login"
                        .to_owned(),
                )
            })?;
        self.api_key
            .as_deref()
            .filter(|credential| !credential.is_empty())
            .ok_or_else(|| {
                BzrError::Auth(
                    "current credential proof requires a configured credential".to_owned(),
                )
            })?;
        let auth = self.auth.as_ref().ok_or_else(|| {
            BzrError::Auth("current credential proof requires a configured auth method".to_owned())
        })?;

        auth::prove_valid_login_current_method(&self.strict_http, &self.base_url, login, auth).await
    }

    /// Override the transient-retry budget. Used by tests to exercise the
    /// retry path without mutating the process-wide `--retry` global (which
    /// would race other tests).
    #[cfg(test)]
    pub(crate) fn set_retry_max(&mut self, n: u32) {
        self.retry_max = n;
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}/rest/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub(super) fn xmlrpc_client(&self) -> &XmlRpcClient {
        &self.xmlrpc
    }

    pub(super) fn strict_xmlrpc_client(&self) -> &XmlRpcClient {
        &self.strict_xmlrpc
    }

    /// Dispatch an operation across the detected API mode. In Hybrid mode the
    /// XML-RPC path is tried first and a transport failure falls back to REST
    /// (the shape shared by the per-resource read methods). `op` names the
    /// operation for the fallback log line.
    pub(super) async fn dispatch_xmlrpc_first<T>(
        &self,
        op: &str,
        rest: impl AsyncFnOnce() -> Result<T>,
        xmlrpc: impl AsyncFnOnce() -> Result<T>,
    ) -> Result<T> {
        match self.api_mode {
            ApiMode::Rest => rest().await,
            ApiMode::XmlRpc => xmlrpc().await,
            ApiMode::Hybrid => match xmlrpc().await {
                Ok(v) => Ok(v),
                Err(e) if e.is_transport_failure() => {
                    tracing::info!(op, error = %e, "XML-RPC {op} failed, retrying via REST");
                    rest().await
                }
                Err(e) => Err(e),
            },
        }
    }
}

#[cfg(test)]
pub(super) mod test_helpers;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
