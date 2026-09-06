use serde::Deserialize;

use super::field::UnsignedWire;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::capabilities::{
    api_modes_for, auth_modes_for, field_type_name, supports_rest_surface, CustomFieldSummary,
    ServerCapabilities, StatusTransitionSummary,
};
use crate::types::server_info::{ServerExtensions, ServerInfoResponse, ServerVersion};
use crate::types::transport::ApiMode;
use crate::types::FieldValue;

#[derive(Deserialize)]
struct ParametersResponse {
    parameters: ParametersBody,
}

#[derive(Deserialize)]
struct ParametersBody {
    #[serde(default)]
    maxattachmentsize: Option<UnsignedWire>,
}

impl BugzillaClient {
    /// Fetch version and extensions from the server (two sequential requests).
    pub async fn server_info(&self) -> Result<ServerInfoResponse> {
        let version = self.server_version().await?;
        let extensions = self.server_extensions().await?;
        Ok(ServerInfoResponse {
            version,
            extensions,
        })
    }

    pub async fn server_version(&self) -> Result<ServerVersion> {
        self.get_json("version").await
    }

    /// Advertised server extensions, over the transport in use (ADR-0052,
    /// amended 2026-09-06, which carries the grounds for each arm).
    ///
    /// Hybrid keeps REST first — the transport `bug search` itself prefers in
    /// that mode — and falls back on ANY error, deliberately not on
    /// [`BzrError::is_transport_failure`], which the rest of the client uses.
    /// Bugzilla returns an error envelope even for a 404 on an absent endpoint,
    /// and `error_from_status_body` turns that into [`BzrError::Api`], which
    /// that predicate does not match — so it would miss the commonest shape of
    /// "REST did not serve this endpoint", the case the fallback exists for.
    /// Falling back more often is safe here because an error from the
    /// extensions endpoint says nothing about the capability, and the XML-RPC
    /// probe returns the server's own list: it can only make the verdict more
    /// determinate, never grant something absent. Do not "fix" this into
    /// consistency with the other call sites.
    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        match self.api_mode {
            ApiMode::Rest => self.get_json("extensions").await,
            ApiMode::XmlRpc => self.xmlrpc_client().server_extensions().await,
            ApiMode::Hybrid => match self.get_json("extensions").await {
                Err(rest_err) => {
                    // warn, not info: this fires only when the REST probe
                    // actually failed, so it is not routine noise — and a
                    // *successful* fallback would otherwise be the one case
                    // with no user-visible signal at the default `bzr=warn`,
                    // silently papering over a degraded REST surface on every
                    // invocation in Hybrid, which is the auto-detected default
                    // for Bugzilla 5.0.x.
                    tracing::warn!(
                        error = %rest_err,
                        "REST extensions probe failed, retrying via XML-RPC"
                    );
                    // Name both attempts when both fail: the line above is
                    // invisible at the default `bzr=warn`, and a user on a
                    // REST-first connection reading only an XML-RPC error would
                    // reasonably conclude bzr never tried REST. The variant
                    // reflects the final attempt; the message carries both.
                    self.xmlrpc_client()
                        .server_extensions()
                        .await
                        .map_err(|xmlrpc_err| {
                            BzrError::XmlRpc(format!(
                                "REST probe failed ({rest_err}); \
                                 XML-RPC probe also failed ({xmlrpc_err})"
                            ))
                        })
                }
                ok => ok,
            },
        }
    }

    /// Assemble the structured capability surface (see ADR-0005). `version` is
    /// required; `status_transitions`/`custom_fields` reuse the field data path;
    /// `max_attachment_size` is best-effort and credential-gated.
    pub async fn server_capabilities(&self) -> Result<ServerCapabilities> {
        let version = self.server_version().await?.version;
        let mode = self.api_mode;

        let status_transitions = match self.get_field_values("status").await {
            Ok(values) => status_transitions(values),
            // A server with no `status` field has no transitions to report; that
            // is a representable state, not a failure.
            Err(BzrError::NotFound { .. }) => Vec::new(),
            Err(err) => return Err(err),
        };
        let custom_fields = self.custom_field_summaries().await?;
        let max_attachment_size = self.attachment_size_limit().await;

        Ok(ServerCapabilities {
            version,
            api_modes: api_modes_for(mode),
            auth_modes: auth_modes_for(mode),
            max_attachment_size,
            status_transitions,
            flag_types: None,
            custom_fields,
            supports_comments: supports_rest_surface(mode),
            supports_attachments: supports_rest_surface(mode),
            supports_history: supports_rest_surface(mode),
            supports_flag_requests: supports_rest_surface(mode),
        })
    }

    /// Custom (`cf_*`) fields with their mapped type name and legal values.
    async fn custom_field_summaries(&self) -> Result<Vec<CustomFieldSummary>> {
        let summaries = self
            .all_bug_fields()
            .await?
            .into_iter()
            .filter(|field| field.is_custom)
            .map(|field| CustomFieldSummary {
                name: field.name,
                field_type: i64::try_from(field.field_type.0)
                    .map_or("unknown", field_type_name)
                    .to_string(),
                values: field
                    .values
                    .into_iter()
                    .filter_map(|value| value.name)
                    .collect(),
            })
            .collect();
        Ok(summaries)
    }

    /// Maximum attachment size in bytes, or `None` when undetermined. Bugzilla's
    /// `maxattachmentsize` parameter is in kilobytes and is absent from the
    /// anonymous whitelist, so the fetch only runs with a credential and any
    /// failure degrades to `None`.
    async fn attachment_size_limit(&self) -> Option<u64> {
        self.api_key.as_ref()?;
        let kib = match self.get_json::<ParametersResponse>("parameters").await {
            Ok(body) => body.parameters.maxattachmentsize,
            // Best-effort: degrade to `None` but leave a `-vv` trail so a
            // credentialed caller can tell "server refused parameters" from
            // "parameter unset". The error's Display is API-key-redacted.
            Err(BzrError::Deserialize(err)) => {
                tracing::debug!(
                    %err,
                    reason = tracing::field::display("response_shape"),
                    "max_attachment_size undetermined: /rest/parameters failed"
                );
                None
            }
            Err(err) => {
                tracing::debug!(
                    %err,
                    reason = tracing::field::display("request"),
                    "max_attachment_size undetermined: /rest/parameters failed"
                );
                None
            }
        };
        kib.map(|kib| kib.0.saturating_mul(1024))
    }
}

/// Build transition summaries, skipping the null-named pseudo-entry and statuses
/// that carry no `can_change_to` list.
fn status_transitions(values: Vec<FieldValue>) -> Vec<StatusTransitionSummary> {
    values
        .into_iter()
        .filter_map(|value| {
            let from = value.name?;
            if from.is_empty() {
                return None;
            }
            let can_change_to = value.can_change_to?;
            Some(StatusTransitionSummary {
                from,
                can_change_to: can_change_to.into_iter().map(|t| t.name).collect(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
