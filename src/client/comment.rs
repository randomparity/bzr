use serde::{Deserialize, Serialize};

use super::encode_path;
use super::BugzillaClient;
use crate::error::Result;
use crate::types::{ApiMode, Comment, UpdateCommentTagsParams};

#[derive(Serialize)]
struct AddCommentBody<'a> {
    comment: &'a str,
}

#[derive(Deserialize)]
struct CommentResponse {
    bugs: std::collections::HashMap<String, CommentBugEntry>,
}

#[derive(Deserialize)]
struct CommentBugEntry {
    comments: Vec<Comment>,
}

fn has_count_gaps(comments: &[crate::types::Comment], since_provided: bool) -> bool {
    if comments.is_empty() {
        return false;
    }
    let first = comments[0].count;
    if !since_provided && first != 0 {
        return true;
    }
    comments.windows(2).any(|w| w[1].count != w[0].count + 1)
}

impl BugzillaClient {
    pub async fn get_comments_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        match self.api_mode {
            ApiMode::XmlRpc => {
                return self
                    .xmlrpc_client()?
                    .get_comments_since(bug_id, since)
                    .await;
            }
            ApiMode::Rest | ApiMode::Hybrid => {}
        }

        match self.get_comments_since_rest(bug_id, since).await {
            Ok(comments)
                if self.api_mode == ApiMode::Hybrid
                    && has_count_gaps(&comments, since.is_some()) =>
            {
                tracing::info!(
                    bug_id,
                    rest_count = comments.len(),
                    "REST comment list has count gaps, retrying via XML-RPC"
                );
                self.xmlrpc_client()?
                    .get_comments_since(bug_id, since)
                    .await
            }
            Ok(comments) => Ok(comments),
            Err(e) if self.api_mode == ApiMode::Hybrid && e.is_transport_failure() => {
                tracing::info!(
                    bug_id,
                    error = %e,
                    "REST comment list failed, retrying via XML-RPC"
                );
                self.xmlrpc_client()?
                    .get_comments_since(bug_id, since)
                    .await
            }
            Err(e) => Err(e),
        }
    }

    async fn get_comments_since_rest(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        let data: CommentResponse = if let Some(since) = since {
            self.get_json_query(&format!("bug/{bug_id}/comment"), &[("new_since", since)])
                .await?
        } else {
            self.get_json(&format!("bug/{bug_id}/comment")).await?
        };
        let comments = data
            .bugs
            .into_values()
            .next()
            .map_or_else(Vec::new, |e| e.comments);
        Ok(comments)
    }

    pub async fn update_comment_tags(
        &self,
        comment_id: u64,
        params: &UpdateCommentTagsParams,
    ) -> Result<Vec<String>> {
        self.put_json_response(&format!("bug/comment/{comment_id}/tags"), params)
            .await
    }

    pub async fn search_comment_tags(&self, query: &str) -> Result<Vec<String>> {
        self.get_json(&format!("bug/comment/tags/{}", encode_path(query)))
            .await
    }

    pub async fn add_comment(&self, bug_id: u64, text: &str) -> Result<u64> {
        self.post_json_id(
            &format!("bug/{bug_id}/comment"),
            &AddCommentBody { comment: text },
        )
        .await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::has_count_gaps;
    use crate::client::test_helpers::{test_client, test_client_hybrid, test_client_xmlrpc};

    #[tokio::test]
    async fn update_comment_tags_sends_put() {
        let mock = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/rest/bug/comment/42/tags"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!(["needinfo", "reviewed"])),
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
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!(["needinfo", "needreview"])),
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

    #[test]
    fn has_count_gaps_empty_is_not_gap() {
        assert!(!has_count_gaps(&[], false));
        assert!(!has_count_gaps(&[], true));
    }

    #[test]
    fn has_count_gaps_full_sequence_starting_at_zero() {
        let comments = vec![comment(0), comment(1), comment(2), comment(3), comment(4)];
        assert!(!has_count_gaps(&comments, false));
    }

    #[test]
    fn has_count_gaps_missing_zero_without_since() {
        let comments = vec![comment(4)];
        assert!(has_count_gaps(&comments, false));
    }

    #[test]
    fn has_count_gaps_internal_gap_without_since() {
        let comments = vec![comment(0), comment(4)];
        assert!(has_count_gaps(&comments, false));
    }

    #[test]
    fn has_count_gaps_with_since_contiguous_subset() {
        let comments = vec![comment(5), comment(6), comment(7)];
        assert!(!has_count_gaps(&comments, true));
    }

    #[test]
    fn has_count_gaps_with_since_internal_gap() {
        let comments = vec![comment(5), comment(7)];
        assert!(has_count_gaps(&comments, true));
    }

    fn comment(count: u64) -> crate::types::Comment {
        crate::types::Comment {
            id: count,
            bug_id: 1,
            text: String::new(),
            creator: None,
            creation_time: None,
            count,
            is_private: false,
        }
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
                        <member><name>comments</name><value><array><data>{entries}</data></array></value></member>\
                    </struct></value></member>\
                </struct></value></member>\
            </struct></value></param></params></methodResponse>"
        )
    }

    #[tokio::test]
    async fn hybrid_healthy_rest_no_fallback() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(comments_response_json(&[0, 1, 2, 3])),
            )
            .mount(&mock)
            .await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let comments = client.get_comments_since(42, None).await.unwrap();
        assert_eq!(comments.len(), 4);
        assert_eq!(comments[3].count, 3);
    }

    #[tokio::test]
    async fn hybrid_truncated_rest_falls_back_to_xmlrpc() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(comments_response_json(&[0, 2, 4])),
            )
            .mount(&mock)
            .await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(xmlrpc_comments_response(&[0, 1, 2, 3, 4])),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let comments = client.get_comments_since(42, None).await.unwrap();
        assert_eq!(comments.len(), 5);
        assert_eq!(
            comments.iter().map(|c| c.count).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        // Confirm we got the XML-RPC text (proves the fallback fired):
        assert_eq!(comments[1].text, "xmlrpc 1");
    }

    #[tokio::test]
    async fn hybrid_rest_transport_error_falls_back_to_xmlrpc() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(ResponseTemplate::new(502))
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
    async fn rest_mode_truncated_no_fallback() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

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

        let client = test_client(&mock.uri()); // Rest mode
        let comments = client.get_comments_since(42, None).await.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].count, 4);
    }

    #[tokio::test]
    async fn xmlrpc_mode_skips_rest() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

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
}
