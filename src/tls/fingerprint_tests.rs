#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn compute_fingerprint_deterministic() {
    let input = b"example certificate bytes";
    let fp1 = compute_fingerprint(input);
    let fp2 = compute_fingerprint(input);
    assert_eq!(fp1, fp2);
    assert!(
        fp1.starts_with("sha256//"),
        "fingerprint must start with sha256//"
    );
}

#[test]
fn compute_fingerprint_format() {
    let fp = compute_fingerprint(b"test data");
    assert!(
        fp.starts_with("sha256//"),
        "fingerprint must start with sha256//"
    );
    let b64_part = fp.strip_prefix("sha256//").unwrap();
    let decoded = BASE64_STANDARD.decode(b64_part);
    assert!(
        decoded.is_ok(),
        "base64 portion must decode successfully: {decoded:?}"
    );
}
