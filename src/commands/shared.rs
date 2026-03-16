use crate::client::{BugzillaClient, FlagUpdate};
use crate::config::Config;
use crate::error::{BzrError, Result};

pub async fn build_client(server: Option<&str>) -> Result<BugzillaClient> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.active_server_named(server)?;
    let (server_name, url, api_key) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
    );
    let auth = crate::auth::resolve_auth_method(&mut config, &server_name).await?;
    BugzillaClient::new(&url, &api_key, auth)
}

/// Parse flag strings like "review?(user@example.com)" or "review+" or "review-"
/// into `FlagUpdate` structs.
///
/// Syntax: `name[+-?](requestee)`
///   - `name` is the flag type name
///   - `[+-?]` is the status character
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

fn parse_single_flag(s: &str) -> Result<(String, String, Option<String>)> {
    // Find the status character (+, -, ?)
    let status_pos = s.find(['+', '-', '?']).ok_or_else(|| {
        BzrError::Other(format!(
            "invalid flag '{s}': must contain +, -, or ? (e.g. 'review?')"
        ))
    })?;

    let name = s[..status_pos].to_string();
    if name.is_empty() {
        return Err(BzrError::Other(format!(
            "invalid flag '{s}': flag name cannot be empty"
        )));
    }

    let status_char = &s[status_pos..=status_pos];
    let remainder = &s[status_pos + 1..];

    let requestee = if remainder.starts_with('(') && remainder.ends_with(')') {
        Some(remainder[1..remainder.len() - 1].to_string())
    } else if remainder.is_empty() {
        None
    } else {
        return Err(BzrError::Other(format!(
            "invalid flag '{s}': requestee must be in parentheses"
        )));
    };

    Ok((name, status_char.to_string(), requestee))
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
        assert_eq!(flags[0].status, "?");
        assert_eq!(flags[0].requestee.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn parse_flag_grant() {
        let flags = parse_flags(&["review+".into()]).unwrap();
        assert_eq!(flags[0].name, "review");
        assert_eq!(flags[0].status, "+");
        assert!(flags[0].requestee.is_none());
    }

    #[test]
    fn parse_flag_deny() {
        let flags = parse_flags(&["review-".into()]).unwrap();
        assert_eq!(flags[0].status, "-");
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
    fn parse_multiple_flags() {
        let flags = parse_flags(&["review+".into(), "approval?".into()]).unwrap();
        assert_eq!(flags.len(), 2);
    }

    #[test]
    fn parse_empty_flags() {
        let flags = parse_flags(&[]).unwrap();
        assert!(flags.is_empty());
    }
}
