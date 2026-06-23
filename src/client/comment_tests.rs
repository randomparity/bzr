#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::client::test_helpers::{test_client, test_client_hybrid, test_client_xmlrpc};

#[tokio::test]
async fn update_comment_tags_sends_put() {
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/42/tags"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!(["needinfo", "reviewed"])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = crate::types::UpdateCommentTagsParams {
        add: vec!["needinfo".into()],
        ..Default::default()
    };
    let tags = client.update_comment_tags(42, &params).await.unwrap();
    assert_eq!(tags, vec!["needinfo", "reviewed"]);
}

#[tokio::test]
async fn search_comment_tags_returns_matches() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/comment/tags/need"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!(["needinfo", "needreview"])),
        )
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let tags = client.search_comment_tags("need").await.unwrap();
    assert_eq!(tags, vec!["needinfo", "needreview"]);
}

#[tokio::test]
async fn get_comments_since_filters_by_date() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1/comment"))
        .and(query_param("new_since", "2025-01-01T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "1": {
                    "comments": [
                        {"id": 5, "bug_id": 1, "text": "new comment", "count": 3}
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since(1, Some("2025-01-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "new comment");
}

fn comments_response_json(counts: &[u64]) -> serde_json::Value {
    let comments: Vec<serde_json::Value> = counts
        .iter()
        .map(|&c| {
            serde_json::json!({
                "id": 1000 + c,
                "bug_id": 42,
                "count": c,
                "text": format!("comment {c}"),
                "creator": "alice@test",
                "creation_time": "2026-01-01T00:00:00Z",
                "is_private": c % 2 == 1
            })
        })
        .collect();
    serde_json::json!({"bugs": {"42": {"comments": comments}}})
}

fn xmlrpc_comments_response(counts: &[u64]) -> String {
    use std::fmt::Write;
    let mut entries = String::new();
    for &c in counts {
        write!(
            entries,
            "<value><struct>\
                <member><name>id</name><value><int>{id}</int></value></member>\
                <member><name>bug_id</name><value><int>42</int></value></member>\
                <member><name>count</name><value><int>{c}</int></value></member>\
                <member><name>text</name><value><string>xmlrpc {c}</string></value></member>\
                <member><name>is_private</name><value><boolean>{p}</boolean></value></member>\
            </struct></value>",
            id = 2000 + c,
            p = u8::from(c % 2 == 1),
        )
        .unwrap();
    }
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>bugs</name><value><struct>\
                <member><name>42</name><value><struct>\
                    <member><name>comments</name><value><array>\
<data>{entries}</data></array></value></member>\
                </struct></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

#[tokio::test]
async fn hybrid_uses_xmlrpc_directly() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_comments_response(&[0, 1, 2])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let comments = client.get_comments_since(42, None).await.unwrap();
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].text, "xmlrpc 0");
}

#[tokio::test]
async fn hybrid_xmlrpc_transport_error_falls_back_to_rest() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(502))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(comments_response_json(&[0, 1, 2])))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_hybrid(&mock.uri());
    let comments = client.get_comments_since(42, None).await.unwrap();
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].text, "comment 0");
}

#[tokio::test]
async fn rest_mode_uses_rest_only() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(comments_response_json(&[4])))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client.get_comments_since(42, None).await.unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].count, 4);
}

#[tokio::test]
async fn xmlrpc_mode_skips_rest() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(xmlrpc_comments_response(&[0, 1, 2])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client_xmlrpc(&mock.uri());
    let comments = client.get_comments_since(42, None).await.unwrap();
    assert_eq!(comments.len(), 3);
}

#[tokio::test]
async fn add_comment_private_sets_is_private_in_body() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .and(body_json(
            serde_json::json!({"comment": "secret", "is_private": true}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 999})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = crate::types::AddCommentParams {
        text: "secret".into(),
        is_private: true,
    };
    let id = client.add_comment(42, &params).await.unwrap();
    assert_eq!(id, 999);
}

#[tokio::test]
async fn add_comment_public_sets_is_private_false() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .and(body_json(
            serde_json::json!({"comment": "public", "is_private": false}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1000})))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = crate::types::AddCommentParams {
        text: "public".into(),
        is_private: false,
    };
    let id = client.add_comment(42, &params).await.unwrap();
    assert_eq!(id, 1000);
}

