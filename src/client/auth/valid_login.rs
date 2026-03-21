use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::http::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::AuthMethod;

#[derive(Deserialize)]
struct ValidLoginResponse {
    #[serde(default)]
    result: ValidLoginResult,
}

/// Bugzilla returns `{"result": true}` (bool) or `{"result": 1}` (integer)
/// depending on version. Accept both.
#[derive(Deserialize, Default)]
#[serde(try_from = "serde_json::Value")]
struct ValidLoginResult(bool);

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
    NetworkError,
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

    for (query, header, method) in &probes {
        match probe_valid_login(http, &url, query, *header, *method).await {
            ValidLoginOutcome::AuthRejected => {} // try next probe
            outcome => return outcome,
        }
    }

    tracing::debug!("valid_login probes both failed");
    ValidLoginOutcome::AuthRejected
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
            tracing::warn!(error = %e, "valid_login probe network error");
            return ValidLoginOutcome::NetworkError;
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
            return ValidLoginOutcome::NetworkError;
        }
    };
    tracing::trace!(probe = "valid_login", %method, body = body_text, "auth probe response");
    let parsed: ValidLoginResponse = match serde_json::from_str(&body_text) {
        Ok(p) => p,
        Err(_) => return ValidLoginOutcome::AuthRejected,
    };
    if parsed.result.0 {
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
