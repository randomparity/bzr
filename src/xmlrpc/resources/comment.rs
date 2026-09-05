use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::comment::Comment;
use crate::xmlrpc::protocol::Value;
use crate::xmlrpc::protocol::XmlRpcClient;
use crate::xmlrpc::resources::mappers::{
    get_datetime_str, get_nonempty_str, get_optional_bool_flag, get_str, get_str_array, get_u64,
    lookup_bug_entry, require_u64, xmlrpc_id,
};

impl XmlRpcClient {
    pub async fn get_comments_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<Comment>> {
        let mut rpc_params = BTreeMap::new();
        let bug_id_value = xmlrpc_id(bug_id, "bug ID")?;
        rpc_params.insert("ids".into(), Value::Array(vec![bug_id_value]));
        if let Some(s) = since {
            rpc_params.insert("new_since".into(), Value::from(s));
        }

        let result = self.call("Bug.comments", rpc_params).await?;
        extract_comments(&result, bug_id)
    }
}

fn extract_comments(response: &Value, bug_id: u64) -> Result<Vec<Comment>> {
    // No entry for this bug is the server saying it has no record of it, not
    // that the bug has zero comments — Bugzilla returns the key with an empty
    // array for that. Conflating them drops the bug silently, which is
    // invisible once the caller requests several (issue #699).
    let Some(bug_entry) = lookup_bug_entry(response, bug_id)? else {
        return Err(BzrError::NotFound {
            resource: "bug",
            id: bug_id.to_string(),
        });
    };

    let entry_struct = bug_entry
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected bug entry struct".into()))?;

    let Some(comments_val) = entry_struct.get("comments") else {
        return Ok(Vec::new());
    };

    let comments_arr = comments_val
        .as_array()
        .ok_or_else(|| BzrError::XmlRpc("expected comments array".into()))?;

    let mut comments = Vec::with_capacity(comments_arr.len());
    for c in comments_arr {
        comments.push(value_to_comment(c)?);
    }
    Ok(comments)
}

fn value_to_comment(val: &Value) -> Result<Comment> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for comment".into()))?;

    Ok(Comment {
        id: require_u64(m, "id", "comment")?,
        bug_id: get_u64(m, "bug_id"),
        text: get_str(m, "text"),
        creator: get_nonempty_str(m, "creator"),
        creation_time: get_datetime_str(m, "creation_time"),
        count: get_u64(m, "count"),
        is_private: get_optional_bool_flag(m, "is_private"),
        attachment_id: get_u64(m, "attachment_id"),
        tags: get_str_array(m, "tags"),
    })
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
