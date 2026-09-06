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
use crate::error::{BzrError, Result, CAPABILITY_ABSENT, CAPABILITY_UNDETERMINED};

/// The Bugzilla extension that provides saved-search resolution on Red Hat
/// Bugzilla. Presence of this extension is a *proxy* for a patched
/// `Bug.search`, not proof of one — see ADR-0052's consequences.
pub(crate) const RED_HAT_EXTENSION: &str = "RedHat";

/// Capabilities bzr can act on. Only these are cached: the probe response is
/// server-controlled and unbounded, and persisting the whole advertised list
/// would write arbitrary server text into the user's config for no gain — the
/// only consumer is a membership test against this table.
const KNOWN_CAPABILITIES: &[&str] = &[RED_HAT_EXTENSION];

/// Label for a server whose configuration could not be read at message time.
const UNKNOWN_SERVER_LABEL: &str = "(unresolved server)";

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
    let server = server_label(ctx);
    let cacheable = ctx.inline_server().is_none();
    let extensions = resolve_extensions(ctx, client, capability, operation, &server).await?;
    if extensions.iter().any(|name| name == capability) {
        Ok(())
    } else {
        Err(unsupported(capability, operation, &server, cacheable))
    }
}

/// How the connected server is named in a message.
///
/// An inline connection has no configured name, so it is named by the host it
/// actually probed — sanitized, because an inline URL can carry an API key in
/// a query parameter.
fn server_label(ctx: &CommandContext) -> String {
    if let Some(inline) = ctx.inline_server() {
        // Origin + path only: an inline URL can carry an API key in a query
        // parameter, and this string reaches stderr and the JSON error body.
        return match reqwest::Url::parse(&inline.url) {
            Ok(url) => format!(
                "at {}{}",
                url.origin().ascii_serialization(),
                url.path().trim_end_matches('/')
            ),
            Err(_) => UNKNOWN_SERVER_LABEL.to_string(),
        };
    }
    cached_server_name_and_extensions(ctx).map_or_else(
        || UNKNOWN_SERVER_LABEL.to_string(),
        |server| format!("'{}'", server.name),
    )
}

/// Cached-or-probed extension names for the connected server.
///
/// A probe failure is an `Err`, never an empty list, so the caller cannot
/// accidentally treat "could not ask" as "not supported".
async fn resolve_extensions(
    ctx: &CommandContext,
    client: &BugzillaClient,
    capability: &str,
    operation: &str,
    server: &str,
) -> Result<Vec<String>> {
    // An inline `--server-url` connection has no config entry, so there is
    // nothing to read from and nothing to write back: probe every time.
    let cached_server = if ctx.inline_server().is_some() {
        None
    } else {
        cached_server_name_and_extensions(ctx)
    };

    if let Some(CachedServer {
        extensions: Some(extensions),
        ..
    }) = &cached_server
    {
        return Ok(extensions.clone());
    }

    let advertised = client
        .server_extensions()
        .await
        .map_err(|e| undetermined(capability, operation, server, &e))?
        .extensions;
    let mut names: Vec<String> = KNOWN_CAPABILITIES
        .iter()
        .filter(|known| advertised.contains_key(**known))
        .map(|known| (*known).to_string())
        .collect();
    names.sort_unstable();

    if let Some(server) = cached_server {
        persist_extensions(ctx, &server, &names);
    }
    Ok(names)
}

/// Read the configured server's name and any cached extension list.
///
/// A configuration that cannot be read or has no resolvable server is not this
/// helper's problem to report — the connection it was handed already succeeded —
/// so it degrades to an uncached probe rather than failing the command here.
fn cached_server_name_and_extensions(ctx: &CommandContext) -> Option<CachedServer> {
    let config = Config::load_at(ctx.config_path_override()).ok()?;
    let (name, srv) = config.resolve_server(ctx.server()).ok()?;
    // Trust the cache only while it still describes this URL *and* was written
    // against the same capability allowlist. A name re-pointed at another host
    // would otherwise inherit an answer that lets the gate pass for a server
    // that never advertised it; a cache written before a capability was added
    // to the table would otherwise report it as "not advertised".
    let url_matches = srv.server_extensions_url.as_deref() == Some(srv.url.as_str());
    let allowlist_matches = srv
        .server_extensions_known
        .as_deref()
        .is_some_and(|known| known == known_capabilities());
    let cached = if url_matches && allowlist_matches {
        srv.server_extensions.clone()
    } else {
        None
    };
    Some(CachedServer {
        name: name.to_string(),
        url: srv.url.clone(),
        extensions: cached,
    })
}

