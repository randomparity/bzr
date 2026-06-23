use clap::Subcommand;

use crate::types::common::AuthMethod;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum ConfigAction {
    /// Add or update a named server in the local config.
    ///
    /// `--url` is required. `--api-key` (inline) and `--api-key-env`
    /// (env-var indirection) are optional; omit both for public read-only
    /// servers. Writes and identity-derived commands require one credential
    /// source, and using the OS keychain is a separate step
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
    ///   bzr config set-server prod --url <https://bz.example.com> \
    ///     --api-key-env BZR_API_KEY
    ///   bzr config set-server staging --url <https://stage.example.com> \
    ///     --api-key-env STAGE_KEY --tls-pin-now
    ///   bzr config set-server self-hosted --url <https://bz.local> \
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
        /// Mutually exclusive with `--api-key-env`. Inline keys can leak via
        /// shell history, process args, or backup copies of `config.toml` --
        /// prefer `--api-key-env` or the keyring for anything beyond a
        /// throwaway test setup.
        #[arg(long, conflicts_with = "api_key_env")]
        api_key: Option<String>,
        /// Name of an environment variable that holds the API key.
        ///
        /// Mutually exclusive with `--api-key`. The variable is resolved at
        /// command time, not at `set-server` time, so rotating the key only
        /// requires updating the env var (or the secret store backing it).
        /// Variable names are stored verbatim in the config file; the secret
        /// itself is not.
        #[arg(long, conflicts_with = "api_key")]
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
    /// entry, or `"<inline>"` for inline keys). Inline API keys are
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
    /// service/account configured by `set-keyring`) and clears the
    /// server's keyring credential reference. The server entry is preserved
    /// with no API key source, so public reads can still work; configure
    /// `--api-key-env`, `--api-key`, or `set-keyring` before writes.
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

    /// Copy an existing inline or env-var API key into the OS keychain.
    ///
    /// Reads the server's currently configured key (whether stored
    /// inline as `api_key` or read from the env var named by
    /// `api_key_env`) and stores it in the OS keychain. `--yes` is
    /// required to confirm the non-interactive migration. Inline
    /// sources are rewritten to use the keychain; env-var sources
    /// leave `config.toml` unchanged so shared env vars are not
    /// removed implicitly. `--service` and `--account` override the
    /// default keychain naming
    /// (`bzr` / `<server-name>`).
    ///
    /// If the env-var path is in use and the variable is unset at
    /// migration time, the command fails with exit code 7 (input
    /// validation).
    ///
    /// Examples:
    ///
    ///   bzr config migrate-to-keyring prod --yes
    ///   bzr config migrate-to-keyring staging --service bzr --yes
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
        /// Acknowledge and run the migration.
        ///
        /// The command exits before reading or writing keychain state unless
        /// this flag is present.
        #[arg(long)]
        yes: bool,
    },

    /// Remove a named server from the local config.
    ///
    /// Deletes the `[servers.<name>]` block and, if the server stored
    /// its API key in the OS keychain, removes that keychain entry too
    /// (idempotently — a missing entry is not an error). The server
    /// must exist.
    ///
    /// Removing the current default server is refused while other
    /// servers remain: set a new default first with
    /// `bzr config set-default <other>`. Removing the only configured
    /// server is allowed and leaves the config with no default.
    ///
    /// Examples:
    ///
    ///   bzr config remove-server staging
    ///   bzr --json config remove-server throwaway
    ///
    /// See bzr-config-set-default(1) to reassign the default before
    /// removing it and bzr-config-rename-server(1) to rename instead.
    #[command(verbatim_doc_comment)]
    RemoveServer {
        /// Server alias name (must exist).
        name: String,
    },

    /// Rename a server alias, preserving its credentials.
    ///
    /// Moves the `[servers.<old>]` block to `[servers.<new>]` with every
    /// field intact. If the server's API key lives in the OS keychain
    /// under the default account (the server name), the stored secret is
    /// moved to the new account so credentials keep working. If
    /// `default_server` pointed at `<old>`, it is updated to `<new>`.
    ///
    /// `<old>` must exist and `<new>` must not already exist.
    ///
    /// Examples:
    ///
    ///   bzr config rename-server stage staging
    ///   bzr --json config rename-server old-name new-name
    ///
    /// See bzr-config-remove-server(1) to delete a server instead.
    #[command(verbatim_doc_comment)]
    RenameServer {
        /// Current server alias (must exist).
        old: String,
        /// New server alias (must not already exist).
        new: String,
    },
}
