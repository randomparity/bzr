#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::{test_client, test_client_hybrid, test_client_xmlrpc};
use crate::error::BzrError;
use crate::types::{FlagStatus, FlagUpdate, UpdateAttachmentParams, UploadAttachmentParams};

fn xmlrpc_fault_response(code: i64, message: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><fault><value><struct>\
            <member><name>faultCode</name><value><int>{code}</int></value></member>\
            <member><name>faultString</name><value><string>{message}</string></value></member>\
        </struct></value></fault></methodResponse>"
    )
}

fn rest_attachments_response_json(ids: &[u64]) -> serde_json::Value {
    let attachments: Vec<serde_json::Value> = ids
        .iter()
        .map(|&id| {
            serde_json::json!({
                "id": id,
                "bug_id": 42,
                "file_name": format!("rest-{id}.txt"),
                "summary": format!("rest {id}"),
                "content_type": "text/plain",
                "creator": "alice@test",
                "creation_time": "2026-01-01T00:00:00Z",
                "last_change_time": "2026-01-01T00:00:00Z",
                "size": 1,
                "is_obsolete": false,
                "is_private": false
            })
        })
        .collect();
    serde_json::json!({"bugs": {"42": attachments}})
}

fn xmlrpc_attachments_response(ids: &[u64]) -> String {
    use std::fmt::Write;
    let mut entries = String::new();
    for &id in ids {
        write!(
            entries,
            "<value><struct>\
                <member><name>id</name><value><int>{id}</int></value></member>\
                <member><name>bug_id</name><value><int>42</int></value></member>\
                <member><name>file_name</name><value><string>xmlrpc-{id}.txt</string></value></member>\
                <member><name>summary</name><value><string>xmlrpc {id}</string></value></member>\
                <member><name>content_type</name><value><string>text/plain</string></value></member>\
                <member><name>size</name><value><int>1</int></value></member>\
                <member><name>is_obsolete</name><value><int>0</int></value></member>\
                <member><name>is_private</name><value><int>0</int></value></member>\
            </struct></value>",
        )
        .unwrap();
    }
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>bugs</name><value><struct>\
                <member><name>42</name><value><array>\
<data>{entries}</data></array></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

#[tokio::test]
async fn hybrid_uses_xmlrpc_directly_for_attachment_list() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_attachments_response(&[1, 2, 3])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 3);
    assert_eq!(attachments[0].file_name, "xmlrpc-1.txt");
}

#[tokio::test]
async fn hybrid_xmlrpc_transport_error_falls_back_to_rest_for_attachment_list() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(502))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(rest_attachments_response_json(&[10, 11])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 2);
    assert_eq!(attachments[0].file_name, "rest-10.txt");
}

#[tokio::test]
async fn hybrid_xmlrpc_fault_does_not_fall_back_to_rest_for_attachment_list() {
    // An XML-RPC `<fault>` (e.g. permission denied) is a domain error, not a
    // transport failure: `is_transport_failure()` is false for `BzrError::Api`,
    // so Hybrid mode must propagate it instead of silently retrying via REST,
    // where the same call would return a permission-filtered (and therefore
    // wrong) result. Regression guard for issue #136.
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_fault_response(410, "You are not authorized")),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(rest_attachments_response_json(&[99])),
        )
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let err = client.get_attachments(42).await.unwrap_err();
    assert!(
        matches!(&err, BzrError::Api { code, .. } if *code == 410),
        "expected Api error with code 410, got {err:?}"
    );
}

#[tokio::test]
async fn rest_mode_uses_rest_only_for_attachment_list() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(rest_attachments_response_json(&[5])),
        )
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].id, 5);
}

fn xmlrpc_attachment_by_id_response(id: u64, is_private: bool) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>attachments</name><value><struct>\
                <member><name>{id}</name><value><struct>\
                    <member><name>id</name><value><int>{id}</int></value></member>\
                    <member><name>bug_id</name><value><int>42</int></value></member>\
                    <member><name>file_name</name><value><string>x-{id}.txt</string></value></member>\
                    <member><name>summary</name><value><string>x</string></value></member>\
                    <member><name>content_type</name><value><string>text/plain</string></value></member>\
                    <member><name>size</name><value><int>1</int></value></member>\
                    <member><name>is_obsolete</name><value><int>0</int></value></member>\
                    <member><name>is_private</name><value><int>{p}</int></value></member>\
                </struct></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>",
        p = u8::from(is_private),
    )
}

