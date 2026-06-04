use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{BzrError, Result};
use crate::types::{ApiMode, AuthMethod, BugTemplate, SavedQuery};

#[derive(Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Config {
    pub default_server: Option<String>,
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub templates: HashMap<String, BugTemplate>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub queries: HashMap<String, SavedQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServerConfig {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_keyring: Option<KeyringRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<AuthMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_mode: Option<ApiMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    /// Accept invalid TLS certificates (self-signed, expired, etc.).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tls_insecure: bool,
    /// Path to a PEM-encoded CA certificate for this server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ca_cert: Option<PathBuf>,
    /// SHA-256 fingerprint of the pinned server certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_sha256: Option<String>,
    /// Issuer DN stored alongside the pin for rotation detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer: Option<String>,
    /// Base64-encoded raw DER bytes of the issuer SEQUENCE for
    /// tamper-proof issuer comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer_der: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct KeyringRef {
    /// Keyring service name. Defaults to "bzr" when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Account/username within the service. Defaults to the server name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl KeyringRef {
    pub fn service_or_default(&self) -> &str {
        self.service.as_deref().unwrap_or("bzr")
    }

    pub fn account_or_default<'a>(&'a self, server_name: &'a str) -> &'a str {
        self.account.as_deref().unwrap_or(server_name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSourceKind {
    Inline,
    Env,
    Keyring,
}

#[derive(Debug)]
pub enum CredentialSource<'a> {
    Inline(&'a str),
    EnvVar(&'a str),
    Keyring { service: &'a str, account: &'a str },
}

impl CredentialSource<'_> {
    pub fn kind(&self) -> CredentialSourceKind {
        match self {
            CredentialSource::Inline(_) => CredentialSourceKind::Inline,
            CredentialSource::EnvVar(_) => CredentialSourceKind::Env,
            CredentialSource::Keyring { .. } => CredentialSourceKind::Keyring,
        }
    }
}

impl CredentialSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialSourceKind::Inline => "inline",
            CredentialSourceKind::Env => "env",
            CredentialSourceKind::Keyring => "keyring",
        }
    }
}

impl ServerConfig {
    pub fn tls_config(&self, server_name: &str) -> crate::tls::TlsConfig {
        crate::tls::TlsConfig {
            insecure: self.tls_insecure,
            ca_cert_path: self.tls_ca_cert.clone(),
            pin_sha256: self.tls_pin_sha256.clone(),
            pin_issuer_der: self.tls_pin_issuer_der.clone(),
            server_name: Some(server_name.to_string()),
        }
    }

    pub fn validate(&self, server_name: &str) -> Result<()> {
        self.credential_source()
            .map(|_| ())
            .map_err(|err| BzrError::config(format!("server '{server_name}': {err}")))?;
        self.validate_tls(server_name)
    }

