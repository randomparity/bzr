use clap::Subcommand;

use crate::types::AuthMethod;

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set up a server
    SetServer {
        /// Server alias name
        name: String,
        /// Server URL
        #[arg(long)]
        url: String,
        /// API key (less secure: may leak via shell history or process args)
        #[arg(
            long,
            conflicts_with = "api_key_env",
            required_unless_present = "api_key_env"
        )]
        api_key: Option<String>,
        /// Name of an environment variable that contains the API key
        #[arg(long, conflicts_with = "api_key", required_unless_present = "api_key")]
        api_key_env: Option<String>,
        /// Login email (required for older Bugzilla servers)
        #[arg(long)]
        email: Option<String>,
        /// Override auto-detected auth method (`header` or `query_param`)
        #[arg(long)]
        auth_method: Option<AuthMethod>,
        /// Accept invalid TLS certificates (self-signed, expired, wrong host)
        #[arg(
            long,
            conflicts_with_all = ["tls_ca_cert", "tls_pin_sha256", "tls_pin_now"],
        )]
        tls_insecure: bool,
        /// Path to a PEM CA certificate file for this server
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_pin_sha256", "tls_pin_now"],
        )]
        tls_ca_cert: Option<String>,
        /// Pin a certificate fingerprint (sha256//<base64> format)
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_ca_cert", "tls_pin_now", "tls_pin_clear"],
        )]
        tls_pin_sha256: Option<String>,
        /// Connect to server and pin its current certificate
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_ca_cert", "tls_pin_sha256", "tls_pin_clear"],
        )]
        tls_pin_now: bool,
        /// Remove a stored certificate pin
        #[arg(
            long,
            conflicts_with_all = ["tls_pin_sha256", "tls_pin_now"],
        )]
        tls_pin_clear: bool,
    },
    /// Set the default server
    SetDefault {
        /// Server alias name
        name: String,
    },
    /// Show current configuration
    Show,
    /// Store an API key for a server in the OS keychain (prompts stdin)
    SetKeyring {
        /// Server alias name (must already exist)
        name: String,
        /// Override keyring service name (defaults to "bzr")
        #[arg(long)]
        service: Option<String>,
        /// Override keyring account name (defaults to the server name)
        #[arg(long)]
        account: Option<String>,
    },
    /// Remove a server's API key from the OS keychain
    UnsetKeyring {
        /// Server alias name
        name: String,
    },
    /// Migrate a server's existing inline/env API key into the OS keychain
    MigrateToKeyring {
        /// Server alias name
        name: String,
        /// Override keyring service name (defaults to "bzr")
        #[arg(long)]
        service: Option<String>,
        /// Override keyring account name (defaults to the server name)
        #[arg(long)]
        account: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}
