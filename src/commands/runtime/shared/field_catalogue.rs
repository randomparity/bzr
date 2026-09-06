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

use std::collections::{BTreeMap, BTreeSet};

use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::types::{FieldName, FieldNameSource};

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

/// Point at the command that answers "what can I set here".
///
/// `bzr field list` with no argument enumerates the whole accepted set — the
/// server's catalogue names and the REST names bzr models — which is exactly
/// the set this function guards (ADR 0062). Earlier wording named
/// `bzr server capabilities` and stopped at "custom fields", because before
/// issue #718 no command could show the rest.
fn undeclared(key: &str) -> BzrError {
    BzrError::input_field(
        format!(
            "--field: this server does not declare a field named '{key}'; \
             run `bzr field list` to see every field name this server accepts"
        ),
        "--field",
        Some(key.to_string()),
    )
}

/// Read the names cached by the last successful probe for `server_name`.
/// A missing config, an unreadable one, or a server without an entry all mean
/// "no fast path" rather than an error — the probe answers authoritatively.
///
/// The cache is bound to the URL it was probed from, following
/// `server_extensions_url` (ADR 0052). Re-pointing a server name at a
/// different host must not let the old host's catalogue accept a key the new
/// one does not declare: that key would go on the wire for Bugzilla to ignore
/// silently, which is the failure this validation exists to prevent. A URL
/// mismatch is a cache miss, and a miss always re-probes.
fn cached_names(
    config_path_override: Option<&std::path::Path>,
    server_name: &str,
) -> Option<Vec<String>> {
    let config = Config::load_at(config_path_override).ok()?;
    let server = config.servers.get(server_name)?;
    if server.bug_field_names_url.as_deref()? != server.url {
        tracing::debug!(
            "cached bug field names for '{server_name}' came from a different URL; ignoring"
        );
        return None;
    }
    server.bug_field_names.clone()
}

/// Upper bound on the number of names cached to disk. The config is parsed on
/// every invocation, so a server answering with an implausibly large catalogue
/// must not be able to make every later command slower. bugzilla.redhat.com,
/// the largest deployment bzr targets, declares a few hundred bug fields; above
/// this ceiling the names are used for this request and simply not cached.
const MAX_CACHED_FIELD_NAMES: usize = 4096;

/// Persist freshly probed names under the config lock. Mirrors
/// `persist_detected_settings`: only a successful probe writes, and a server
/// that is no longer in config is a logged no-op rather than a resurrection.
///
/// Never fails the caller. The cache is an optimisation with no role in the
/// answer, so a read-only or locked config must not turn an otherwise valid
/// write into an error — the next invocation simply probes again.
fn persist_names(
    config_path_override: Option<&std::path::Path>,
    server_name: &str,
    names: &[String],
) {
    if names.len() > MAX_CACHED_FIELD_NAMES {
        tracing::debug!(
            "server '{server_name}' declared {} bug fields, above the {MAX_CACHED_FIELD_NAMES} \
             cache ceiling; not caching",
            names.len()
        );
        return;
    }
    let result = Config::update_locked_at(config_path_override, |config| {
        let Some(srv) = config.servers.get_mut(server_name) else {
            tracing::debug!("server '{server_name}' not in config; skipping field-name persist");
            return Ok(());
        };
        srv.bug_field_names = Some(names.to_vec());
        srv.bug_field_names_url = Some(srv.url.clone());
        Ok(())
    });
    if let Err(e) = result {
        tracing::debug!("could not cache bug field names for '{server_name}': {e}");
    }
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

/// Every bug field name `--field` / `--field-json` accepts, given the names the
/// server's catalogue declares, each marked with why it is accepted.
///
/// This is the listing half of the contract [`validate_bug_fields`] enforces:
/// the two read the same two sources — the catalogue and `BUG_FIELDS` via
/// [`is_bzr_known_bug_field`] — so a name this function emits is a name that
/// function accepts. Keeping them in one module is what makes that agreement
/// structural rather than a comment (ADR 0062).
///
/// `BTreeMap` gives sorted, deduplicated output in one pass and collapses a
/// name present in both sources into a single `Both` row.
pub(crate) fn accepted_bug_fields(declared: &[String]) -> Vec<FieldName> {
    let mut rows: BTreeMap<&str, FieldNameSource> = BTreeMap::new();
    for name in declared {
        rows.insert(name.as_str(), FieldNameSource::Server);
    }
    for field in crate::types::bug::BUG_FIELDS {
        let canonical = field.canonical();
        debug_assert!(
            is_bzr_known_bug_field(canonical),
            "accepted_bug_fields and the validator must read BUG_FIELDS the same way"
        );
        rows.entry(canonical)
            .and_modify(|source| *source = FieldNameSource::Both)
            .or_insert(FieldNameSource::Bzr);
    }
    rows.into_iter()
        .map(|(name, source)| FieldName {
            name: name.to_string(),
            source,
        })
        .collect()
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
    persist_names(config_path, server_name, &declared);
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