fn rest_attachment_by_id_response_json(id: u64) -> serde_json::Value {
    serde_json::json!({
        "attachments": {
            id.to_string(): {
                "id": id,
                "bug_id": 42,
                "file_name": format!("rest-{id}.txt"),
                "summary": "rest",
                "content_type": "text/plain",
                "creator": "alice@test",
                "creation_time": "2026-01-01T00:00:00Z",
                "last_change_time": "2026-01-01T00:00:00Z",
                "size": 1,
                "is_obsolete": false,
                "is_private": false,
                "data": "aGVsbG8="
            }
        }
    })
}

#[tokio::test]
async fn hybrid_uses_xmlrpc_directly_for_get_attachment() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/2001"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_attachment_by_id_response(2001, true)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let attachment = client.get_attachment(2001).await.unwrap();
    assert_eq!(attachment.id, 2001);
    assert!(attachment.is_private);
    assert_eq!(attachment.file_name, "x-2001.txt");
}

#[tokio::test]
async fn hybrid_xmlrpc_transport_error_falls_back_to_rest_for_get_attachment() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(502))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/2002"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(rest_attachment_by_id_response_json(2002)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let attachment = client.get_attachment(2002).await.unwrap();
    assert_eq!(attachment.id, 2002);
    assert_eq!(attachment.file_name, "rest-2002.txt");
}

#[tokio::test]
async fn hybrid_xmlrpc_fault_does_not_fall_back_to_rest_for_get_attachment() {
    // Mirror of the list-path regression: a server-side fault for `Bug.attachments`
    // by attachment id must propagate as `BzrError::Api`, not silently fall through
    // to REST where private content is filtered (issue #133/#136).
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_fault_response(410, "You are not authorized")),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/2002"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(rest_attachment_by_id_response_json(2002)),
        )
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let err = client.get_attachment(2002).await.unwrap_err();
    assert!(
        matches!(&err, BzrError::Api { code, .. } if *code == 410),
        "expected Api error with code 410, got {err:?}"
    );
}

#[tokio::test]
async fn rest_mode_uses_rest_only_for_get_attachment() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/2003"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(rest_attachment_by_id_response_json(2003)),
        )
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachment = client.get_attachment(2003).await.unwrap();
    assert_eq!(attachment.id, 2003);
}

#[tokio::test]
async fn xmlrpc_mode_skips_rest_for_get_attachment() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/2004"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_attachment_by_id_response(2004, false)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_xmlrpc(&mock.uri());
    let attachment = client.get_attachment(2004).await.unwrap();
    assert_eq!(attachment.id, 2004);
}

#[tokio::test]
async fn xmlrpc_mode_skips_rest_for_attachment_list() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_attachments_response(&[7, 8, 9])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_xmlrpc(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 3);
}

#[tokio::test]
async fn update_attachment_sends_put() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 100, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = UpdateAttachmentParams {
        is_obsolete: Some(true),
        summary: Some("Updated patch".into()),
        ..Default::default()
    };
    client.update_attachment(100, &params).await.unwrap();
}

#[tokio::test]
async fn upload_attachment_private_sets_is_private_in_body() {
    use wiremock::matchers::body_string_contains;
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .and(body_string_contains("\"is_private\":true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [301]})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let id = client
        .upload_attachment(&UploadAttachmentParams {
            bug_id: 1,
            file_name: "secret.bin".into(),
            summary: "secret".into(),
            content_type: "application/octet-stream".into(),
            data: b"shh".to_vec(),
            flags: Vec::new(),
            is_private: true,
        })
        .await
        .unwrap();
    assert_eq!(id, 301);
}

#[tokio::test]
async fn upload_attachment_public_omits_is_private_or_sets_false() {
    use wiremock::matchers::body_string_contains;
    let mock = MockServer::start().await;
    // Public uploads must not send is_private:true. They may either omit
    // the field or send is_private:false.
    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .and(body_string_contains("\"is_private\":false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [302]})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let id = client
        .upload_attachment(&UploadAttachmentParams {
            bug_id: 1,
            file_name: "public.txt".into(),
            summary: "public".into(),
            content_type: "text/plain".into(),
            data: b"hello".to_vec(),
            flags: Vec::new(),
            is_private: false,
        })
        .await
        .unwrap();
    assert_eq!(id, 302);
}

#[tokio::test]
async fn upload_attachment_with_flags_sends_flags() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let flags = vec![FlagUpdate {
        name: "review".into(),
        status: FlagStatus::Request,
        requestee: Some("alice@example.com".into()),
    }];
    let id = client
        .upload_attachment(&UploadAttachmentParams {
            bug_id: 1,
            file_name: "test.txt".into(),
            summary: "test".into(),
            content_type: "text/plain".into(),
            data: b"hello".to_vec(),
            flags,
            is_private: false,
        })
        .await
        .unwrap();
    assert_eq!(id, 200);
}

#[tokio::test]
async fn get_attachments_accepts_bugs_envelope() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": [
                    {
                        "id": 100,
                        "bug_id": 42,
                        "file_name": "patch.diff",
                        "summary": "test patch",
                        "content_type": "text/plain",
                        "creator": "alice@example.com",
                        "creation_time": "2026-01-01T00:00:00Z",
                        "last_change_time": "2026-01-01T00:00:00Z",
                        "size": 100,
                        "is_obsolete": false,
                        "is_patch": true,
                        "is_private": false
                    }
                ]
            },
            "attachments": {}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].id, 100);
    assert_eq!(attachments[0].file_name, "patch.diff");
}

