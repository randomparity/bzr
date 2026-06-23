use crate::error::{BzrError, Result};
use crate::types::common::{FlagStatus, FlagUpdate};

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
#[path = "flags_tests.rs"]
mod tests;
