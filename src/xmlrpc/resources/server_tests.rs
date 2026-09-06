#![expect(clippy::unwrap_used, clippy::expect_used)]

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::xmlrpc::protocol::XmlRpcClient;

fn client(base_url: &str) -> XmlRpcClient {
    XmlRpcClient::new(reqwest::Client::new(), base_url, None)
}

/// Mount one `POST /xmlrpc.cgi` response and return the client pointed at it.
async fn mount(mock: &MockServer, body: &str) -> XmlRpcClient {
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(mock)
        .await;
    client(&mock.uri())
}

const ADVERTISED: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct>",
    r"<member><name>RedHat</name><value><struct>",
    r"<member><name>version</name><value><string>1.0</string></value></member>",
    r"</struct></value></member>",
    r"<member><name>Voting</name><value><struct>",
    r"<member><name>version</name><value><string>2.5</string></value></member>",
    r"</struct></value></member>",
    r"</struct></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

/// The shape bz50, bz52 and bz53 actually return: `extensions` present, empty.
const EMPTY_STRUCT: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct /></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

const MISSING_MEMBER: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>other</name><value><string>x</string></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

const NON_STRUCT_EXTENSIONS_MEMBER: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><string>yes</string></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

const NON_STRUCT_EXTENSION_VALUE: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct>",
    r"<member><name>RedHat</name><value><int>5</int></value></member>",
    r"</struct></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

const NON_STRING_VERSION: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param><value><struct>"#,
    r"<member><name>extensions</name><value><struct>",
    r"<member><name>RedHat</name><value><struct>",
    r"<member><name>version</name><value><int>5</int></value></member>",
    r"</struct></value></member>",
    r"</struct></value></member>",
    r"</struct></value></param></params></methodResponse>",
);

const NON_STRUCT_RESPONSE: &str = concat!(
    r#"<?xml version="1.0"?><methodResponse><params><param>"#,
    r"<value><string>nope</string></value>",
    r"</param></params></methodResponse>",
);

#[tokio::test]
async fn server_extensions_calls_bugzilla_extensions() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains(
            "<methodName>Bugzilla.extensions</methodName>",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_STRUCT))
        .expect(1)
        .mount(&mock)
        .await;

    client(&mock.uri()).server_extensions().await.unwrap();
}

#[tokio::test]
async fn server_extensions_parses_advertised_extensions() {
    let mock = MockServer::start().await;
    let advertised = mount(&mock, ADVERTISED).await.server_extensions().await;

    let extensions = advertised.unwrap().extensions;
    assert_eq!(extensions.len(), 2);
    assert_eq!(
        extensions.get("RedHat").and_then(|e| e.version.as_deref()),
        Some("1.0")
    );
    assert_eq!(
        extensions.get("Voting").and_then(|e| e.version.as_deref()),
        Some("2.5")
    );
}

/// All three supported images return `<struct />` here, so this is the shape
/// the negative verdict is actually built on.
#[tokio::test]
async fn server_extensions_parses_empty_extension_struct() {
    let mock = MockServer::start().await;
    let parsed = mount(&mock, EMPTY_STRUCT).await.server_extensions().await;

    assert!(parsed.unwrap().extensions.is_empty());
}

/// An absent member must not become an empty map: an empty map renders as a
/// settled *absent*, where the REST path's serde decode fails and renders
/// *undetermined* (ADR-0052, amended 2026-09-06).
#[tokio::test]
async fn server_extensions_missing_member_is_an_error() {
    let mock = MockServer::start().await;
    let err = mount(&mock, MISSING_MEMBER)
        .await
        .server_extensions()
        .await
        .expect_err("a missing extensions member must not become an empty map");

    assert!(err.to_string().contains("extensions"), "{err}");
}

#[tokio::test]
async fn server_extensions_non_struct_extensions_member_is_an_error() {
    let mock = MockServer::start().await;
    mount(&mock, NON_STRUCT_EXTENSIONS_MEMBER)
        .await
        .server_extensions()
        .await
        .expect_err("a non-struct extensions member must not become an empty map");
}

/// Keeping the name with `version: None` would render as *advertised* — the
/// permissive direction — where serde refuses the whole decode.
#[tokio::test]
async fn server_extensions_non_struct_extension_value_is_an_error() {
    let mock = MockServer::start().await;
    let err = mount(&mock, NON_STRUCT_EXTENSION_VALUE)
        .await
        .server_extensions()
        .await
        .expect_err("a non-struct extension value must not be absorbed into version: None");

    assert!(err.to_string().contains("RedHat"), "{err}");
}

/// `get_str` would yield `None` here; `Option<String>` refuses on the REST side.
#[tokio::test]
async fn server_extensions_non_string_version_is_an_error() {
    let mock = MockServer::start().await;
    let err = mount(&mock, NON_STRING_VERSION)
        .await
        .server_extensions()
        .await
        .expect_err("a non-string version must not be read leniently");

    assert!(err.to_string().contains("version"), "{err}");
}

#[tokio::test]
async fn server_extensions_non_struct_response_is_an_error() {
    let mock = MockServer::start().await;
    mount(&mock, NON_STRUCT_RESPONSE)
        .await
        .server_extensions()
        .await
        .expect_err("a non-struct top-level response must be rejected");
}
