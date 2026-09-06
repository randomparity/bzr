//! Server capability gating for vendor-extension parameters (ADR-0052).
//!
//! Some Bugzilla search and mutation parameters are Red Hat extensions rather
//! than upstream API. Upstream Bugzilla accepts them and silently discards
//! them, so forwarding one produces a result that looks filtered and is not.
//! bzr therefore establishes support before dispatching and refuses otherwise.
//!
//! The capability is read from the per-server detection state in config,
//! probed through `GET /rest/extensions` on a cache miss, and written back —
//! the same shape as the auth-method / API-mode / server-version state that
//! `connection::detect` already persists.

use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};

/// The Bugzilla extension that provides saved-search resolution on Red Hat
/// Bugzilla. Presence of this extension is a *proxy* for a patched
/// `Bug.search`, not proof of one — see ADR-0052's consequences.
pub(crate) const RED_HAT_EXTENSION: &str = "RedHat";

/// Ensure `capability` is advertised by the server, or fail before dispatch.
///
/// `operation` is the user-facing first clause of the error message (e.g.
/// `saved search 'triage'`), so the message reads as operation, cause, fix.
///
/// Three outcomes, deliberately distinct: advertised, not advertised, and
/// undetermined. Collapsing the third into the second would let a transient
/// network fault masquerade as a statement about the server's capabilities.
pub(crate) async fn require_server_capability(
    ctx: &CommandContext,
    client: &BugzillaClient,
    capability: &str,
    operation: &str,
) -> Result<()> {
    let extensions = resolve_extensions(ctx, client, capability).await?;
    if extensions.iter().any(|name| name == capability) {
        Ok(())
    } else {
        Err(unsupported(capability, operation))
    }
}

/// Cached-or-probed extension names for the connected server.
///
/// A probe failure is an `Err`, never an empty list, so the caller cannot
/// accidentally treat "could not ask" as "not supported".
async fn resolve_extensions(
    ctx: &CommandContext,
    client: &BugzillaClient,
    capability: &str,
) -> Result<Vec<String>> {
    // An inline `--server-url` connection has no config entry, so there is
    // nothing to read from and nothing to write back: probe every time.
    let cached_server = if ctx.inline_server().is_some() {
        None
    } else {
        cached_server_name_and_extensions(ctx)
    };

    if let Some((_, Some(extensions))) = &cached_server {
        return Ok(extensions.clone());
    }

    let mut names: Vec<String> = client
        .server_extensions()
        .await
        .map_err(|e| undetermined(capability, &e))?
        .extensions
        .into_keys()
        .collect();
    names.sort_unstable();

    if let Some((server_name, _)) = cached_server {
        persist_extensions(ctx, &server_name, &names);
    }
    Ok(names)
}

/// Read the configured server's name and any cached extension list.
///
/// A configuration that cannot be read or has no resolvable server is not this
/// helper's problem to report — the connection it was handed already succeeded —
/// so it degrades to an uncached probe rather than failing the command here.
fn cached_server_name_and_extensions(
    ctx: &CommandContext,
) -> Option<(String, Option<Vec<String>>)> {
    let config = Config::load_at(ctx.config_path_override()).ok()?;
    let (name, srv) = config.resolve_server(ctx.server()).ok()?;
    Some((name.to_string(), srv.server_extensions.clone()))
}

/// Cache the probed extension list under the config lock.
///
/// Best-effort: a server removed concurrently, or a config that cannot be
/// written, costs one extra probe next time and is not worth failing a
/// successful command over. Logged, not silent.
fn persist_extensions(ctx: &CommandContext, server_name: &str, names: &[String]) {
    let result = Config::update_locked_at(ctx.config_path_override(), |config| {
        if let Some(srv) = config.servers.get_mut(server_name) {
            srv.server_extensions = Some(names.to_vec());
        }
        Ok(())
    });
    if let Err(e) = result {
        tracing::debug!("could not cache server extensions for '{server_name}': {e}");
    }
}

fn unsupported(capability: &str, operation: &str) -> BzrError {
    BzrError::UnsupportedServerCapability {
        capability: capability.to_string(),
        operation: operation.to_string(),
        detail: format!(
            "server does not implement the Bugzilla '{capability}' extension \
             (not advertised at /rest/extensions). Stock Bugzilla accepts this \
             parameter and ignores it, so bzr refuses rather than returning an \
             unfiltered result; use `bzr bug list` filters, or `bzr query` for a \
             saved query stored locally"
        ),
    }
}

fn undetermined(capability: &str, error: &BzrError) -> BzrError {
    BzrError::UnsupportedServerCapability {
        capability: capability.to_string(),
        operation: "server capability check".to_string(),
        detail: format!(
            "could not determine whether the server implements the Bugzilla \
             '{capability}' extension: reading /rest/extensions failed ({error}). \
             This is not evidence the extension is absent; retry, or check \
             connectivity to the server"
        ),
    }
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
