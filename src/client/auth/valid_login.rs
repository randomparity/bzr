use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::bugzilla_auth::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::client::PreparedAuth;
use crate::error::BzrError;
use crate::types::transport::AuthMethod;

use super::MalformedProbeResponse;

#[derive(Deserialize)]
struct ValidLoginResponse {
    result: ValidLoginResult,
}

/// Bugzilla returns `{"result": true}` (bool) or `{"result": 1}` (integer)
/// depending on version. Accept both.
#[derive(Deserialize)]
#[serde(try_from = "serde_json::Value")]
struct ValidLoginResult(bool);

impl ValidLoginResult {
    fn is_valid(&self) -> bool {
        self.0
    }
}

impl TryFrom<serde_json::Value> for ValidLoginResult {
    type Error = String;

    fn try_from(v: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        match v {
            serde_json::Value::Bool(b) => Ok(Self(b)),
            serde_json::Value::Number(n) => Ok(Self(n.as_u64() == Some(1))),
            other => Err(format!("expected bool or integer, got {other}")),
        }
    }
}

/// Outcome of a `valid_login` probe, mirroring [`super::whoami::WhoamiOutcome`].
pub(super) enum ValidLoginOutcome {
    Authenticated(AuthMethod),
    AuthRejected,
    /// Server returned 200 but the response body was unparseable or anomalous.
    MalformedResponse(MalformedProbeResponse),
    /// Could not complete the probe due to a transport failure. Carries the
    /// underlying error so the caller can classify TLS/network failures
    /// instead of masking them as a successful header-auth fallback.
    NetworkError(reqwest::Error),
}

pub(super) async fn detect_valid_login_auth(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
    login: &str,
) -> ValidLoginOutcome {
    let url = format!("{base}/rest/valid_login");

    let probes: [(_, _, _); 2] = [
        // Probe: header-based auth
        (vec![("login", login)], Some(key_header), AuthMethod::Header),
        // Probe: query-param auth
        (
            vec![("login", login), (AUTH_QUERY_PARAM, api_key)],
            None,
            AuthMethod::QueryParam,
        ),
    ];

    let mut malformed_response = None;
    for (query, header, method) in &probes {
        match probe_valid_login(http, &url, query, *header, *method).await {
            ValidLoginOutcome::AuthRejected => {} // try next probe
            ValidLoginOutcome::MalformedResponse(error) => {
                malformed_response.get_or_insert(error);
            }
            outcome => return outcome,
        }
    }

    tracing::debug!("valid_login probes both failed");
    malformed_response.map_or(
        ValidLoginOutcome::AuthRejected,
        ValidLoginOutcome::MalformedResponse,
    )
}

/// Prove the configured credential with exactly its current auth method.
///
/// This is deliberately separate from auth detection: it sends one request,
/// never tries the alternate method, and treats every non-conclusive response
/// as an authentication error.
pub(in crate::client) async fn prove_valid_login_current_method(
    http: &reqwest::Client,
    base: &str,
    login: &str,
    auth: &PreparedAuth,
) -> crate::error::Result<()> {
    let url = format!("{base}/rest/valid_login");
    let request = http.get(url).query(&[("login", login)]);
    let request = match auth {
        PreparedAuth::Header(key) => request.header(AUTH_HEADER_NAME, key.clone()),
        PreparedAuth::QueryParam(key) => request.query(&[(AUTH_QUERY_PARAM, key)]),
    };
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(BzrError::Auth(format!(
            "current credentials received unexpected HTTP status {status} from rest/valid_login"
        )));
    }

    let body = response.text().await?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
        BzrError::Auth(format!(
            "current credentials received invalid response from rest/valid_login: {error}"
        ))
    })?;
    if value.get("error").is_some() {
        return Err(BzrError::Auth(
            "current credentials received invalid response from rest/valid_login: top-level error"
                .to_owned(),
        ));
    }
    let parsed: ValidLoginResponse = serde_json::from_str(&body).map_err(|error| {
        BzrError::Auth(format!(
            "current credentials received invalid response from rest/valid_login: {error}"
        ))
    })?;
    if parsed.result.is_valid() {
        Ok(())
    } else {
        Err(BzrError::Auth(
            "current credentials did not confirm via rest/valid_login".to_owned(),
        ))
    }
}

async fn probe_valid_login(
    http: &reqwest::Client,
    url: &str,
    query: &[(&str, &str)],
    key_header: Option<&HeaderValue>,
    method: AuthMethod,
) -> ValidLoginOutcome {
    let mut req = http.get(url).query(query);
    if let Some(hdr) = key_header {
        req = req.header(AUTH_HEADER_NAME, hdr.clone());
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            super::log_probe_send_error("valid_login", method, &e);
            return ValidLoginOutcome::NetworkError(e);
        }
    };
    let status = resp.status();
    if !status.is_success() {
        tracing::debug!(%status, %method, "valid_login probe failed");
        return ValidLoginOutcome::AuthRejected;
    }
    let body_text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "valid_login response read error");
            return ValidLoginOutcome::NetworkError(e);
        }
    };
    tracing::trace!(
        probe = "valid_login",
        %method,
        body = super::trace_body_preview(&body_text),
        "auth probe response"
    );
    let parsed: ValidLoginResponse = match serde_json::from_str(&body_text) {
        Ok(p) => p,
        Err(error) => {
            return ValidLoginOutcome::MalformedResponse(MalformedProbeResponse::new(
                "valid_login",
                method,
                error,
            ));
        }
    };
    if parsed.result.is_valid() {
        ValidLoginOutcome::Authenticated(method)
    } else {
        tracing::debug!(%method, "valid_login returned false");
        ValidLoginOutcome::AuthRejected
    }
}

/// Try header auth on a real API endpoint to verify it works.
///
/// Some servers (e.g. IBM LTC Bugzilla) report header auth as unsupported
/// via `valid_login` but accept it on actual API endpoints. A minimal
/// `rest/bug?limit=1` request is used -- any 2xx confirms header auth works.
pub(super) async fn verify_header_auth_via_rest(
    http: &reqwest::Client,
    base: &str,
    key_header: &HeaderValue,
) -> bool {
    let url = format!("{base}/rest/bug");
    let resp = http
        .get(&url)
        .query(&[("limit", "1")])
        .header(AUTH_HEADER_NAME, key_header.clone())
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            tracing::debug!("header auth probe on rest/bug succeeded");
            true
        }
        Ok(r) => {
            tracing::debug!(
                status = %r.status(),
                "header auth probe on rest/bug failed"
            );
            false
        }
        Err(e) => {
            tracing::debug!("header auth probe request failed: {e}");
            false
        }
    }
}

#[cfg(test)]
#[path = "valid_login_tests.rs"]
mod tests;
