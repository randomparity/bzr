use serde::Deserialize;

use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::capabilities::{
    api_modes_for, auth_modes_for, field_type_name, supports_rest_surface, CustomFieldSummary,
    ServerCapabilities, StatusTransitionSummary,
};
use crate::types::server_info::{ServerExtensions, ServerInfoResponse, ServerVersion};
use crate::types::FieldValue;

/// One field as returned by `/rest/field/bug` (the all-fields listing).
#[derive(Deserialize)]
struct FieldDef {
    name: String,
    #[serde(rename = "type", default)]
    field_type: i64,
    #[serde(default)]
    is_custom: bool,
    #[serde(default)]
    values: Vec<FieldValue>,
}

#[derive(Deserialize)]
struct AllFieldsResponse {
    fields: Vec<FieldDef>,
}

#[derive(Deserialize)]
struct ParametersResponse {
    parameters: ParametersBody,
}

#[derive(Deserialize)]
struct ParametersBody {
    #[serde(default)]
    maxattachmentsize: Option<u64>,
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

    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        self.get_json("extensions").await
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
        let response: AllFieldsResponse = self.get_json("field/bug").await?;
        let summaries = response
            .fields
            .into_iter()
            .filter(|field| field.is_custom)
            .map(|field| CustomFieldSummary {
                name: field.name,
                field_type: field_type_name(field.field_type).to_string(),
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
        let response: Result<ParametersResponse> = self.get_json("parameters").await;
        response
            .ok()
            .and_then(|body| body.parameters.maxattachmentsize)
            .map(|kib| kib.saturating_mul(1024))
    }
}

/// Build transition summaries, skipping the null-named pseudo-entry and statuses
/// that carry no `can_change_to` list.
fn status_transitions(values: Vec<FieldValue>) -> Vec<StatusTransitionSummary> {
    values
        .into_iter()
        .filter_map(|value| {
            let from = value.name?;
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