    pub fn credential_source(&self) -> Result<CredentialSource<'_>> {
        let count = usize::from(self.api_key.is_some())
            + usize::from(self.api_key_env.is_some())
            + usize::from(self.api_key_keyring.is_some());
        match count {
            0 => Err(BzrError::config(
                "server config must define one of 'api_key', 'api_key_env', or 'api_key_keyring'",
            )),
            1 => {
                if let Some(api_key) = self.api_key.as_deref() {
                    Ok(CredentialSource::Inline(api_key))
                } else if let Some(var_name) = self.api_key_env.as_deref() {
                    Ok(CredentialSource::EnvVar(var_name))
                } else {
                    let r = self.api_key_keyring.as_ref().ok_or_else(|| {
                        BzrError::config("internal: keyring credential unexpectedly missing")
                    })?;
                    // Empty string means "default to the server_name"; the
                    // real account is resolved in resolve_api_key() which
                    // has the server name in scope. We cannot use
                    // KeyringRef::account_or_default here because that would
                    // require plumbing the server name through every caller.
                    Ok(CredentialSource::Keyring {
                        service: r.service_or_default(),
                        account: r.account.as_deref().unwrap_or(""),
                    })
                }
            }
            _ => Err(BzrError::config(
                "server config cannot define multiple API key sources \
                 (api_key, api_key_env, api_key_keyring)",
            )),
        }
    }

    pub fn credential_source_kind(&self) -> Result<CredentialSourceKind> {
        Ok(self.credential_source()?.kind())
    }

    pub fn resolve_api_key(&self, server_name: &str) -> Result<String> {
        match self.credential_source()? {
            CredentialSource::Inline(api_key) => Ok(api_key.to_string()),
            CredentialSource::EnvVar(var_name) => {
                let value = std::env::var(var_name).map_err(|_| {
                    BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is not set"
                    ))
                })?;
                if value.is_empty() {
                    return Err(BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is empty"
                    )));
                }
                Ok(value)
            }
            CredentialSource::Keyring { service, account } => {
                // Empty `account` means "default to server_name" (see the
                // sentinel explanation in credential_source()).
                let account = if account.is_empty() {
                    server_name
                } else {
                    account
                };
                crate::credentials::keyring::retrieve(service, account)
            }
        }
    }

    pub fn validate_tls(&self, server_name: &str) -> Result<()> {
        let ctx = |msg: &str| BzrError::config(format!("server '{server_name}': {msg}"));

        if self.tls_insecure && self.tls_ca_cert.is_some() {
            return Err(ctx("tls_insecure and tls_ca_cert are mutually exclusive"));
        }
        if self.tls_insecure && self.tls_pin_sha256.is_some() {
            return Err(ctx(
                "tls_insecure and tls_pin_sha256 are mutually exclusive",
            ));
        }
        if self.tls_ca_cert.is_some() && self.tls_pin_sha256.is_some() {
            return Err(ctx("tls_ca_cert and tls_pin_sha256 are mutually exclusive"));
        }
        if let Some(path) = &self.tls_ca_cert {
            if !path.exists() {
                return Err(BzrError::config(format!(
                    "server '{server_name}': tls_ca_cert file not found: {}",
                    path.display()
                )));
            }
        }
        if let Some(pin) = &self.tls_pin_sha256 {
            crate::tls::fingerprint::parse_pin(pin)
                .map_err(|e| ctx(&format!("invalid tls_pin_sha256: {e}")))?;
        }
        Ok(())
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(dirs::config_dir)
            .ok_or_else(|| BzrError::config("cannot determine config directory"))?;
        Ok(config_dir.join("bzr").join("config.toml"))
    }

    pub fn load() -> Result<Config> {
        let path = Self::path()?;
        match fs::read_to_string(&path) {
            Ok(content) => {
                Self::warn_on_insecure_permissions(&path);
                let config: Config = toml::from_str(&content)?;
                config.validate()?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        self.write_to_disk()
    }

    /// Persist the config **without** running the credential-source validator.
    ///
    /// Used by `unset-keyring`, which intentionally leaves a server without a
    /// credential source (the user re-credentials it afterward). Applies the
    /// same `0o600`/`0o700` hardening as [`Self::save`] so a recreated config
    /// file is never world-readable.
    pub fn save_without_validation(&self) -> Result<()> {
        self.write_to_disk()
    }

    /// Serialize and write the config to its on-disk path atomically:
    /// a uniquely-named sibling temp is written `0o600`, fsync'd (unix),
    /// renamed over the target (atomic replace), and the directory is
    /// fsync'd (unix) so the rename survives a crash. A concurrent reader
    /// therefore always sees either the complete old or complete new file.
    fn write_to_disk(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            let parent_exists = parent.exists();
            fs::create_dir_all(parent)?;
            if !parent_exists {
                set_private_directory_permissions(parent)?;
            }
            reap_stale_temps(parent);
        }
        let content = toml::to_string_pretty(self)?;
        atomic_write(&path, &content)?;
        Self::warn_on_insecure_permissions(&path);
        Ok(())
    }

    pub fn resolve_server<'a>(
        &'a self,
        server_name: Option<&'a str>,
    ) -> Result<(&'a str, &'a ServerConfig)> {
        let name = self.resolve_server_name_only(server_name)?;
        let srv = self
            .servers
            .get(name)
            .ok_or_else(|| BzrError::config(format!("server '{name}' not found in config")))?;
        Ok((name, srv))
    }

    pub fn resolve_server_name_only<'a>(&'a self, server_name: Option<&'a str>) -> Result<&'a str> {
        server_name
            .or(self.default_server.as_deref())
            .ok_or_else(|| {
                BzrError::config(
                    "no server configured. Run `bzr config set-server <name> --url <url> --api-key-env <env-var>` first",
                )
            })
    }

    fn warn_on_insecure_permissions(path: &std::path::Path) {
        #[cfg(unix)]
        {
            if let Some(parent) = path.parent() {
                warn_if_path_permissions_too_open(parent, 0o077, "config directory");
            }
            if path.exists() {
                warn_if_path_permissions_too_open(path, 0o077, "config file");
            }
        }
    }

    fn validate(&self) -> Result<()> {
        for (name, server) in &self.servers {
            server.validate(name)?;
        }
        Ok(())
    }
}

