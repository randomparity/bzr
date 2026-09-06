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
            tracing::warn!(
                error = super::redacted_probe_error(&e),
                "valid_login response read error"
            );
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

/// A verification leg's response, reduced to what the decision needs.
///
/// The status is checked but deliberately not compared: including it would make
/// each status guard unfalsifiable, since a leg the guard rejects would compare
/// unequal anyway. See ADR 0056.
struct ProbeLeg {
    status: reqwest::StatusCode,
    body: ProbeBody,
}

/// A leg's body, compared as a parsed JSON value rather than as bytes.
///
/// Bugzilla randomises JSON object key order per response (measured on the
/// project's `bz50` image: three identical authenticated requests returned three
/// different byte sequences and one identical value), so a byte comparison would
/// find two identical records unequal and leave the probe permanently negative.
/// A body that is not JSON is compared as raw text and never equals a parsed one.
#[derive(PartialEq)]
enum ProbeBody {
    Json(serde_json::Value),
    Text(String),
}

impl ProbeBody {
    /// Bugzilla returns some errors inside an HTTP 200 -- see
    /// `crate::client::response`'s `check_bugzilla_200_error`, whose own doc
    /// comment names IBM LTC Bugzilla, the server class this fallback exists
    /// for. Two credentialed legs carrying the same 200 error would otherwise
    /// read as agreement. This test is deliberately broader than that helper's,
    /// which also requires the absence of real data: the helper must not discard
    /// a result the user asked for, while this only decides whether to trust a
    /// leg, and the safe answer to an ambiguous leg is no.
    fn carries_error(&self) -> bool {
        let Self::Json(value) = self else {
            return false;
        };
        value.get("error").is_some_and(|error| {
            !matches!(
                error,
                serde_json::Value::Bool(false) | serde_json::Value::Null
            )
        })
    }
}

impl ProbeLeg {
    /// Whether a *credentialed* leg shows the server accepted the credential.
    ///
    /// Only the header and query-parameter legs are held to this: their job is to
    /// stand in for the authenticated response, so a refusal -- by status, or by
    /// a Bugzilla error delivered inside a 200 -- disqualifies them. The
    /// anonymous leg is deliberately exempt: Bugzilla answers an anonymous caller
    /// with a status *and* an error body together (issue #713 measured `401` with
    /// `code 410`), and that refusal is the discrimination the probe is looking
    /// for, not a reason to discard the leg. See ADR 0056.
    fn credential_accepted(&self) -> bool {
        self.status.is_success() && !self.body.carries_error()
    }
}

/// Whether a leg's status says anything about authentication at all.
///
/// A success does. So does `401`/`403`, which is why the anonymous leg keeps
/// them. Everything else -- `5xx`, `404`, a redirect -- is a response the server
/// would have given whatever credential it was shown.
fn leg_status_is_conclusive(status: reqwest::StatusCode) -> bool {
    status.is_success()
        || status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
}