#[tokio::test]
async fn get_attachments_accepts_flat_attachments_envelope() {
    // Some Bugzilla 5.0.x deployments (e.g. IBM LTC) return only an
    // `attachments` array, no `bugs` key. Issue #135.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [
                {
                    "id": 200,
                    "bug_id": 42,
                    "file_name": "alt.diff",
                    "summary": "alt-envelope patch",
                    "content_type": "text/plain",
                    "creator": "bob@example.com",
                    "creation_time": "2026-01-01T00:00:00Z",
                    "last_change_time": "2026-01-01T00:00:00Z",
                    "size": 50,
                    "is_obsolete": false,
                    "is_patch": true,
                    "is_private": false
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].id, 200);
    assert_eq!(attachments[0].file_name, "alt.diff");
}

#[tokio::test]
async fn get_attachments_prefers_bugs_when_both_envelopes_populated() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": [
                    {
                        "id": 100,
                        "bug_id": 42,
                        "file_name": "from_bugs.diff",
                        "summary": "from bugs",
                        "content_type": "text/plain",
                        "creator": "alice@example.com",
                        "creation_time": "2026-01-01T00:00:00Z",
                        "last_change_time": "2026-01-01T00:00:00Z",
                        "size": 1,
                        "is_obsolete": false,
                        "is_patch": true,
                        "is_private": false
                    }
                ]
            },
            "attachments": [
                {
                    "id": 200,
                    "bug_id": 42,
                    "file_name": "from_attachments.diff",
                    "summary": "from attachments",
                    "content_type": "text/plain",
                    "creator": "bob@example.com",
                    "creation_time": "2026-01-01T00:00:00Z",
                    "last_change_time": "2026-01-01T00:00:00Z",
                    "size": 1,
                    "is_obsolete": false,
                    "is_patch": true,
                    "is_private": false
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(
        attachments[0].file_name, "from_bugs.diff",
        "should prefer bugs-keyed envelope when both present"
    );
}

#[tokio::test]
async fn get_attachments_falls_through_when_bugs_map_empty_and_flat_populated() {
    // Regression: an empty top-level `bugs` map (no bug ID acknowledged)
    // alongside a populated flat `attachments` array used to silently
    // return [] because the bugs extractor swallowed the empty case.
    // The bugs extractor now returns Err on an empty top-level map so
    // try_envelopes falls through to the flat extractor.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {},
            "attachments": [
                {
                    "id": 300,
                    "bug_id": 42,
                    "file_name": "from_flat.diff",
                    "summary": "from flat envelope",
                    "content_type": "text/plain",
                    "creator": "alice@example.com",
                    "creation_time": "2026-01-01T00:00:00Z",
                    "last_change_time": "2026-01-01T00:00:00Z",
                    "size": 1,
                    "is_obsolete": false,
                    "is_patch": true,
                    "is_private": false
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert_eq!(
        attachments.len(),
        1,
        "should fall through to flat envelope, not return empty"
    );
    assert_eq!(attachments[0].file_name, "from_flat.diff");
}

#[tokio::test]
async fn get_attachments_returns_empty_when_bug_acknowledged_with_no_attachments() {
    // Legitimate empty case: server acknowledges bug 42 but reports no
    // attachments. Must return Ok([]) — NOT fall through to flat (where
    // the empty {} would fail to deserialize as Vec) and NOT error.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {"42": []},
            "attachments": {}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let attachments = client.get_attachments(42).await.unwrap();
    assert!(
        attachments.is_empty(),
        "no attachments expected: {attachments:?}"
    );
}
