use crate::error::{BzrError, Result};
use crate::types::flag::{FlagStatus, FlagUpdate};

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
    // The status is the final character, or immediately before a requestee.
    let status_pos = status_position(s).ok_or_else(|| {
        BzrError::input(format!(
            "invalid flag '{s}': must contain +, -, ?, or X (e.g. 'review?')"
        ))
    })?;

    let name = s[..status_pos].to_string();
    if name.is_empty() {
        return Err(BzrError::input(format!(
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
        return Err(BzrError::input(format!(
            "invalid flag '{s}': requestee must be in parentheses"
        )));
    };

    Ok((name, status, requestee))
}

fn status_position(s: &str) -> Option<usize> {
    let candidate = if s.ends_with(')') {
        s.rfind('(')
            .and_then(|open_pos| open_pos.checked_sub(1))
            .filter(|&pos| is_status_byte(s.as_bytes().get(pos).copied()))
    } else {
        s.char_indices()
            .next_back()
            .map(|(pos, _)| pos)
            .filter(|&pos| is_status_byte(s.as_bytes().get(pos).copied()))
    };

    candidate.or_else(|| s.rfind(['+', '-', '?', 'X']))
}

fn is_status_byte(byte: Option<u8>) -> bool {
    matches!(byte, Some(b'+' | b'-' | b'?' | b'X'))
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod tests;
