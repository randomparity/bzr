use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::bugzilla_auth::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::common::AuthMethod;

use super::MalformedProbeResponse;

#[derive(Deserialize)]
struct WhoamiProbeResponse {
    #[serde(default)]
    id: u64,
}

pub(super) enum WhoamiOutcome {
    Authenticated(AuthMethod),
    NotFound,
    /// Server responded but auth was rejected (e.g. 401) or credentials were not accepted.
    AuthRejected,
    /// Server returned 200 but the response body was unparseable or anomalous.
    MalformedResponse(MalformedProbeResponse),
    /// Could not reach the server at all (network/TLS/timeout). Carries the
    /// underlying transport error so the caller can classify it (TLS cert
    /// failure, pin mismatch, plain transport error) rather than masking it
    /// as a successful header-auth fallback.
    NetworkError(reqwest::Error),
}

pub(super) async fn detect_whoami_auth(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
) -> WhoamiOutcome {
    let url = format!("{base}/rest/whoami");
    let mut malformed_response = None;

    // Probe: header-based auth
    let header_req = http.get(&url).header(AUTH_HEADER_NAME, key_header.clone());
    let outcome = probe_whoami(header_req, AuthMethod::Header).await;
    match outcome {
        WhoamiOutcome::AuthRejected => {} // try query-param next
        WhoamiOutcome::MalformedResponse(error) => {
            malformed_response.get_or_insert(error);
        }
        other => return other,
    }

    // Probe: query-param auth
    let query_req = http.get(&url).query(&[(AUTH_QUERY_PARAM, api_key)]);
    match probe_whoami(query_req, AuthMethod::QueryParam).await {
        WhoamiOutcome::AuthRejected => malformed_response.map_or(
            WhoamiOutcome::AuthRejected,
            WhoamiOutcome::MalformedResponse,
        ),
        WhoamiOutcome::MalformedResponse(error) => {
            WhoamiOutcome::MalformedResponse(malformed_response.unwrap_or(error))
        }
        outcome => outcome,
    }
}

async fn probe_whoami(request: reqwest::RequestBuilder, method: AuthMethod) -> WhoamiOutcome {
    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            super::log_probe_send_error("whoami", method, &e);
            return WhoamiOutcome::NetworkError(e);
        }
    };

    let status = resp.status();
    let body = match resp.text().await {
        Ok(body) => body,
        Err(e) => {
            tracing::warn!(error = %e, "whoami response read error");
            return WhoamiOutcome::NetworkError(e);
        }
    };
    tracing::trace!(probe = "whoami", %method, %status, body, "auth probe response");
    if status.is_success() {
        match serde_json::from_str::<WhoamiProbeResponse>(&body) {
            Ok(parsed) if parsed.id > 0 => {
                return WhoamiOutcome::Authenticated(method);
            }
            Ok(_) => {
                // id==0 means unauthenticated/anonymous — treat as auth rejection.
                return WhoamiOutcome::AuthRejected;
            }
            Err(error) => {
                // 200 OK but body is not valid whoami JSON — server is behaving unexpectedly.
                tracing::debug!(%method, "whoami returned 200 but body is not valid whoami JSON");
                return WhoamiOutcome::MalformedResponse(MalformedProbeResponse::new(
                    "whoami", method, error,
                ));
            }
        }
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("rest/whoami not available on this server");
        return WhoamiOutcome::NotFound;
    }
    tracing::debug!(%status, %method, "whoami auth probe failed");
    WhoamiOutcome::AuthRejected
}

#[cfg(test)]
#[path = "whoami_tests.rs"]
mod tests;
