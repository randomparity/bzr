use clap::Subcommand;

#[derive(Subcommand)]
pub enum ServerAction {
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
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
