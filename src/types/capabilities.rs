//! Structured `server capabilities` data model and the pure derivations that
//! fill it (see ADR-0005). No I/O lives here — the client resource layer fetches
//! the data and assembles a [`ServerCapabilities`].

use serde::Serialize;

use crate::types::ApiMode;

/// The behavior surface an agent needs to plan mutations against a server.
///
/// Serializes to the documented `bzr server capabilities --json` shape. Nullable
/// fields keep their key (no `skip_serializing_if`) so a consumer can distinguish
/// "undetermined" (`null`) from a populated value.
#[derive(Debug, Serialize)]
#[non_exhaustive]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the four supports_* flags are a published flat JSON contract (issue \
              #457 shape / schemas/server-capabilities.json); each is an independent \
              transport-capability signal and collapsing them into an enum would \
              break the documented output"
)]
pub struct ServerCapabilities {
    pub version: String,
    pub api_modes: Vec<String>,
    pub auth_modes: Vec<String>,
    pub max_attachment_size: Option<u64>,
    pub status_transitions: Vec<StatusTransitionSummary>,
    pub flag_types: Option<Vec<FlagTypeSummary>>,
    pub custom_fields: Vec<CustomFieldSummary>,
    pub supports_comments: bool,
    pub supports_attachments: bool,
    pub supports_history: bool,
    pub supports_flag_requests: bool,
}

/// One status and the set of statuses it may transition to.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct StatusTransitionSummary {
    pub from: String,
    pub can_change_to: Vec<String>,
}

/// A custom (`cf_*`) field's name, human-readable type, and legal values.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct CustomFieldSummary {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub values: Vec<String>,
}

/// A flag type's name and request semantics. Reserved for the future per-product
/// flag path; defines the `flag_types` array element schema.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct FlagTypeSummary {
    pub name: String,
    pub requestable: bool,
    pub multiplicable: bool,
}

/// Map Bugzilla's integer field-type code to a stable human-readable name.
/// Codes follow Bugzilla's `FIELD_TYPE_*` constants; unknown codes are reported
/// as `"unknown"` rather than failing, so a future Bugzilla type does not error.
pub fn field_type_name(code: i64) -> &'static str {
    match code {
        1 => "freetext",
        2 => "single_select",
        3 => "multi_select",
        4 => "textarea",
        5 => "datetime",
        6 => "bug_id",
        7 => "bug_urls",
        8 => "keywords",
        9 => "date",
        10 => "integer",
        _ => "unknown",
    }
}

/// The transports a server with the given detected mode supports.
pub fn api_modes_for(mode: ApiMode) -> Vec<String> {
    match mode {
        ApiMode::Rest => vec!["rest".to_string()],
        ApiMode::Hybrid => vec!["rest".to_string(), "xmlrpc".to_string()],
        ApiMode::XmlRpc => vec!["xmlrpc".to_string()],
    }
}

/// Whether the server exposes a REST surface (Rest or Hybrid).
pub fn supports_rest_surface(mode: ApiMode) -> bool {
    match mode {
        ApiMode::Rest | ApiMode::Hybrid => true,
        ApiMode::XmlRpc => false,
    }
}

/// The auth mechanisms the server accepts. API-key auth requires a REST surface;
/// a pure XML-RPC (`< 5.0`) server reports none.
pub fn auth_modes_for(mode: ApiMode) -> Vec<String> {
    if supports_rest_surface(mode) {
        vec!["api_key".to_string()]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
