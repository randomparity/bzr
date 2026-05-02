use clap::Subcommand;

use crate::types::AuthMethod;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum ConfigAction {
    /// Add or update a named server in the local config.
    ///
    /// `--url` is required. Exactly one of `--api-key` (inline) or
    /// `--api-key-env` (env-var indirection) must be supplied; using
    /// the OS keychain instead is a separate step
    /// (`bzr config set-keyring`).
    ///
    /// TLS handling is mutually exclusive across these flags:
    /// `--tls-insecure` (accept any cert), `--tls-ca-cert <path>`
    /// (custom CA), `--tls-pin-sha256 <fp>` (pin a fingerprint), and
    /// `--tls-pin-now` (connect once to capture the current cert and
    /// pin it). `--tls-pin-clear` removes a stored pin.
    ///
    /// `--auth-method` overrides bzr's auto-detection of header
    /// vs. query-param API-key transport. Most servers don't need
    /// this -- bzr probes on first use and caches the working
    /// method.
    ///
    /// Examples:
    ///
    ///   bzr config set-server prod --url https://bz.example.com \
    ///     --api-key-env BZR_API_KEY
    ///   bzr config set-server staging --url https://stage.example.com \
    ///     --api-key-env STAGE_KEY --tls-pin-now
    ///   bzr config set-server self-hosted --url https://bz.local \
    ///     --api-key-env BZR_API_KEY \
    ///     --tls-ca-cert /etc/pki/tls/local-ca.pem
    ///
    /// See bzr-config-set-default(1) to pick which server `--server`
    /// resolves to by default and bzr-config-set-keyring(1) for OS
    /// keychain credential storage.
    #[command(verbatim_doc_comment)]
    SetServer {
        /// Server alias name
        name: String,
        /// Server URL
        #[arg(long)]
        url: String,
        /// API key, stored inline in the config file.
        ///
        /// Mutually exclusive with `--api-key-env`; one of the two
        /// is required (or use `bzr config set-keyring` to keep
        /// the secret in the OS keychain instead). Inline keys can
        /// leak via shell history, process args, or backup copies
        /// of `config.toml` -- prefer `--api-key-env` or the
        /// keyring for anything beyond a throwaway test setup.
        #[arg(
            long,
            conflicts_with = "api_key_env",
            required_unless_present = "api_key_env"
        )]
        api_key: Option<String>,
        /// Name of an environment variable that holds the API key.
        ///
        /// Mutually exclusive with `--api-key`. The variable is
        /// resolved at command time, not at `set-server` time, so
        /// rotating the key only requires updating the env var
        /// (or the secret store backing it). Variable names are
        /// stored verbatim in the config file; the secret itself
        /// is not.
        #[arg(long, conflicts_with = "api_key", required_unless_present = "api_key")]
        api_key_env: Option<String>,
        /// Login email used for fallback auth on older Bugzilla servers.
        ///
        /// Required only when bzr's auto-detected auth method
        /// (header API-key) is unavailable and the server falls
        /// back to query-parameter auth, which uses
        /// `email`+`api_key` as a credential pair. Most modern
        /// Bugzilla servers don't need this.
        #[arg(long)]
        email: Option<String>,
        /// Override bzr's auto-detected API-key transport.
        ///
        /// Accepted values: `header` (use the
        /// `X-BUGZILLA-API-KEY` HTTP header) or `query-param` (use
        /// the `?api_key=...` query parameter). bzr probes both on
        /// first use and caches the working method per server;
        /// override only when the cached value is wrong (e.g. the
        /// server changed configuration).
        #[arg(long)]
        auth_method: Option<AuthMethod>,
        /// Accept invalid TLS certificates -- self-signed, expired, wrong host.
        ///
        /// Disables every TLS validation check for this server.
        /// Use only against a server you control or in a trusted
        /// development environment; the server's responses cannot
        /// be authenticated. Mutually exclusive with
        /// `--tls-ca-cert`, `--tls-pin-sha256`, and
        /// `--tls-pin-now`. Prefer one of those for self-signed or
        /// pinned-cert deployments.
        #[arg(
            long,
            conflicts_with_all = ["tls_ca_cert", "tls_pin_sha256", "tls_pin_now"],
        )]
        tls_insecure: bool,
        /// Path to a PEM-encoded CA certificate file for this server.
        ///
        /// Adds the given CA to the trust store for this server
        /// without affecting other servers or the system trust
        /// store. Useful for self-hosted Bugzilla instances behind
        /// a private CA. Mutually exclusive with `--tls-insecure`,
        /// `--tls-pin-sha256`, and `--tls-pin-now`.
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_pin_sha256", "tls_pin_now"],
        )]
        tls_ca_cert: Option<String>,
        /// Pin a certificate fingerprint in `sha256//<base64>` format.
        ///
        /// The exact format used by curl's `--pinnedpubkey`. Once
        /// pinned, every subsequent connection to this server
        /// must present a leaf certificate whose SHA-256 hash
        /// matches; mismatches exit with code 13. Mutually
        /// exclusive with `--tls-insecure`, `--tls-ca-cert`,
        /// `--tls-pin-now`, and `--tls-pin-clear`. Use
        /// `--tls-pin-now` to capture the current cert
        /// automatically instead of computing the fingerprint by
        /// hand.
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_ca_cert", "tls_pin_now", "tls_pin_clear"],
        )]
        tls_pin_sha256: Option<String>,
        /// Connect to the server and pin its current certificate.
        ///
        /// Issues a one-shot TLS connection, captures the leaf
        /// certificate's SHA-256 fingerprint, and stores it as the
        /// pin (TOFU -- trust on first use). Subsequent connections
        /// require the same fingerprint. Mutually exclusive with
        /// `--tls-insecure`, `--tls-ca-cert`, `--tls-pin-sha256`,
        /// and `--tls-pin-clear`.
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_ca_cert", "tls_pin_sha256", "tls_pin_clear"],
        )]
        tls_pin_now: bool,
        /// Remove a stored certificate pin from this server.
        ///
        /// Reverts the server to default TLS validation against
        /// the OS trust store. Mutually exclusive with
        /// `--tls-pin-sha256` and `--tls-pin-now` -- use one of
        /// those to install a new pin in the same call as clearing
        /// the old one is not supported.
        #[arg(
            long,
            conflicts_with_all = ["tls_pin_sha256", "tls_pin_now"],
        )]
        tls_pin_clear: bool,
    },
    /// Pick which server is used when `--server` is not specified.
    ///
    /// The default server is used by every command that doesn't
    /// pass an explicit `--server <name>`. The named server must
    /// already exist in the config.
    ///
    /// Examples:
    ///
    ///   bzr config set-default prod
    ///   bzr config set-default staging
    ///
    /// See bzr-config-set-server(1) to add a server before making
    /// it default and bzr-config-show(1) to verify the current
    /// default.
    #[command(verbatim_doc_comment)]
    SetDefault {
        /// Server alias name
        name: String,
    },

    /// Print the current configuration.
    ///
    /// Lists every configured server with its URL, default-flag
    /// status, and credential indirection (env-var name, keyring
    /// entry, or "<inline>" for inline keys). Inline API keys are
    /// redacted -- the secret never appears in this output. With
    /// `--json`, the same data is emitted as a JSON object suitable
    /// for scripting.
    ///
    /// Use this to confirm config-file location, the resolved
    /// default server, and which credential channel each server
    /// uses.
    ///
    /// Examples:
    ///
    ///   bzr config show
    ///   bzr config show --json | jq '.servers[] | .url'
    ///
    /// See bzr-config-set-server(1) to add or modify entries.
    #[command(verbatim_doc_comment)]
    Show,

    /// Store an API key for a server in the OS keychain.
    ///
    /// Prompts on stdin for the API key (input is hidden). Stores
    /// the key in the platform's native credential store (Keychain
    /// on macOS, Secret Service / GNOME Keyring on Linux, Windows
    /// Credential Manager on Windows) under the service name `bzr`
    /// and account `<server-name>`, both of which can be overridden
    /// with `--service` and `--account`.
    ///
    /// After this completes, the server's stored `api_key` /
    /// `api_key_env` field is replaced with a keyring marker, and
    /// the secret is read from the keychain on each invocation.
    ///
    /// Examples:
    ///
    ///   bzr config set-keyring prod
    ///   bzr config set-keyring shared --service bzr-team --account ci
    ///
    /// Exit codes: 0 on success, 12 on keyring access errors
    /// (locked keychain, daemon not running, permission denied).
    ///
    /// See bzr-config-unset-keyring(1) to remove a stored key,
    /// bzr-config-migrate-to-keyring(1) to move an existing inline
    /// or env key into the keychain, and the project README's
    /// "Credential storage" section for platform setup notes.
    #[command(verbatim_doc_comment)]
    SetKeyring {
        /// Server alias name (must already exist).
        name: String,
        /// Override the keyring service name (defaults to `bzr`).
        ///
        /// The service name groups related credentials in the OS
        /// keychain. Override when sharing credentials across
        /// multiple bzr installs or when the default `bzr`
        /// collides with another tool's entries.
        #[arg(long)]
        service: Option<String>,
        /// Override the keyring account name (defaults to the server name).
        ///
        /// The account name identifies an individual credential
        /// within the service. Override when storing multiple
        /// keys for the same server (e.g. personal vs. CI).
        #[arg(long)]
        account: Option<String>,
    },
    /// Remove a server's API key from the OS keychain.
    ///
    /// Deletes the keychain entry for `<server-name>` (or the
    /// service/account configured by `set-keyring`) and reverts the
    /// server's config to its inline / env-var credential
    /// indirection. Use this when rotating keys, decommissioning a
    /// server, or switching back to env-var or inline storage.
    ///
    /// Examples:
    ///
    ///   bzr config unset-keyring prod
    ///
    /// Exit codes: 0 on success, 12 on keyring access errors.
    ///
    /// See bzr-config-set-keyring(1) for the inverse operation.
    #[command(verbatim_doc_comment)]
    UnsetKeyring {
        /// Server alias name
        name: String,
    },

    /// Move an existing inline or env-var API key into the OS keychain.
    ///
    /// Reads the server's currently configured key (whether stored
    /// inline as `api_key` or read from the env var named by
    /// `api_key_env`), stores it in the OS keychain, and rewrites
    /// the server's config to use the keychain. Prompts for
    /// confirmation unless `--yes` is passed. `--service` and
    /// `--account` override the default keychain naming
    /// (`bzr` / `<server-name>`).
    ///
    /// If the env-var path is in use and the variable is unset at
    /// migration time, the command fails with exit code 7 (input
    /// validation).
    ///
    /// Examples:
    ///
    ///   bzr config migrate-to-keyring prod
    ///   bzr config migrate-to-keyring staging --yes
    ///
    /// See bzr-config-set-keyring(1) for storing a fresh key
    /// (without reading from the existing config) and
    /// bzr-config-unset-keyring(1) for the inverse direction.
    #[command(verbatim_doc_comment)]
    MigrateToKeyring {
        /// Server alias name.
        name: String,
        /// Override the keyring service name (defaults to `bzr`).
        #[arg(long)]
        service: Option<String>,
        /// Override the keyring account name (defaults to the server name).
        #[arg(long)]
        account: Option<String>,
        /// Skip the confirmation prompt before migrating.
        ///
        /// Without this flag, the command prints the source of
        /// the existing key (inline vs. env var) and waits for a
        /// `y` on stdin before writing to the keychain. Useful
        /// for scripted migrations across many servers.
        #[arg(long)]
        yes: bool,
    },
}
