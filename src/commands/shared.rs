use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::types::ApiMode;
use crate::types::{FlagStatus, FlagUpdate};

/// Connect to a Bugzilla server.
pub async fn connect_client(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let (client, _email) = connect_client_with_email(server, api_override).await?;
    Ok(client)
}

/// Connect to a Bugzilla server, also returning the server's configured email
/// (if any). The email is needed by whoami for Bugzilla 5.0 fallback.
pub async fn connect_client_with_email(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<(BugzillaClient, Option<String>)> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let (server_name, url, api_key, email) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
        srv.email.clone(),
    );
    let (auth, detected_mode) =
        crate::auth::detect_and_persist_server_settings(&mut config, &server_name).await?;
    let api_mode = api_override.unwrap_or(detected_mode);
    let client = BugzillaClient::new(&url, &api_key, auth, api_mode)?;
    Ok((client, email))
}

/// Parse flag strings like "review?(user@example.com)" or "review+" or "review-"
/// into `FlagUpdate` structs.
///
/// Syntax: `name[+-?X](requestee)`
///   - `name` is the flag type name
///   - `[+-?X]` is the status character (`X` clears the flag)
///   - `(requestee)` is optional, only valid with `?`
pub fn parse_flags(raw: &[String]) -> Result<Vec<FlagUpdate>> {
    let mut flags = Vec::new();
    for s in raw {
        let (name, status, requestee) = parse_single_flag(s)?;
        flags.push(FlagUpdate {
            name,
            status,
            requestee,
        });
    }
    Ok(flags)
}

fn parse_single_flag(s: &str) -> Result<(String, FlagStatus, Option<String>)> {
    // Find the status character (+, -, ?, X)
    let status_pos = s.find(['+', '-', '?', 'X']).ok_or_else(|| {
        BzrError::InputValidation(format!(
            "invalid flag '{s}': must contain +, -, ?, or X (e.g. 'review?')"
        ))
    })?;

    let name = s[..status_pos].to_string();
    if name.is_empty() {
        return Err(BzrError::InputValidation(format!(
            "invalid flag '{s}': flag name cannot be empty"
        )));
    }

    let status = match s.as_bytes()[status_pos] {
        b'+' => FlagStatus::Grant,
        b'-' => FlagStatus::Deny,
        b'?' => FlagStatus::Request,
        b'X' => FlagStatus::Clear,
        _ => unreachable!("find() only matches +, -, ?, X"),
    };
    let remainder = &s[status_pos + 1..];

    let requestee = if remainder.starts_with('(') && remainder.ends_with(')') {
        Some(remainder[1..remainder.len() - 1].to_string())
    } else if remainder.is_empty() {
        None
    } else {
        return Err(BzrError::InputValidation(format!(
            "invalid flag '{s}': requestee must be in parentheses"
        )));
    };

    Ok((name, status, requestee))
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_flag_with_request() {
        let flags = parse_flags(&["review?(alice@example.com)".into()]).unwrap();
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].name, "review");
        assert_eq!(flags[0].status, FlagStatus::Request);
        assert_eq!(flags[0].requestee.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn parse_flag_grant() {
        let flags = parse_flags(&["review+".into()]).unwrap();
        assert_eq!(flags[0].name, "review");
        assert_eq!(flags[0].status, FlagStatus::Grant);
        assert!(flags[0].requestee.is_none());
    }

    #[test]
    fn parse_flag_deny() {
        let flags = parse_flags(&["review-".into()]).unwrap();
        assert_eq!(flags[0].status, FlagStatus::Deny);
    }

    #[test]
    fn parse_flag_no_status_char_fails() {
        let err = parse_flags(&["review".into()]).unwrap_err();
        assert!(err.to_string().contains("must contain"));
    }

    #[test]
    fn parse_flag_empty_name_fails() {
        let err = parse_flags(&["?".into()]).unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn parse_flag_bad_requestee_fails() {
        let err = parse_flags(&["review?alice".into()]).unwrap_err();
        assert!(err.to_string().contains("parentheses"));
    }

    #[test]
    fn parse_flag_clear() {
        let flags = parse_flags(&["reviewX".into()]).unwrap();
        assert_eq!(flags[0].name, "review");
        assert_eq!(flags[0].status, FlagStatus::Clear);
        assert!(flags[0].requestee.is_none());
    }

    #[test]
    fn parse_multiple_flags() {
        let flags = parse_flags(&["review+".into(), "approval?".into()]).unwrap();
        assert_eq!(flags.len(), 2);
        assert_eq!(flags[0].name, "review");
        assert_eq!(flags[0].status, FlagStatus::Grant);
        assert_eq!(flags[1].name, "approval");
        assert_eq!(flags[1].status, FlagStatus::Request);
    }

    #[test]
    fn parse_empty_flags() {
        let flags = parse_flags(&[]).unwrap();
        assert!(flags.is_empty());
    }
}
