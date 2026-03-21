use serde::Deserialize;

use super::encode_path;
use super::BugzillaClient;
use crate::error::Result;
use crate::types::{Comment, UpdateCommentTagsParams};

#[derive(Deserialize)]
struct CommentResponse {
    bugs: std::collections::HashMap<String, CommentBugEntry>,
}

#[derive(Deserialize)]
struct CommentBugEntry {
    comments: Vec<Comment>,
}

impl BugzillaClient {
    pub async fn get_comments_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        let mut req_builder = self.http.get(self.url(&format!("bug/{bug_id}/comment")));
        if let Some(since) = since {
            req_builder = req_builder.query(&[("new_since", since)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: CommentResponse = self.parse_json(resp).await?;
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
        let req = self.apply_auth(
            self.http
                .put(self.url(&format!("bug/comment/{comment_id}/tags")))
                .json(params),
        );
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn search_comment_tags(&self, query: &str) -> Result<Vec<String>> {
        let req = self.apply_auth(
            self.http
                .get(self.url(&format!("bug/comment/tags/{}", encode_path(query)))),
        );
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    pub async fn add_comment(&self, bug_id: u64, text: &str) -> Result<u64> {
        let body = serde_json::json!({ "comment": text });
        let req = self.apply_auth(
            self.http
                .post(self.url(&format!("bug/{bug_id}/comment")))
                .json(&body),
        );
        let resp = self.send(req).await?;
        let data: super::IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::test_helpers::test_client;

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
}
