use super::INLINE_SERVER_NAME;

#[test]
fn inline_name_is_parenthesized() {
    // The synthetic name must not be a legal TOML table key, so it can never
    // shadow a real configured server.
    assert!(INLINE_SERVER_NAME.starts_with('('));
}
