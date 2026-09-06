//! Wire transport for [`BugzillaClient`]: applying auth to a request, the
//! send/retry state machine (429 / 5xx / timeout with exponential backoff),
//! and the 401 alternate-auth (header ↔ query param) fallback.

use reqwest::header::HeaderValue;
use reqwest::RequestBuilder;

use crate::bugzilla_auth::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::error::{BzrError, Result};

use super::{BugzillaClient, PreparedAuth};

/// What the 401 alternate-auth retry established about the original refusal.
enum AlternateAuth {
    /// The retry's response replaces the original 401.
    Replace(reqwest::Response),
    /// The retry authenticated and the server refused the request for a reason
    /// unrelated to authentication. Establishing that consumed the body, so the
    /// refusal travels as the error it will be reported as.
    Refused(BzrError),
    /// The original 401 stands: no retry was possible, or the retry proved
    /// authentication itself failed, or it said nothing that distinguishes the
    /// two.
    Original,
}

/// Bugzilla's own taxonomy of authentication failure, from
/// `Bugzilla/WebService/Constants.pm`: "Authentication errors are usually
/// 300-400", plus the historical `login_required` at 410. Every other code
/// Bugzilla maps onto HTTP 401 — 102, 106, 109, 110, 113, 115, 120, 504, 505 —
/// refuses a caller who did authenticate. Keying on the band rather than an
/// enumerated list matters because `REST_STATUS_CODE_MAP` is extended at
/// runtime through the `webservice_status_code_map` hook. The band is
/// `300..=399`: 400 is Bugzilla's own `STATUS_BAD_REQUEST` and no
/// `WS_ERROR_CODE` entry uses it (ADR 0057).
fn code_proves_auth_failure(code: i64) -> bool {
    const LOGIN_REQUIRED: i64 = 410;
    (300..=399).contains(&code) || code == LOGIN_REQUIRED
}