/// The configured server a capability answer belongs to.
struct CachedServer {
    name: String,
    /// URL at the moment of the read — the probe is issued against this, and
    /// the write is skipped if the entry has since been re-pointed.
    url: String,
    extensions: Option<Vec<String>>,
}

/// `KNOWN_CAPABILITIES` as an owned sorted vector, for comparison against the
/// snapshot persisted alongside a cached answer.
fn known_capabilities() -> Vec<String> {
    let mut known: Vec<String> = KNOWN_CAPABILITIES
        .iter()
        .map(|c| (*c).to_string())
        .collect();
    known.sort_unstable();
    known
}

/// Cache the probed extension list under the config lock.
///
/// Best-effort: a server removed concurrently, or a config that cannot be
/// written, costs one extra probe next time and is not worth failing a
/// successful command over. Logged, not silent.
fn persist_extensions(ctx: &CommandContext, server: &CachedServer, names: &[String]) {
    let server_name = server.name.as_str();
    let probed_url = server.url.as_str();
    let result = Config::update_locked_at(ctx.config_path_override(), |config| {
        if let Some(srv) = config.servers.get_mut(server_name) {
            // The entry may have been re-pointed between the read and this
            // write. Stamping the new URL onto an answer probed from the old
            // host is exactly the fail-open the URL binding exists to prevent,
            // so skip instead — costing one extra probe next time.
            if srv.url == probed_url {
                srv.server_extensions_url = Some(probed_url.to_string());
                srv.server_extensions_known = Some(known_capabilities());
                srv.server_extensions = Some(names.to_vec());
            } else {
                tracing::debug!(
                    "server '{server_name}' was re-pointed during the capability probe; \
                     not caching the result"
                );
            }
        }
        Ok(())
    });
    if let Err(e) = result {
        tracing::debug!("could not cache server extensions for '{server_name}': {e}");
    }
}

fn unsupported(capability: &str, operation: &str, server: &str, cacheable: bool) -> BzrError {
    // Only a configured server has a cached answer to explain; an inline
    // `--server-url` connection probes every time.
    let cache_note = if cacheable {
        " This answer is cached per server: if the server has since been \
         upgraded, clear `server_extensions` for it in config.toml to re-probe."
    } else {
        ""
    };
    BzrError::UnsupportedServerCapability {
        capability: capability.to_string(),
        status: CAPABILITY_ABSENT,
        operation: operation.to_string(),
        detail: format!(
            "server {server} does not implement the Bugzilla '{capability}' \
             extension (not advertised at /rest/extensions). Stock Bugzilla \
             accepts this parameter and ignores it, so bzr refuses rather than \
             returning an unfiltered result; use `bzr bug list` filters, or \
             `bzr query` for a saved query stored locally.{cache_note}"
        ),
    }
}

fn undetermined(capability: &str, operation: &str, server: &str, error: &BzrError) -> BzrError {
    BzrError::UnsupportedServerCapability {
        capability: capability.to_string(),
        status: CAPABILITY_UNDETERMINED,
        operation: operation.to_string(),
        detail: format!(
            "could not determine whether server {server} implements the Bugzilla \
             '{capability}' extension: reading /rest/extensions failed ({error}). \
             The probe needs the server's REST surface even when --api xmlrpc is \
             in use. This is not evidence the extension is absent; retry, or \
             check that REST is reachable"
        ),
    }
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
