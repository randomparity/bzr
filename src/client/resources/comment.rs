use serde::Deserialize;

use crate::client::encode_path;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::types::comment::{AddCommentParams, Comment, UpdateCommentTagsParams};

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
        let mut comments = self
            .dispatch_xmlrpc_first(
                &format!("comment list (bug {bug_id})"),
                || self.get_comments_since_rest(bug_id, since),
                || async { self.xmlrpc_client().get_comments_since(bug_id, since).await },
            )
            .await?;
        // The flat `{"comments": [...]}` envelope carries no bug context, and
        // XML-RPC may omit the field, so a record can arrive unattributed. A
        // server-supplied value wins; the keyed lookup above is what rules out
        // another bug's records reaching this point mislabelled.
        for comment in &mut comments {
            comment.bug_id.get_or_insert(bug_id);
        }
        Ok(comments)
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
        Self::extract_comments_for(&value, bug_id)
    }

    /// Pull one bug's comments out of a REST response.
    ///
    /// The `bugs` map is keyed by bug ID, and the requested key is the only
    /// correct entry: taking whichever key happened to come first would label
    /// another bug's comments as this one's, which multi-ID output has no way
    /// to detect. ADR-0024 sets the same ID-equality rule for `bug adjacency`.
    fn extract_comments_for(value: &serde_json::Value, bug_id: u64) -> Result<Vec<Comment>> {
        let bugs = value.get("bugs");
        if let Some(entry) = bugs.and_then(|b| b.get(bug_id.to_string())) {
            return CommentBugEntry::deserialize(entry)
                .map(|e| e.comments)
                .map_err(|e| {
                    crate::error::BzrError::Deserialize(format!("comments `bugs` envelope: {e}"))
                });
        }
        // No entry for this bug. A populated flat `comments` array is still a
        // valid answer on some Bugzilla 5.0.x deployments (issue #135), so try
        // it before concluding anything.
        let flat = Self::try_envelopes(value, &[("comments", extract_flat_comment_envelope)]);
        match flat {
            Ok(comments) => Ok(comments),
            // A `bugs` map that answered without this bug's key is the server
            // saying it has no record of it -- the same condition the XML-RPC
            // path reports, and one `--permissive` can skip per bug. Without a
            // `bugs` map at all the envelope is simply unrecognised, which is a
            // parse failure and must not be masked as a missing bug.
            Err(e) => {
                if bugs.is_some() {
                    Err(crate::error::BzrError::NotFound {
                        resource: "bug",
                        id: bug_id.to_string(),
                    })
                } else {
                    Err(e)
                }
            }
        }
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

    pub async fn add_comment(&self, bug_id: u64, params: &AddCommentParams) -> Result<u64> {
        self.post_json_id(&format!("bug/{bug_id}/comment"), params)
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
    ) -> Result<Vec<Comment>> {
        self.get_comments_since_rest(bug_id, since).await
    }
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