/// Verify that the server actually honours `X-BUGZILLA-API-KEY`, rather than
/// answering the probe the same way it answers an anonymous caller.
///
/// Some servers (e.g. IBM LTC Bugzilla) report header auth as unsupported via
/// `valid_login` but accept it on real API endpoints. Confirming that needs
/// evidence the header changed the response, so this compares three
/// `rest/user?names=<login>` responses: with the header, with no credentials,
/// and with the query parameter `valid_login` already proved. Header auth is
/// confirmed only when the credentialed legs succeed, the header response
/// differs from the anonymous one, and it matches the query-parameter one. Every
/// other outcome -- including every inconclusive one -- returns `false`, leaving
/// the caller on the method `valid_login` proved. See ADR 0056.
pub(super) async fn verify_header_auth_via_rest(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
    login: &str,
) -> bool {
    let url = format!("{base}/rest/user");

    let Some(header_leg) = read_probe_leg(
        http.get(&url)
            .query(&[("names", login)])
            .header(AUTH_HEADER_NAME, key_header.clone()),
        "header",
    )
    .await
    else {
        return false;
    };
    if !header_leg.credential_accepted() {
        tracing::debug!(status = %header_leg.status, "header auth probe: header leg refused");
        return false;
    }

    let Some(anonymous_leg) =
        read_probe_leg(http.get(&url).query(&[("names", login)]), "anonymous").await
    else {
        return false;
    };
    if anonymous_leg.body == header_leg.body {
        tracing::info!(
            "header auth probe on rest/user matched the anonymous response; \
             the header changed nothing, so keeping query-parameter auth -- \
             the API key travels in request URLs and so reaches the server's access log"
        );
        return false;
    }

    let Some(query_leg) = read_probe_leg(
        http.get(&url)
            .query(&[("names", login), (AUTH_QUERY_PARAM, api_key)]),
        "query-param",
    )
    .await
    else {
        return false;
    };
    if !query_leg.credential_accepted() {
        tracing::debug!(status = %query_leg.status, "header auth probe: query-param leg refused");
        return false;
    }

    if query_leg.body != header_leg.body {
        tracing::info!(
            "header auth probe on rest/user matched neither the anonymous nor the \
             authenticated response, so keeping query-parameter auth -- the API key \
             travels in request URLs and so reaches the server's access log"
        );
        return false;
    }

    // The anonymous leg is a single observation, and the whole differential rests
    // on it: reaching here means it differed from the header leg. That difference
    // is only evidence about auth if it repeats. A rate limiter or WAF answering
    // the second request of a burst differently -- a `401`, a `200` HTML
    // interstitial, a `200` Bugzilla error -- produces the same inequality, and on
    // a server that ignores the header and does not discriminate at this endpoint
    // the query-parameter leg then matches the header leg, so one anomaly would
    // confirm header auth. Re-observe before confirming; a server that genuinely
    // discriminates answers an anonymous caller the same way twice. One extra
    // request, only on the path that is about to return `true`.
    let Some(recheck) = read_probe_leg(
        http.get(&url).query(&[("names", login)]),
        "anonymous re-check",
    )
    .await
    else {
        return false;
    };
    if recheck.status.is_success() != anonymous_leg.status.is_success()
        || recheck.body != anonymous_leg.body
    {
        tracing::info!(
            "header auth probe on rest/user saw the anonymous response change between \
             requests, so the difference was transient rather than authentication; \
             keeping query-parameter auth -- the API key travels in request URLs and so \
             reaches the server's access log"
        );
        return false;
    }

    tracing::debug!("header auth probe on rest/user matched the authenticated response");
    true
}

/// Send one verification leg and reduce it to a [`ProbeLeg`]. `None` means the
/// leg said nothing about authentication at all -- a transport failure, an
/// unreadable body, or an inconclusive status -- which every caller treats as
/// "not confirmed". Whether a leg's *content* is trustworthy is the caller's
/// question, and it differs by leg: see [`ProbeLeg::credential_accepted`].
async fn read_probe_leg(request: reqwest::RequestBuilder, leg: &'static str) -> Option<ProbeLeg> {
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            // Never format the error verbatim: the query-parameter leg's URL
            // carries the API key and reqwest appends it to the message.
            tracing::debug!(
                "header auth {leg} probe request failed: {}",
                super::redacted_probe_error(&error)
            );
            return None;
        }
    };
    let status = response.status();
    if !leg_status_is_conclusive(status) {
        tracing::debug!(%status, %leg, "header auth probe leg returned an inconclusive status");
        return None;
    }
    let body = match response.text().await {
        Ok(body) => parse_probe_body(&body),
        Err(error) => {
            tracing::debug!(
                "header auth {leg} probe response read failed: {}",
                super::redacted_probe_error(&error)
            );
            return None;
        }
    };
    Some(ProbeLeg { status, body })
}

fn parse_probe_body(body: &str) -> ProbeBody {
    serde_json::from_str::<serde_json::Value>(body)
        .map_or_else(|_| ProbeBody::Text(body.to_owned()), ProbeBody::Json)
}

#[cfg(test)]
#[path = "valid_login_tests.rs"]
mod tests;
