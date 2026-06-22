use super::{get, set, InlineServer, InlineTlsOptions, INLINE_SERVER_NAME};
use crate::ENV_LOCK;

#[tokio::test]
async fn set_then_get_round_trips() {
    let _lock = ENV_LOCK.lock().await;
    set(None);
    assert_eq!(get(), None);

    let inline = InlineServer {
        url: "https://bugzilla.example.com".into(),
        api_key_env: Some("BZR_KEY".into()),
        email: Some("dev@example.com".into()),
        tls: InlineTlsOptions::default(),
    };
    set(Some(inline.clone()));
    assert_eq!(get(), Some(inline));

    set(None);
    assert_eq!(get(), None, "clearing restores the no-inline state");
}

#[tokio::test]
async fn set_then_get_round_trips_without_api_key_env() {
    let _lock = ENV_LOCK.lock().await;
    set(None);

    let inline = InlineServer {
        url: "https://bugzilla.example.com".into(),
        api_key_env: None,
        email: None,
        tls: InlineTlsOptions::default(),
    };
    set(Some(inline.clone()));
    assert_eq!(get(), Some(inline));

    set(None);
}

#[test]
fn inline_name_is_parenthesized() {
    // The synthetic name must not be a legal TOML table key, so it can never
    // shadow a real configured server.
    assert!(INLINE_SERVER_NAME.starts_with('('));
}
