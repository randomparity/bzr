use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum AuthAction {
    /// Log in with a Bugzilla email and password and save the returned token.
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        restrict_login: bool,
    },
    /// Invalidate the saved login token and remove it from local configuration.
    Logout,
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