/// Atomically write `content` to `path`: write a uniquely-named sibling
/// temp file, durably flush it (unix), then rename it over `path`.
/// `rename` replaces the destination atomically on POSIX and on Windows
/// (`MoveFileExW`). The directory is fsync'd on unix so the rename is
/// durable across a crash; on non-unix the concurrent-reader atomicity
/// holds but crash-durability is best-effort.
fn atomic_write(path: &std::path::Path, content: &str) -> Result<()> {
    // Create + write the temp. `create_new` is collision-tolerant: a stale
    // same-pid crash-orphan younger than the reaper's age gate could share
    // the first candidate name, so retry with fresh names before failing.
    let tmp = write_unique_temp(path, content)?;
    // Test-only fault seam: simulate a crash/failure *after* the temp is
    // written but *before* the rename, to deterministically verify that a
    // failed write leaves the previous config intact (CONC-1 atomicity).
    #[cfg(test)]
    if FAIL_AFTER_TEMP.with(std::cell::Cell::get) {
        let _ = fs::remove_file(&tmp);
        return Err(BzrError::config("injected post-temp failure (test)"));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    fsync_parent_dir(path);
    Ok(())
}

#[cfg(test)]
thread_local! {
    /// When set, [`atomic_write`] fails after writing the temp but before the
    /// rename. Lets a test prove a failed write does not destroy the old file.
    static FAIL_AFTER_TEMP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Arm/disarm the [`atomic_write`] post-temp fault seam (test-only).
#[cfg(test)]
pub(crate) fn set_fail_after_temp(on: bool) {
    FAIL_AFTER_TEMP.with(|f| f.set(on));
}

/// Max attempts to find an unused temp name. A collision only happens
/// against a stale same-pid orphan younger than the reaper's age gate, so
/// a few retries with fresh counter values is always enough.
const TEMP_CREATE_ATTEMPTS: u32 = 16;

/// A candidate sibling temp path: `config.toml.<pid>.<counter>.tmp`. The
/// counter is process-global and monotonic, so each call yields a fresh
/// name; combined with the pid this is unique across concurrent writers.
fn candidate_temp_path(path: &std::path::Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{pid}.{n}.tmp"));
    match path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// Create a fresh `0600` sibling temp (collision-tolerant) and write
/// `content` to it durably. Returns the temp path on success. On an
/// `AlreadyExists` collision with a stale orphan, retries with a fresh
/// name; on a write/flush failure, removes the temp it created and
/// propagates the error.
fn write_unique_temp(path: &std::path::Path, content: &str) -> Result<PathBuf> {
    for _ in 0..TEMP_CREATE_ATTEMPTS {
        let tmp = candidate_temp_path(path);
        let mut file = match create_new_private(&tmp) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        };
        if let Err(e) = file
            .write_all(content.as_bytes())
            .and_then(|()| file.sync_all())
        {
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
        }
        return Ok(tmp);
    }
    Err(BzrError::config(
        "could not create a unique config temp file after repeated attempts",
    ))
}

#[cfg(unix)]
fn create_new_private(tmp: &std::path::Path) -> std::io::Result<fs::File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(tmp)
}

#[cfg(not(unix))]
fn create_new_private(tmp: &std::path::Path) -> std::io::Result<fs::File> {
    use std::fs::OpenOptions;

    OpenOptions::new().create_new(true).write(true).open(tmp)
}

#[cfg(unix)]
fn fsync_parent_dir(path: &std::path::Path) {
    if let Some(dir) = path.parent() {
        if let Ok(handle) = fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &std::path::Path) {}

/// How old a temp sibling must be before the reaper treats it as a
/// crash orphan. Comfortably longer than any single atomic write, so a
/// *live* temp belonging to a concurrent `bzr` process is never reaped.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(3600);

/// Remove crash-orphaned `config.toml.*.tmp` siblings **older than
/// [`STALE_TEMP_AGE`]**. A crash between temp-create and rename leaves a
/// unique-named orphan that no graceful cleanup reaps; sweep old ones so
/// they do not accumulate. The age gate is essential: CONC-1 ships before
/// the CONC-2 lock, so two concurrent processes can each have a fresh
/// in-flight temp — reaping unconditionally would delete the other's live
/// temp and make its `rename` fail (lost write). Only temps untouched for
/// an hour — which no live write produces — are removed.
fn reap_stale_temps(dir: &std::path::Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with("config.toml.") && name.ends_with(".tmp")) {
            continue;
        }
        let is_old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| mtime.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if is_old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn warn_if_path_permissions_too_open(path: &std::path::Path, mask: u32, kind: &str) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let mode = metadata.permissions().mode();
    if mode & mask == 0 {
        return;
    }

    warn_security(&format!(
        "{kind} '{}' has overly broad permissions ({:o}); expected owner-only access. Fix with `chmod {}` '{}'",
        path.display(),
        mode & 0o777,
        if kind == "config directory" { "700" } else { "600" },
        path.display()
    ));
}

#[expect(clippy::print_stderr)]
fn warn_security(message: &str) {
    eprintln!("warning: {message}");
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
