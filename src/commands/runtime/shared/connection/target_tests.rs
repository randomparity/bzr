#[test]
fn extract_hostname_parses_url() {
    assert_eq!(
        super::extract_hostname("https://example.com/path"),
        "example.com"
    );
}

#[test]
fn extract_hostname_with_port() {
    assert_eq!(
        super::extract_hostname("https://example.com:8443/path"),
        "example.com"
    );
}

#[test]
fn extract_hostname_returns_raw_on_invalid() {
    assert_eq!(super::extract_hostname("not-a-url"), "not-a-url");
}
