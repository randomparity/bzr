use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::runtime::invocation::CommandContext;
use crate::config::{Config, ServerConfig};
use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;

type Sections = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Serialize)]
struct ImportResult<'a> {
    imported: usize,
    unsupported_password_credentials: usize,
    unsupported_certificates: usize,
    config_file: &'a str,
}

pub(super) fn handle(path: Option<&Path>, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let imported = resolved_servers(&read_sections(path)?)?;
    if imported.is_empty() {
        return Err(BzrError::input(
            "bugzillarc defines no URL to import".into(),
        ));
    }
    let unsupported_password_credentials = imported
        .iter()
        .filter(|server| server.has_password_credentials)
        .count();
    let unsupported_certificates = imported
        .iter()
        .filter(|server| server.cert.is_some())
        .count();
    Config::update_locked_at(ctx.config_path_override(), |config| {
        for imported_server in &imported {
            let name = server_name(config, &imported_server.url);
            let server = config
                .servers
                .entry(name.clone())
                .or_insert_with(|| ServerConfig {
                    url: imported_server.url.clone(),
                    ..ServerConfig::default()
                });
            imported_server.apply_to(server);
            if config.default_server.is_none() {
                config.default_server = Some(name);
            }
        }
        Ok(())
    })?;
    let config_path = Config::path_at(ctx.config_path_override())?;
    let config_file = config_path.to_string_lossy();
    let mut human = format!(
        "Imported {} server(s) from bugzillarc.\nConfig file: {config_file}",
        imported.len()
    );
    if unsupported_password_credentials != 0 {
        human.push_str("\nUsername/password credentials were not imported (unsupported).");
    }
    if unsupported_certificates != 0 {
        human.push_str("\nClient certificate settings were not imported (unsupported).");
    }
    write_result(
        &ImportResult {
            imported: imported.len(),
            unsupported_password_credentials,
            unsupported_certificates,
            config_file: &config_file,
        },
        &human,
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[derive(Debug)]
struct ImportedServer {
    url: String,
    api_key: Option<String>,
    has_password_credentials: bool,
    cert: Option<String>,
}

impl ImportedServer {
    fn apply_to(&self, server: &mut ServerConfig) {
        server.url.clone_from(&self.url);
        if let Some(api_key) = &self.api_key {
            server.api_key = Some(api_key.clone());
            server.api_key_env = None;
            server.api_key_keyring = None;
            server.token = None;
        }
    }
}

fn read_sections(path: Option<&Path>) -> Result<Sections> {
    let paths = path.map_or_else(default_paths, |file| vec![file.to_path_buf()]);
    let mut all = Sections::new();
    for file in paths {
        let contents = match fs::read_to_string(&file) {
            Ok(contents) => contents,
            Err(error) if path.is_none() && error.kind() == std::io::ErrorKind::NotFound => {
                continue
            }
            Err(error) => {
                return Err(BzrError::config(format!(
                    "read bugzillarc '{}': {error}",
                    file.display()
                )))
            }
        };
        merge_sections(&mut all, parse_sections(&contents, &file)?);
    }
    Ok(all)
}

fn default_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/etc/bugzillarc")];
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".bugzillarc"));
        paths.push(home.join(".config/python-bugzilla/bugzillarc"));
    }
    paths
}

fn parse_sections(contents: &str, path: &Path) -> Result<Sections> {
    let mut sections = Sections::new();
    let mut current = "DEFAULT".to_owned();
    sections.entry(current.clone()).or_default();
    for (line_no, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            if name.trim().is_empty() {
                return Err(parse_error(path, line_no));
            }
            name.trim().clone_into(&mut current);
            sections.entry(current.clone()).or_default();
            continue;
        }
        let Some((key, value)) = line.split_once(['=', ':']) else {
            return Err(parse_error(path, line_no));
        };
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(parse_error(path, line_no));
        }
        sections
            .entry(current.clone())
            .or_default()
            .insert(key, value.trim().to_owned());
    }
    Ok(sections)
}

fn parse_error(path: &Path, line_no: usize) -> BzrError {
    BzrError::config(format!(
        "parse bugzillarc '{}': invalid INI syntax at line {}",
        path.display(),
        line_no + 1
    ))
}

fn merge_sections(target: &mut Sections, source: Sections) {
    for (section, values) in source {
        target.entry(section).or_default().extend(values);
    }
}

fn resolved_servers(sections: &Sections) -> Result<Vec<ImportedServer>> {
    let defaults = sections.get("DEFAULT").cloned().unwrap_or_default();
    let Some(url) = defaults.get("url").filter(|url| !url.is_empty()).cloned() else {
        return Ok(Vec::new());
    };
    url::Url::parse(&url)
        .map_err(|error| BzrError::input(format!("bugzillarc DEFAULT url is invalid: {error}")))?;
    let authority = raw_authority(&url);
    let mut values = defaults;
    for (section, override_values) in sections {
        let matches_url = if section.contains('/') {
            url.contains(section)
        } else {
            section == authority
        };
        if section != "DEFAULT" && matches_url {
            values.extend(override_values.clone());
            break;
        }
    }
    Ok(vec![ImportedServer {
        url,
        api_key: values
            .get("api_key")
            .filter(|value| !value.is_empty())
            .cloned(),
        has_password_credentials: values.get("user").is_some_and(|value| !value.is_empty())
            || values
                .get("password")
                .is_some_and(|value| !value.is_empty()),
        cert: values
            .get("cert")
            .filter(|value| !value.is_empty())
            .cloned(),
    }])
}

fn raw_authority(url: &str) -> &str {
    url.split_once(':')
        .and_then(|(_, remainder)| remainder.strip_prefix("//"))
        .map_or("", |remainder| {
            remainder.split(['/', '?', '#']).next().unwrap_or_default()
        })
}

fn server_name(config: &Config, url: &str) -> String {
    if let Some((name, _)) = config
        .servers
        .iter()
        .filter(|(_, server)| server.url == url)
        .min_by_key(|(name, _)| *name)
    {
        return name.clone();
    }
    let host = url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_else(|| "imported".into());
    let base: String = host
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    if !config.servers.contains_key(&base) {
        return base;
    }
    (2..=u32::MAX)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|name| !config.servers.contains_key(name))
        .unwrap_or(base)
}

#[cfg(test)]
#[path = "import_bugzillarc_tests.rs"]
mod tests;
