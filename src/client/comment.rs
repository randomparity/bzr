use serde::{Deserialize, Serialize};

use super::encode_path;
use super::BugzillaClient;
use crate::error::Result;
use crate::types::{Comment, UpdateCommentTagsParams};

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

/// Flat envelope variant: `{"comments": [...]}` at the root. Observed
/// on some Bugzilla 5.0.x deployments (issue #135).
#[derive(Deserialize)]
struct FlatCommentsResponse {
    comments: Vec<Comment>,
}

fn extract_bugs_comment_envelope(value: &serde_json::Value) -> Result<Vec<Comment>> {
    let resp = CommentResponse::deserialize(value).map_err(|e| {
        crate::error::BzrError::Deserialize(format!("comments `bugs` envelope: {e}"))
    })?;
    // Treat a structurally empty `bugs` map as a non-match so try_envelopes
    // falls through to the flat extractor. `bugs: {"42": {"comments": []}}`
    // (bug acknowledged, no comments) is a legitimate empty result and still
    // returns Ok(vec![]).
    resp.bugs
        .into_values()
        .next()
        .map(|e| e.comments)
        .ok_or_else(|| {
            crate::error::BzrError::Deserialize(
                "comments `bugs` envelope: empty top-level map".into(),
            )
        })
}

fn extract_flat_comment_envelope(value: &serde_json::Value) -> Result<Vec<Comment>> {
    let resp = FlatCommentsResponse::deserialize(value)
        .map_err(|e| crate::error::BzrError::Deserialize(format!("comments flat envelope: {e}")))?;
    Ok(resp.comments)
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
        self.dispatch_xmlrpc_first(
            &format!("comment list (bug {bug_id})"),
            || self.get_comments_since_rest(bug_id, since),
            || async { self.xmlrpc_client().get_comments_since(bug_id, since).await },
        )
        .await
    }

    async fn get_comments_since_rest(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        let path_str = format!("bug/{bug_id}/comment");
        let value = if let Some(since) = since {
            // get_json_query returns typed T; we need the raw Value for try_envelopes.
            // Build the request manually.
            let req = self.apply_auth(
                self.http
                    .get(self.url(&path_str))
                    .query(&[("new_since", since)]),
            );
            let resp = self.send(req).await?;
            self.parse_json_value(resp).await?
        } else {
            self.get_json_value(&path_str).await?
        };
        Self::try_envelopes(
            &value,
            &[
                ("bugs", extract_bugs_comment_envelope),
                ("comments", extract_flat_comment_envelope),
            ],
        )
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
impl BugzillaClient {
    /// Test-only thin wrapper around the private REST path so tests can
    /// exercise envelope tolerance without going through API-mode dispatch.
    pub(crate) async fn get_comments_since_rest_for_test(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<crate::types::Comment>> {
        self.get_comments_since_rest(bug_id, since).await
    }
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
