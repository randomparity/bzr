use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum AuthAction {
    /// Log in with a Bugzilla email and password and save the returned token.
    ///
    /// The selected server must be named and must not already use an API-key
    /// credential source. Omitting `--password` prompts with hidden input;
    /// use `--restrict-login` when the server supports restricted sessions.
    ///
    /// Examples:
    ///
    ///   bzr --server prod auth login --email alice@example.com
    ///   bzr --server prod auth login --email alice@example.com --restrict-login
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        restrict_login: bool,
    },
    /// Invalidate the saved login token and remove it from local configuration.
    ///
    /// The remote logout request runs first. A failed request leaves the local
    /// token intact so it can be retried rather than silently losing access.
    ///
    /// Examples:
    ///
    ///   bzr --server prod auth logout
    Logout,
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