#[tokio::test]
async fn get_comments_since_rest_accepts_flat_comments_envelope() {
    // Some Bugzilla 5.0.x deployments return {"comments": [...]} at the
    // root instead of the stock {"bugs": {<id>: {"comments": [...]}}}.
    // Issue #135.
    use crate::client::test_helpers::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": 1,
                    "bug_id": 42,
                    "creator": "alice@example.com",
                    "time": "2026-01-01T00:00:00Z",
                    "creation_time": "2026-01-01T00:00:00Z",
                    "text": "hello from alt envelope",
                    "is_private": false,
                    "count": 0
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since_rest_for_test(42, None)
        .await
        .unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, 1);
    assert_eq!(comments[0].text, "hello from alt envelope");
}

#[tokio::test]
async fn get_comments_since_rest_accepts_bugs_envelope() {
    use crate::client::test_helpers::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {
                            "id": 1,
                            "bug_id": 42,
                            "creator": "alice@example.com",
                            "time": "2026-01-01T00:00:00Z",
                            "creation_time": "2026-01-01T00:00:00Z",
                            "text": "from bugs envelope",
                            "is_private": false,
                            "count": 0
                        }
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since_rest_for_test(42, None)
        .await
        .unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, 1);
    assert_eq!(comments[0].text, "from bugs envelope");
}

#[tokio::test]
async fn get_comments_since_rest_prefers_bugs_when_both_envelopes_populated() {
    use crate::client::test_helpers::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {
                            "id": 1,
                            "bug_id": 42,
                            "creator": "alice@example.com",
                            "time": "2026-01-01T00:00:00Z",
                            "creation_time": "2026-01-01T00:00:00Z",
                            "text": "from bugs envelope",
                            "is_private": false,
                            "count": 0
                        }
                    ]
                }
            },
            "comments": [
                {
                    "id": 2,
                    "bug_id": 42,
                    "creator": "bob@example.com",
                    "time": "2026-01-01T00:00:00Z",
                    "creation_time": "2026-01-01T00:00:00Z",
                    "text": "from flat envelope",
                    "is_private": false,
                    "count": 0
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since_rest_for_test(42, None)
        .await
        .unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(
        comments[0].text, "from bugs envelope",
        "should prefer bugs-keyed envelope when both present"
    );
}

#[tokio::test]
async fn get_comments_since_rest_falls_through_when_bugs_map_empty_and_flat_populated() {
    // Regression: an empty top-level `bugs` map (no bug ID acknowledged)
    // alongside a populated flat `comments` array used to silently return
    // [] because the bugs extractor swallowed the empty case. The bugs
    // extractor now returns Err on an empty top-level map so try_envelopes
    // falls through to the flat extractor.
    use crate::client::test_helpers::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {},
            "comments": [
                {
                    "id": 99,
                    "bug_id": 42,
                    "creator": "alice@example.com",
                    "time": "2026-01-01T00:00:00Z",
                    "creation_time": "2026-01-01T00:00:00Z",
                    "text": "from flat envelope",
                    "is_private": false,
                    "count": 0
                }
            ]
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since_rest_for_test(42, None)
        .await
        .unwrap();
    assert_eq!(
        comments.len(),
        1,
        "should fall through to flat envelope, not return empty"
    );
    assert_eq!(comments[0].text, "from flat envelope");
}

#[tokio::test]
async fn get_comments_since_rest_returns_empty_when_bug_acknowledged_with_no_comments() {
    // Legitimate empty case: server acknowledges bug 42 but reports no
    // comments. Must return Ok([]) — NOT fall through to flat and NOT error.
    use crate::client::test_helpers::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {"42": {"comments": []}}
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since_rest_for_test(42, None)
        .await
        .unwrap();
    assert!(comments.is_empty(), "no comments expected: {comments:?}");
}

#[tokio::test]
async fn get_comments_since_rest_propagates_attachment_id() {
    // Regression guard for issue #170: the REST envelope path must preserve
    // `Comment.attachment_id` end-to-end so `flip_new_comment_private` can
    // identify the auto-generated upload comment. A wrapper that strips
    // unknown fields before deserialization would pass the type-level unit
    // test in `src/types/comment_tests.rs` while breaking this round trip.
    use crate::client::test_helpers::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {
                            "id": 7,
                            "bug_id": 42,
                            "creator": "alice@example.com",
                            "time": "2026-01-01T00:00:00Z",
                            "creation_time": "2026-01-01T00:00:00Z",
                            "text": "Created attachment 99",
                            "is_private": false,
                            "count": 1,
                            "attachment_id": 99
                        }
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let comments = client
        .get_comments_since_rest_for_test(42, None)
        .await
        .unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].attachment_id, Some(99));
}
