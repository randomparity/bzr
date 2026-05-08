use crate::error::{format_error_chain, BzrError};

#[derive(Debug)]
pub(crate) enum TlsPinFailure {
    PinMismatch { error: BzrError, new_issuer: String },
    IssuerChanged(BzrError),
}

pub(crate) fn classify(err: &BzrError, server_name: &str) -> Option<TlsPinFailure> {
    let BzrError::Http(reqwest_error) = err else {
        return None;
    };
    classify_chain(&format_error_chain(reqwest_error), server_name)
}

fn classify_chain(chain: &str, server_name: &str) -> Option<TlsPinFailure> {
    if let Some((expected, actual, issuer)) = parse_pin_mismatch_details(chain) {
        return Some(TlsPinFailure::PinMismatch {
            error: BzrError::PinMismatch {
                server: server_name.to_string(),
                expected,
                actual,
            },
            new_issuer: issuer,
        });
    }
    if let Some((expected_issuer, actual_issuer)) = parse_issuer_changed_details(chain) {
        return Some(TlsPinFailure::IssuerChanged(BzrError::IssuerChanged {
            server: server_name.to_string(),
            expected_issuer,
            actual_issuer,
        }));
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
    if rest.contains("issuer DER mismatch") {
        return Some((
            "pinned issuer DER".to_string(),
            "presented issuer DER".to_string(),
        ));
    }
    let expected_start = rest.find("expected \"")? + "expected \"".len();
    let after_expected = &rest[expected_start..];
    let expected_end = after_expected.find('"')?;
    let expected = after_expected[..expected_end].to_string();
    let got_start = after_expected[expected_end..].find("got \"")? + expected_end + "got \"".len();
    let after_got = &after_expected[got_start..];
    let got_end = after_got.find('"')?;
    let actual = after_got[..got_end].to_string();
    Some((expected, actual))
}

#[cfg(test)]
#[path = "pin_failure_tests.rs"]
mod tests;
