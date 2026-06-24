#![expect(clippy::unwrap_used)]

use crate::error::BzrError;
use crate::xmlrpc::protocol::XmlRpcClient;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Wrap an inner array of `<value><struct>...</struct></value>` items in
/// the standard `bugs -> {bug_id} -> array` XML-RPC response envelope.
fn xmlrpc_bugs_envelope(bug_id: u64, inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>bugs</name><value><struct>\
                <member><name>{bug_id}</name><value><array><data>{inner}</data></array></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

/// Wrap inner `<member>...</member>` entries in the
/// `attachments -> {attachment_id}` XML-RPC response envelope used by
/// `Bug.attachments` when called with `attachment_ids`.
fn xmlrpc_attachments_keyed_envelope(inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>attachments</name><value><struct>{inner}</struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

/// Custom wiremock matcher: body must NOT contain the given substring.
struct NotBodyContains(&'static str);

impl wiremock::Match for NotBodyContains {
    fn matches(&self, request: &wiremock::Request) -> bool {
        !std::str::from_utf8(&request.body).is_ok_and(|s| s.contains(self.0))
    }
}

#[tokio::test]
async fn xmlrpc_get_attachments_parses_full_response() {
    let mock = MockServer::start().await;
    let inner = "\
        <value><struct>\
            <member><name>id</name><value><int>2001</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>public.txt</string></value></member>\
            <member><name>summary</name><value><string>public file</string></value></member>\
            <member><name>content_type</name><value><string>text/plain</string></value></member>\
            <member><name>creator</name><value><string>alice@test</string></value></member>\
            <member><name>creation_time</name><value><dateTime.iso8601>20260101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>last_change_time</name><value><dateTime.iso8601>20260101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>size</name><value><int>11</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>0</int></value></member>\
            <member><name>data</name><value><base64>aGVsbG8gd29ybGQK</base64></value></member>\
        </struct></value>\
        <value><struct>\
            <member><name>id</name><value><int>2002</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>private.bin</string></value></member>\
            <member><name>summary</name><value><string>private file</string></value></member>\
            <member><name>content_type</name><value><string>application/octet-stream</string></value></member>\
            <member><name>creator</name><value><string>bob@test</string></value></member>\
            <member><name>creation_time</name><value><dateTime.iso8601>20260102T00:00:00</dateTime.iso8601></value></member>\
            <member><name>last_change_time</name><value><dateTime.iso8601>20260102T00:00:00</dateTime.iso8601></value></member>\
            <member><name>size</name><value><int>4</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>1</int></value></member>\
            <member><name>data</name><value><base64>YmVlZg==</base64></value></member>\
        </struct></value>";
    let response_xml = xmlrpc_bugs_envelope(42, inner);

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let attachments = client.get_attachments(42).await.unwrap();

    assert_eq!(attachments.len(), 2);
    assert_eq!(attachments[0].id, 2001);
    assert_eq!(attachments[0].file_name, "public.txt");
    assert!(!attachments[0].is_private);
    assert_eq!(attachments[0].size, 11);
    assert_eq!(attachments[0].data.as_deref(), Some("aGVsbG8gd29ybGQK"));
    assert_eq!(attachments[1].id, 2002);
    assert_eq!(attachments[1].file_name, "private.bin");
    assert!(attachments[1].is_private);
    assert_eq!(attachments[1].data.as_deref(), Some("YmVlZg=="));
}

#[tokio::test]
async fn xmlrpc_get_attachments_requests_inline_data_field() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_bugs_envelope(42, "");

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .and(body_string_contains("include_fields"))
        .and(body_string_contains("<string>id</string>"))
        .and(body_string_contains("<string>bug_id</string>"))
        .and(body_string_contains("<string>file_name</string>"))
        .and(body_string_contains("<string>data</string>"))
        .and(NotBodyContains("exclude_fields"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let _ = client.get_attachments(42).await.unwrap();
}

#[tokio::test]
async fn xmlrpc_get_attachments_rejects_bug_id_outside_xmlrpc_integer_range() {
    let client = XmlRpcClient::new(test_http_client(), "http://127.0.0.1:1", None);

    let err = client
        .get_attachments(u64::try_from(i64::MAX).unwrap() + 1)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        BzrError::InputValidation(ref msg)
            if msg.contains("bug ID") && msg.contains("XML-RPC signed integer range")
    ));
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_request_body_omits_exclude_fields() {
    use wiremock::matchers::body_string_contains;
    let mock = MockServer::start().await;

    // If the request carried exclude_fields, the download path would
    // get data:None and fail. Mock will only match a request that
    // does NOT contain "exclude_fields" via a custom matcher.
    let response_xml = xmlrpc_attachments_keyed_envelope(
        "<member><name>9</name><value><struct>\
            <member><name>id</name><value><int>9</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>y.bin</string></value></member>\
            <member><name>summary</name><value><string>y</string></value></member>\
            <member><name>content_type</name><value><string>application/octet-stream</string></value></member>\
            <member><name>size</name><value><int>2</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>0</int></value></member>\
            <member><name>data</name><value><base64>YmU=</base64></value></member>\
        </struct></value></member>",
    );
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("attachment_ids"))
        .and(NotBodyContains("exclude_fields"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .expect(1)
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let attachment = client.get_attachment_by_id(9).await.unwrap();
    assert_eq!(attachment.data.as_deref(), Some("YmU="));
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_parses_response() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_attachments_keyed_envelope(
        "<member><name>2002</name><value><struct>\
            <member><name>id</name><value><int>2002</int></value></member>\
            <member><name>bug_id</name><value><int>42</int></value></member>\
            <member><name>file_name</name><value><string>private.bin</string></value></member>\
            <member><name>summary</name><value><string>private file</string></value></member>\
            <member><name>content_type</name><value><string>application/octet-stream</string></value></member>\
            <member><name>size</name><value><int>4</int></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>1</int></value></member>\
            <member><name>data</name><value><base64>YmVlZg==</base64></value></member>\
        </struct></value></member>",
    );
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .and(body_string_contains("attachment_ids"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let attachment = client.get_attachment_by_id(2002).await.unwrap();
    assert_eq!(attachment.id, 2002);
    assert!(attachment.is_private);
    assert_eq!(attachment.data.as_deref(), Some("YmVlZg=="));
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_rejects_id_outside_xmlrpc_integer_range() {
    let client = XmlRpcClient::new(test_http_client(), "http://127.0.0.1:1", None);

    let err = client
        .get_attachment_by_id(u64::try_from(i64::MAX).unwrap() + 1)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        BzrError::InputValidation(ref msg)
            if msg.contains("attachment ID") && msg.contains("XML-RPC signed integer range")
    ));
}

#[tokio::test]
async fn xmlrpc_get_attachment_by_id_not_found_returns_error() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_attachments_keyed_envelope("");
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let err = client.get_attachment_by_id(9999).await.unwrap_err();
    assert!(matches!(
        err,
        BzrError::NotFound {
            resource: "attachment",
            ..
        }
    ));
}

#[tokio::test]
async fn xmlrpc_attachment_nonempty_string_data_is_kept() {
    // Some servers return attachment payload as <string> rather than <base64>.
    // A non-empty string must be preserved verbatim.
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_attachments_keyed_envelope(
        "<member><name>7</name><value><struct>\
            <member><name>id</name><value><int>7</int></value></member>\
            <member><name>data</name><value><string>plain-text-data</string></value></member>\
        </struct></value></member>",
    );
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("attachment_ids"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let attachment = client.get_attachment_by_id(7).await.unwrap();
    assert_eq!(attachment.data.as_deref(), Some("plain-text-data"));
}

#[tokio::test]
async fn xmlrpc_attachment_empty_string_data_becomes_none() {
    // An empty <string> payload is treated as absent (None), not Some(""), so
    // the empty-string guard must reject it.
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_attachments_keyed_envelope(
        "<member><name>8</name><value><struct>\
            <member><name>id</name><value><int>8</int></value></member>\
            <member><name>data</name><value><string></string></value></member>\
        </struct></value></member>",
    );
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("attachment_ids"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let attachment = client.get_attachment_by_id(8).await.unwrap();
    assert_eq!(attachment.data, None);
}

#[tokio::test]
async fn xmlrpc_get_attachments_returns_empty_when_bug_has_none() {
    let mock = MockServer::start().await;
    let response_xml = xmlrpc_bugs_envelope(42, "");
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .and(body_string_contains("Bug.attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_xml))
        .mount(&mock)
        .await;

    let client = XmlRpcClient::new(test_http_client(), &mock.uri(), Some("test-key"));
    let attachments = client.get_attachments(42).await.unwrap();
    assert!(attachments.is_empty());
}
