//! Server field-catalogue lookup for the `--field` / `--field-json`
//! passthrough on `bug create` and `bug update` (ADR 0053).
//!
//! No key reaches the wire that the server has not declared, or that bzr's own
//! REST payloads already use (the catalogue reports internal column names for
//! many built-ins, so the two sets are not the same). The persisted
//! `ServerConfig.bug_field_names` list is a fast path only: a key missing from
//! it always forces a fresh `field/bug` probe, so a stale list can never
//! reject a field the server has since declared. When the probe itself fails
//! the write is refused — a probe failure is not an absent field, and the two
//! diagnostics never overlap.

use std::collections::BTreeSet;

use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};

const PROBE_CONTEXT: &str = "could not validate --field keys: the server's bug field catalogue \
                             was not retrieved, so no changes were sent";

/// Re-surface a failed catalogue probe as itself, recording that it was the
/// probe that failed. The variant — and therefore the exit code — is preserved
/// so the caller still learns whether this was a server fault, a transport
/// fault, or an auth fault; only the message gains context. Follows the
/// `annotate_search_fallback` pattern in `client/resources/bug.rs`.
///
/// `Http` carries an opaque `reqwest::Error` that cannot be rebuilt with a new
/// message. It already names the connection failure and the write is still
/// refused, so it passes through unchanged.
fn annotate_probe_failure(original: BzrError) -> BzrError {
    match original {
        BzrError::Api { code, message } => BzrError::Api {
            code,
            message: format!("{message} ({PROBE_CONTEXT})"),
        },
        BzrError::HttpStatus { status, body } => BzrError::HttpStatus {
            status,
            body: format!("{body} ({PROBE_CONTEXT})"),
        },
        BzrError::Auth(message) => BzrError::Auth(format!("{message} ({PROBE_CONTEXT})")),
        BzrError::Deserialize(message) => {
            BzrError::Deserialize(format!("{message} ({PROBE_CONTEXT})"))
        }
        other => other,
    }
}

fn undeclared(key: &str) -> BzrError {
    BzrError::input_field(
        format!(
            "--field: this server does not declare a field named '{key}'; \
             run `bzr field list` to see the fields it accepts"
        ),
        "--field",
        Some(key.to_string()),
    )
}

/// Read the names cached by the last successful probe for `server_name`.
/// A missing config, an unreadable one, or a server without an entry all mean
/// "no fast path" rather than an error — the probe answers authoritatively.
fn cached_names(
    config_path_override: Option<&std::path::Path>,
    server_name: &str,
) -> Option<Vec<String>> {
    Config::load_at(config_path_override)
        .ok()?
        .servers
        .get(server_name)?
        .bug_field_names
        .clone()
}

/// Persist freshly probed names under the config lock. Mirrors
/// `persist_detected_settings`: only a successful probe writes, and a server
/// that is no longer in config is a logged no-op rather than a resurrection.
fn persist_names(
    config_path_override: Option<&std::path::Path>,
    server_name: &str,
    names: &[String],
) -> Result<()> {
    Config::update_locked_at(config_path_override, |config| {
        let Some(srv) = config.servers.get_mut(server_name) else {
            tracing::debug!("server '{server_name}' not in config; skipping field-name persist");
            return Ok(());
        };
        srv.bug_field_names = Some(names.to_vec());
        Ok(())
    })?;
    Ok(())
}

/// True for a bug field name bzr's own REST payloads already use.
///
/// The catalogue answers with Bugzilla's internal column names for many
/// built-ins — `status_whiteboard` for `whiteboard`, `short_desc` for
/// `summary`, `rep_platform` for `platform` — while `Bug.create` and
/// `Bug.update` take the REST names. A catalogue-only check would therefore
/// reject `--field whiteboard=...`, which is exactly the case the
/// python-bugzilla comparison drives. `BUG_FIELDS` is the REST bug-field list
/// bzr already maintains for `--fields`, so reusing it keeps the accepted set
/// in step with the names bzr knows the server's REST layer speaks, without a
/// second hand-written alias table to drift.
fn is_bzr_known_bug_field(key: &str) -> bool {
    crate::types::bug::BUG_FIELDS
        .iter()
        .any(|field| field.canonical() == key)
}

/// Refuse the write unless every key in `keys` is a field the server declares
/// or a REST bug field bzr itself models.
///
/// A no-op for an empty key set, so callers pay nothing when `--field` was not
/// used. Keys bzr already models, and a key set fully covered by the cached
/// names, cost no network call.
pub(crate) async fn validate_bug_fields(
    client: &BugzillaClient,
    ctx: &CommandContext,
    keys: &BTreeSet<String>,
) -> Result<()> {
    let unknown: Vec<&str> = keys
        .iter()
        .map(String::as_str)
        .filter(|key| !is_bzr_known_bug_field(key))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let server_name = client.server_name();
    let config_path = ctx.config_path_override();
    if let Some(cached) = cached_names(config_path, server_name) {
        let cached: BTreeSet<&str> = cached.iter().map(String::as_str).collect();
        if unknown.iter().all(|key| cached.contains(key)) {
            return Ok(());
        }
    }
    let declared = client
        .bug_field_names()
        .await
        .map_err(annotate_probe_failure)?;
    persist_names(config_path, server_name, &declared)?;
    let declared: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    for key in unknown {
        if !declared.contains(key) {
            return Err(undeclared(key));
        }
    }
    Ok(())
}

/// Connect, then refuse the write unless the server declares every key.
/// Drop-in replacement for `connect_and_configure` on the bug write paths, so
/// the validation cannot be forgotten when a new write path copies the shape.
pub(crate) async fn connect_and_validate_bug_fields(
    ctx: &CommandContext,
    keys: &BTreeSet<String>,
) -> Result<BugzillaClient> {
    let client = super::connect_and_configure(ctx).await?;
    validate_bug_fields(&client, ctx, keys).await?;
    Ok(client)
}

#[cfg(test)]
#[path = "field_catalogue_tests.rs"]
mod tests;