impl BugzillaClient {
    /// Apply auth credentials to a request. Infallible because any configured
    /// API key was validated at client construction time. Anonymous clients
    /// leave the request unchanged.
    pub(super) fn apply_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            Some(PreparedAuth::Header(value)) => {
                crate::bugzilla_auth::apply_auth_to_request(builder, Some(value), None)
            }
            Some(PreparedAuth::QueryParam(key)) => {
                crate::bugzilla_auth::apply_auth_to_request(builder, None, Some(key))
            }
            None => builder,
        }
    }

    /// Apply only the configured authentication method and send exactly once.
    /// The response is left untouched so a focused caller can classify its
    /// status and body without retries or alternate-auth fallback.
    pub(super) async fn send_strict_once(
        &self,
        builder: RequestBuilder,
    ) -> Result<reqwest::Response> {
        let response = self.apply_auth(builder).send().await?;
        tracing::debug!(
            url = Self::safe_url(response.url()),
            status = %response.status(),
            "strict API response"
        );
        Ok(response)
    }

    /// Send a request, applying transient-failure retries (429 / 5xx / connect
    /// timeout) with exponential backoff when `--retry` is enabled. Each attempt
    /// also performs the 401 alternate-auth fallback (see [`Self::send_raw`]).
    /// The status of the final attempt is checked normally, so exhausted retries
    /// surface the usual `HttpStatus`/`Http` error (exit code 5).
    pub(super) async fn send(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
        // A write must not be replayed after a 5xx or read timeout: the server
        // may have already applied it, so a retry would duplicate the effect.
        // We gate on `is_safe()` (GET/HEAD), not `is_idempotent()`, because
        // bzr's PUT `Bug.update` is not effect-idempotent — `--work-time` is
        // additive and `--comment` posts atomically, so a replayed PUT could
        // double-count work or duplicate a comment. 429s and connect failures
        // are provably un-processed and stay retryable for any method; an
        // undeterminable method is treated as unsafe.
        let safe = builder
            .try_clone()
            .and_then(|b| b.build().ok())
            .is_some_and(|r| r.method().is_safe());
        let mut attempt: u32 = 0;
        loop {
            // Clone for a possible retry; a non-cloneable body (streaming) can't
            // be replayed, so send it once without retry.
            let Some(this) = builder.try_clone() else {
                return self
                    .check_response_status(self.send_raw(builder).await?)
                    .await;
            };
            match self.send_raw(this).await {
                Ok(resp)
                    if attempt < self.retry_max
                        && crate::http::should_retry_status(resp.status().as_u16(), safe) =>
                {
                    let status = resp.status().as_u16();
                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(crate::http::parse_retry_after);
                    Self::sleep_before_retry(attempt, retry_after, &format!("HTTP {status}")).await;
                    attempt += 1;
                }
                Ok(resp) => return self.check_response_status(resp).await,
                Err(e) if attempt < self.retry_max && Self::is_transient(&e, safe) => {
                    Self::sleep_before_retry(attempt, None, &e.to_string()).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Whether an error is a transient transport failure worth retrying for a
    /// request with the given safety (GET/HEAD).
    fn is_transient(err: &BzrError, safe: bool) -> bool {
        matches!(err, BzrError::Http(e) if crate::http::should_retry_transport(e, safe))
    }

    /// Sleep for the backoff interval before the next retry, logging the reason.
    async fn sleep_before_retry(
        attempt: u32,
        retry_after: Option<std::time::Duration>,
        reason: &str,
    ) {
        let delay = crate::http::backoff_delay(attempt, retry_after);
        tracing::warn!(
            attempt = attempt + 1,
            delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
            reason,
            "transient failure; retrying after backoff"
        );
        tokio::time::sleep(delay).await;
    }

    /// One request attempt: send, log, and perform the 401 alternate-auth
    /// fallback. Returns the raw response without status-checking so the caller
    /// ([`Self::send`]) can decide whether a retryable status warrants a retry.
    async fn send_raw(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
        let retry_builder = builder.try_clone();
        let resp = builder.send().await?;
        tracing::debug!(
            url = Self::safe_url(resp.url()),
            status = %resp.status(),
            "API response"
        );
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            match self.retry_with_alternate_auth(retry_builder).await? {
                AlternateAuth::Replace(retried) => return Ok(retried),
                AlternateAuth::Refused(err) => return Err(err),
                AlternateAuth::Original => {}
            }
        }
        Ok(resp)
    }

    /// On 401, retry the request with the alternate auth method (header ↔ query
    /// param) and classify what came back. Bugzilla maps fifteen distinct error
    /// codes onto HTTP 401, so the status alone cannot say whether the retry
    /// failed to authenticate or authenticated and was then refused; only the
    /// body can (ADR 0057).
    async fn retry_with_alternate_auth(
        &self,
        retry_builder: Option<RequestBuilder>,
    ) -> Result<AlternateAuth> {
        if self.auth.is_none() {
            return Ok(AlternateAuth::Original);
        }
        let Some(clone) = retry_builder else {
            return Ok(AlternateAuth::Original);
        };
        tracing::debug!("401 received, retrying with alternate auth method");
        let retried = self.apply_alternate_auth(clone)?.send().await?;
        let status = retried.status();
        tracing::debug!(
            url = Self::safe_url(retried.url()),
            status = %status,
            "auth fallback response"
        );
        if status != reqwest::StatusCode::UNAUTHORIZED && status != reqwest::StatusCode::FORBIDDEN {
            return Ok(AlternateAuth::Replace(retried));
        }
        // Reading the body is the only way to tell the two apart, and it
        // consumes the response — so a refusal is returned as the error it will
        // be reported as rather than as a response.
        let Ok(body) = retried.text().await else {
            // The transport error's `Display` carries the request URL, which on
            // the query-parameter path holds the API key, so nothing derived
            // from it is logged. Nothing was learned; the original stands.
            tracing::debug!("auth fallback response body unreadable, returning original 401");
            return Ok(AlternateAuth::Original);
        };
        match Self::bugzilla_error_code(&body) {
            Some(code) if code_proves_auth_failure(code) => {
                tracing::debug!(code, "auth fallback also failed to authenticate");
                Ok(AlternateAuth::Original)
            }
            Some(code) => {
                tracing::debug!(
                    code,
                    "auth fallback authenticated; server refused the request"
                );
                Ok(AlternateAuth::Refused(Self::error_from_status_body(
                    status, &body,
                )))
            }
            None => {
                tracing::debug!("auth fallback carried no Bugzilla error code");
                Ok(AlternateAuth::Original)
            }
        }
    }

    fn apply_alternate_auth(&self, builder: RequestBuilder) -> Result<RequestBuilder> {
        let mut request = builder.build()?;
        request.headers_mut().remove(AUTH_HEADER_NAME);
        strip_auth_query_param(request.url_mut());
        let builder = RequestBuilder::from_parts(self.http.clone(), request);

        match (&self.auth, self.api_key.as_deref()) {
            (Some(PreparedAuth::Header(_)), Some(api_key)) => {
                Ok(builder.query(&[(AUTH_QUERY_PARAM, api_key)]))
            }
            (Some(PreparedAuth::QueryParam(_)), Some(api_key)) => {
                let value = HeaderValue::from_str(api_key).map_err(|e| {
                    BzrError::Config(format!("API key contains invalid header characters: {e}"))
                })?;
                Ok(builder.header(AUTH_HEADER_NAME, value))
            }
            _ => Ok(builder),
        }
    }

    pub(super) fn safe_url(url: &reqwest::Url) -> String {
        format!("{}{}", url.origin().ascii_serialization(), url.path())
    }
}

fn strip_auth_query_param(url: &mut reqwest::Url) {
    let mut removed = false;
    let pairs = url
        .query_pairs()
        .filter_map(|(name, value)| {
            if name == AUTH_QUERY_PARAM {
                removed = true;
                None
            } else {
                Some((name.into_owned(), value.into_owned()))
            }
        })
        .collect::<Vec<_>>();

    if !removed {
        return;
    }
    if pairs.is_empty() {
        url.set_query(None);
        return;
    }

    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(name, value)| (name, value)));
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
