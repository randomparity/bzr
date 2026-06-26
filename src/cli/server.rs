use clap::Subcommand;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands and reference JSON field \
              names (max_attachment_size, flag_types); backticks would degrade \
              copy-paste/help-text UX"
)]
pub(crate) enum ServerAction {
    /// Show the configured server's version, extensions, and capabilities.
    ///
    /// Prints the Bugzilla version string, the active API transport
    /// (REST, XML-RPC, or hybrid), and the list of installed
    /// extensions. Use this to confirm connectivity, version-gate
    /// features that require a specific Bugzilla release, or detect
    /// the presence of optional extensions before invoking commands
    /// that depend on them.
    ///
    /// Examples:
    ///
    ///   bzr server info
    ///   bzr --server staging server info --json
    ///   bzr server info --json | jq .extensions
    ///
    /// Exit codes: 0 on success, 5 on HTTP/network error, 9 on
    /// auth failure, 13 on TLS pin mismatch.
    ///
    /// See bzr-whoami(1) for an authentication smoke test.
    #[command(verbatim_doc_comment)]
    Info,

    /// Dump the connected server's capability surface as structured JSON.
    ///
    /// Reports the behavior an agent needs to plan mutations: supported
    /// API transports and auth modes, status-transition summaries, custom
    /// field definitions, attachment-size limit, and feature-support
    /// flags. Complements `server info` (which reports version and
    /// extensions) with what the server actually lets you do.
    ///
    /// Works without a saved config or API key; fields a stock server
    /// does not expose anonymously (for example max_attachment_size) are
    /// reported as null rather than failing. flag_types is null until a
    /// per-product path lands.
    ///
    /// Best paired with --json or --output ndjson; the default table form
    /// is a human-readable summary.
    ///
    /// Examples:
    ///
    ///   bzr server capabilities --json
    ///   bzr --server-url <https://bugzilla.example.com> server capabilities --json
    ///   bzr server capabilities --json | jq .status_transitions
    ///
    /// Exit codes: 0 on success, 5 on HTTP/network error, 9 on auth
    /// failure, 13 on TLS pin mismatch.
    #[command(verbatim_doc_comment)]
    Capabilities,
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
