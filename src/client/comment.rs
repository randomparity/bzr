use serde::{Deserialize, Serialize};

use super::encode_path;
use super::BugzillaClient;
use crate::error::Result;
use crate::types::{ApiMode, Comment, UpdateCommentTagsParams};

#[derive(Serialize)]
struct AddCommentBody<'a> {
    comment: &'a str,
    is_private: bool,
}

#[derive(Deserialize)]
struct CommentResponse {
    bugs: std::collections::HashMap<String, CommentBugEntry>,
}

#[derive(Deserialize)]
struct CommentBugEntry {
    comments: Vec<Comment>,
}

impl BugzillaClient {
    /// In Hybrid mode, comments are fetched via XML-RPC `Bug.comments`
    /// rather than REST. Bugzilla 5.0.x REST silently filters private
    /// comments under API-key auth (issue #125), and the truncation is
    /// not reliably detectable from the REST response — XML-RPC is the
    /// only path that returns the full thread. REST is the fallback
    /// when the server doesn't expose `xmlrpc.cgi`.
    pub async fn get_comments_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        match self.api_mode {
            ApiMode::Rest => self.get_comments_since_rest(bug_id, since).await,
            ApiMode::XmlRpc => {
                self.xmlrpc_client()?
                    .get_comments_since(bug_id, since)
                    .await
            }
            ApiMode::Hybrid => {
                match self
                    .xmlrpc_client()?
                    .get_comments_since(bug_id, since)
                    .await
                {
                    Ok(comments) => Ok(comments),
                    Err(e) if e.is_transport_failure() => {
                        tracing::info!(
                            bug_id,
                            error = %e,
                            "XML-RPC comment list failed, retrying via REST"
                        );
                        self.get_comments_since_rest(bug_id, since).await
                    }
                    Err(e) => Err(e),
                }
            }
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

    pub async fn add_comment(&self, bug_id: u64, text: &str, is_private: bool) -> Result<u64> {
        self.post_json_id(
            &format!("bug/{bug_id}/comment"),
            &AddCommentBody {
                comment: text,
                is_private,
            },
        )
        .await
    }
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
