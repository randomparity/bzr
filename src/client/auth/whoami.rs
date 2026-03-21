use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::http::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::AuthMethod;

#[derive(Deserialize)]
struct WhoamiProbeResponse {
    #[serde(default)]
    id: u64,
}

pub(super) enum WhoamiOutcome {
    Authenticated(AuthMethod),
    NotFound,
    /// Server responded but auth was rejected (e.g. 401).
    AuthRejected,
    /// Could not reach the server at all (network/TLS/timeout).
    NetworkError,
}

pub(super) async fn detect_whoami_auth(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
) -> WhoamiOutcome {
    let url = format!("{base}/rest/whoami");

    // Probe: header-based auth
    let header_req = http.get(&url).header(AUTH_HEADER_NAME, key_header.clone());
    let outcome = probe_whoami(header_req, AuthMethod::Header).await;
    match outcome {
        WhoamiOutcome::AuthRejected => {} // try query-param next
        other => return other,
    }

    // Probe: query-param auth
    let query_req = http.get(&url).query(&[(AUTH_QUERY_PARAM, api_key)]);
    probe_whoami(query_req, AuthMethod::QueryParam).await
}

async fn probe_whoami(request: reqwest::RequestBuilder, method: AuthMethod) -> WhoamiOutcome {
    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("whoami {method} request failed: {e:#}");
            return WhoamiOutcome::NetworkError;
        }
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    tracing::trace!(probe = "whoami", %method, %status, body, "auth probe response");
    if status.is_success() {
        // A valid whoami response contains a positive user ID. Treat id==0
        // (unauthenticated/anonymous) and unparseable bodies as auth failures
        // -- the server responded but didn't confirm our credentials.
        if let Ok(parsed) = serde_json::from_str::<WhoamiProbeResponse>(&body) {
            if parsed.id > 0 {
                return WhoamiOutcome::Authenticated(method);
            }
        }
    } else if status == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("rest/whoami not available on this server");
        return WhoamiOutcome::NotFound;
    } else {
        tracing::debug!(%status, %method, "whoami auth probe failed");
    }

    WhoamiOutcome::AuthRejected
}
