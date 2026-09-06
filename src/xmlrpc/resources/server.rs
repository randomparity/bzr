use std::collections::{BTreeMap, HashMap};

use crate::error::{BzrError, Result};
use crate::types::server_info::{ExtensionInfo, ServerExtensions};
use crate::xmlrpc::protocol::{Value, XmlRpcClient};
use crate::xmlrpc::resources::mappers::EXPECTED_STRUCT_RESPONSE;

impl XmlRpcClient {
    /// Advertised server extensions, via `Bugzilla.extensions`.
    ///
    /// The REST path decodes `{"extensions": {...}}` into
    /// `HashMap<String, ExtensionInfo>` with serde, which fails on a missing
    /// member, a non-object member, a non-object extension value, and a
    /// non-string `version`. This must agree, in the conservative direction:
    /// an absent member must not become an empty map (which renders as a
    /// settled "not advertised") and a malformed value must not become a name
    /// with `version: None` (which renders as "advertised"). All four are
    /// errors, so the capability gate says "could not determine" on evidence it
    /// could not read (ADR-0052, amended 2026-09-06).
    pub async fn server_extensions(&self) -> Result<ServerExtensions> {
        let result = self.call("Bugzilla.extensions", BTreeMap::new()).await?;
        let top = result
            .as_struct()
            .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;
        let advertised = top
            .get("extensions")
            .and_then(Value::as_struct)
            .ok_or_else(|| {
                BzrError::XmlRpc("expected an extensions struct in the response".into())
            })?;
        let extensions = advertised
            .iter()
            .map(|(name, info)| {
                // Bound the interpolated name: it is server-controlled and
                // read with no length cap, and both sibling paths bound server
                // text the same way (`HttpStatus` via `diagnostic_body_preview`,
                // REST's decode failure via `format_body_preview`).
                let label = crate::http::utf8_prefix(name, 128);
                let fields = info.as_struct().ok_or_else(|| {
                    BzrError::XmlRpc(format!(
                        "expected a struct for extension '{label}' in the response"
                    ))
                })?;
                // Strict, not `mappers::get_str`: that yields `None` for a
                // present non-string, where the REST side's `Option<String>`
                // decode fails. Absent stays `None` on both.
                let version = match fields.get("version") {
                    None => None,
                    Some(Value::String(version)) => Some(version.clone()),
                    Some(_) => {
                        return Err(BzrError::XmlRpc(format!(
                            "expected a string version for extension '{label}'"
                        )))
                    }
                };
                Ok((name.clone(), ExtensionInfo { version }))
            })
            .collect::<Result<HashMap<String, ExtensionInfo>>>()?;
        Ok(ServerExtensions { extensions })
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
