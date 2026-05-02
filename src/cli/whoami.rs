use clap::Subcommand;

/// Whoami action -- shows the authenticated user's identity.
#[derive(Subcommand)]
pub enum WhoamiAction {
    /// Show the login, real name, and email of the authenticated user.
    ///
    /// Issues a single API call (`GET /rest/whoami` on REST, or the
    /// equivalent on XML-RPC) using the configured server's
    /// credentials and prints the resolved user's login, real name,
    /// and email. Useful as a quick auth smoke test before running
    /// commands that depend on credentials.
    ///
    /// `bzr whoami` (with no subcommand) is equivalent to
    /// `bzr whoami show` -- the explicit `show` form exists so the
    /// command is consistent with the verb-after-resource pattern of
    /// the rest of bzr.
    ///
    /// Examples:
    ///
    ///   bzr whoami show
    ///   bzr whoami show --json
    ///   bzr --server staging whoami show
    ///
    /// Exit codes: 0 on success, 9 on auth failure (key invalid or
    /// missing), 13 on TLS pin mismatch.
    #[command(verbatim_doc_comment)]
    Show,
}
