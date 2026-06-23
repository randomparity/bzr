use crate::error::{format_error_chain, BzrError};

#[derive(Debug)]
pub(crate) enum TlsPinFailure {
    PinMismatch {
        expected: String,
        actual: String,
        new_issuer: String,
    },
    IssuerChanged {
        expected_issuer: String,
        actual_issuer: String,
    },
}

pub(crate) fn classify(err: &BzrError) -> Option<TlsPinFailure> {
    let BzrError::Http(reqwest_error) = err else {
        return None;
    };
    classify_chain(&format_error_chain(reqwest_error))
}

fn classify_chain(chain: &str) -> Option<TlsPinFailure> {
    if let Some((expected, actual, issuer)) = parse_pin_mismatch_details(chain) {
        return Some(TlsPinFailure::PinMismatch {
            expected,
            actual,
            new_issuer: issuer,
        });
    }
    if let Some((expected_issuer, actual_issuer)) = parse_issuer_changed_details(chain) {
        return Some(TlsPinFailure::IssuerChanged {
            expected_issuer,
            actual_issuer,
        });
    }
    None
}

fn parse_pin_mismatch_details(chain: &str) -> Option<(String, String, String)> {
    let rest = chain.get(chain.find("PIN_MISMATCH")?..)?;
    let expected_start = rest.find("expected ")? + "expected ".len();
    let after_expected = &rest[expected_start..];
    let got_pos = after_expected.find(", got ")?;
    let expected = after_expected[..got_pos].to_string();
    let after_got = &after_expected[got_pos + ", got ".len()..];
    let issuer_pos = after_got.find(", issuer ")?;
    let actual = after_got[..issuer_pos].to_string();
    let issuer = after_got[issuer_pos + ", issuer ".len()..].to_string();
    Some((expected, actual, issuer))
}

fn parse_issuer_changed_details(chain: &str) -> Option<(String, String)> {
    let rest = chain.get(chain.find("ISSUER_CHANGED")?..)?;
    rest.contains("issuer DER mismatch").then(|| {
        (
            "pinned issuer DER".to_string(),
            "presented issuer DER".to_string(),
        )
    })
}

#[cfg(test)]
#[path = "pin_failure_tests.rs"]
mod tests;
