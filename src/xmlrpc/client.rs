use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::http::AUTH_QUERY_PARAM;
use crate::types::{
    partition_filters, Bug, CreateUserParams, GroupInfo, GroupMember, SearchParams, FIELD_MAPPINGS,
};
use crate::xmlrpc::call::build_request;
use crate::xmlrpc::parsing::parse_response;
use crate::xmlrpc::value::Value;

const ATTACHMENT_LIST_FIELDS: &[&str] = &[
    "id",
    "bug_id",
    "file_name",
    "summary",
    "content_type",
    "creator",
    "creation_time",
    "last_change_time",
    "size",
    "is_obsolete",
    "is_private",
    "is_patch",
    "data",
];

pub struct XmlRpcClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl XmlRpcClient {
    pub fn new(http: reqwest::Client, base_url: &str, api_key: &str) -> Self {
        XmlRpcClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    // SECURITY: The request body contains Bugzilla_api_key in plain text.
    // Never log the request body. Response bodies are safe to log at trace
    // level since Bugzilla does not echo auth credentials back.
    //
    // NOTE: XML-RPC always transmits the API key in the request body
    // (as a method parameter), regardless of the REST AuthMethod detected
    // for this server. This is an inherent XML-RPC protocol constraint —
    // there is no header-based auth equivalent for XML-RPC calls.
    async fn call(&self, method: &str, mut params: BTreeMap<String, Value>) -> Result<Value> {
        params.insert(AUTH_QUERY_PARAM.into(), Value::from(self.api_key.as_str()));

        let body = build_request(method, params);
        let url = format!("{}/xmlrpc.cgi", self.base_url);

        tracing::debug!(
            method,
            url = %self.base_url,
            "XML-RPC call"
        );

        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            let body = resp.text().await.unwrap_or_else(|e| {
                tracing::warn!("failed to read XML-RPC error response body: {e}");
                String::new()
            });
            tracing::debug!(%status, body = &body[..body.len().min(512)], "XML-RPC HTTP error");
            return Err(BzrError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        let body_text = resp.text().await?;
        tracing::trace!(body_len = body_text.len(), "XML-RPC response received");

        parse_response(&body_text)
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let mut rpc_params = BTreeMap::new();

        // Multi-value Vec fields: positive values sent as XML-RPC arrays,
        // negated values sent as fN/oN/vN boolean chart params.
        add_vec_filters(&mut rpc_params, params);

        // Single-value Option fields
        let option_fields: &[(&str, &Option<String>)] = &[
            ("cc", &params.cc),
            ("alias", &params.alias),
            ("summary", &params.summary),
            ("quicksearch", &params.quicksearch),
            ("creation_time", &params.creation_time),
            ("last_change_time", &params.last_change_time),
        ];
        for &(key, value) in option_fields {
            if let Some(ref v) = *value {
                rpc_params.insert(key.into(), Value::from(v.as_str()));
            }
        }

        if !params.id.is_empty() {
            #[expect(clippy::cast_possible_wrap, reason = "bug IDs fit in i64")]
            let ids: Vec<Value> = params.id.iter().map(|id| Value::Int(*id as i64)).collect();
            rpc_params.insert("ids".into(), Value::Array(ids));
        }
        if let Some(limit) = params.limit {
            rpc_params.insert("limit".into(), Value::Int(i64::from(limit)));
        }
        add_field_lists(&mut rpc_params, params);

        let result = self.call("Bug.search", rpc_params).await?;
        extract_bugs(&result)
    }

    pub async fn get_bug(&self, id: &str) -> Result<Bug> {
        let mut rpc_params = BTreeMap::new();

        // Try parsing as integer ID first, fall back to alias.
        // Both must be wrapped in an array — Bugzilla XML-RPC requires
        // ids to always be an array, even for aliases.
        if let Ok(numeric_id) = id.parse::<i64>() {
            rpc_params.insert("ids".into(), Value::Array(vec![Value::Int(numeric_id)]));
        } else {
            rpc_params.insert("ids".into(), Value::Array(vec![Value::from(id)]));
        }

        let result = self.call("Bug.get", rpc_params).await?;
        let mut bugs = extract_bugs(&result)?;
        if bugs.is_empty() {
            return Err(BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            });
        }
        Ok(bugs.swap_remove(0))
    }

    pub async fn create_user(&self, params: &CreateUserParams) -> Result<u64> {
        let mut rpc_params = BTreeMap::new();
        rpc_params.insert("email".into(), Value::from(params.email.as_str()));
        if let Some(ref login) = params.login {
            rpc_params.insert("login".into(), Value::from(login.as_str()));
        }
        if let Some(ref full_name) = params.full_name {
            rpc_params.insert("full_name".into(), Value::from(full_name.as_str()));
        }
        if let Some(ref password) = params.password {
            rpc_params.insert("password".into(), Value::from(password.as_str()));
        }

        let result = self.call("User.create", rpc_params).await?;
        extract_id(&result)
    }

    pub async fn get_group(&self, name: &str) -> Result<GroupInfo> {
        let mut rpc_params = BTreeMap::new();
        rpc_params.insert("names".into(), Value::Array(vec![Value::from(name)]));
        rpc_params.insert("membership".into(), Value::Bool(true));

        let result = self.call("Group.get", rpc_params).await?;
        let top = result
            .as_struct()
            .ok_or_else(|| BzrError::XmlRpc("expected struct response".into()))?;
        let groups = top
            .get("groups")
            .and_then(Value::as_array)
            .ok_or_else(|| BzrError::XmlRpc("expected groups array".into()))?;
        let group_val = groups.first().ok_or_else(|| BzrError::NotFound {
            resource: "group",
            id: name.to_string(),
        })?;
        value_to_group_info(group_val)
    }

    pub async fn get_comments_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<crate::types::Comment>> {
        let mut rpc_params = BTreeMap::new();
        #[expect(clippy::cast_possible_wrap, reason = "bug IDs fit in i64")]
        let bug_id_value = Value::Int(bug_id as i64);
        rpc_params.insert("ids".into(), Value::Array(vec![bug_id_value]));
        if let Some(s) = since {
            rpc_params.insert("new_since".into(), Value::from(s));
        }

        let result = self.call("Bug.comments", rpc_params).await?;
        extract_comments(&result, bug_id)
    }

    pub async fn get_attachments(&self, bug_id: u64) -> Result<Vec<crate::types::Attachment>> {
        let mut rpc_params = BTreeMap::new();
        #[expect(clippy::cast_possible_wrap, reason = "bug IDs fit in i64")]
        let bug_id_value = Value::Int(bug_id as i64);
        rpc_params.insert("ids".into(), Value::Array(vec![bug_id_value]));
        let include_fields = ATTACHMENT_LIST_FIELDS
            .iter()
            .copied()
            .map(Value::from)
            .collect();
        rpc_params.insert("include_fields".into(), Value::Array(include_fields));

        let result = self.call("Bug.attachments", rpc_params).await?;
        extract_attachments(&result, bug_id)
    }

    pub async fn get_attachment_by_id(
        &self,
        attachment_id: u64,
    ) -> Result<crate::types::Attachment> {
        let mut rpc_params = BTreeMap::new();
        #[expect(clippy::cast_possible_wrap, reason = "attachment IDs fit in i64")]
        let id_value = Value::Int(attachment_id as i64);
        rpc_params.insert("attachment_ids".into(), Value::Array(vec![id_value]));

        let result = self.call("Bug.attachments", rpc_params).await?;
        extract_attachment_by_id(&result, attachment_id)
    }
}

fn extract_id(response: &Value) -> Result<u64> {
    let m = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct response".into()))?;

    let id = m
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| BzrError::XmlRpc("response missing id field".into()))?;

    #[expect(clippy::cast_sign_loss, reason = "user IDs are non-negative")]
    Ok(id as u64)
}

fn add_vec_filters(rpc_params: &mut BTreeMap<String, Value>, params: &SearchParams) {
    let mut chart_idx = 1u32;
    for mapping in FIELD_MAPPINGS {
        let (positive, negated) =
            partition_filters(params.get_field(mapping.struct_field).unwrap_or_default());
        if !positive.is_empty() {
            let arr: Vec<Value> = positive.iter().map(|v| Value::from(*v)).collect();
            rpc_params.insert(mapping.struct_field.into(), Value::Array(arr));
        }
        for v in negated {
            rpc_params.insert(format!("f{chart_idx}"), Value::from(mapping.internal_name));
            rpc_params.insert(
                format!("o{chart_idx}"),
                Value::from(mapping.negation_operator.as_str()),
            );
            rpc_params.insert(format!("v{chart_idx}"), Value::from(v));
            chart_idx += 1;
        }
    }
}

fn add_field_lists(rpc_params: &mut BTreeMap<String, Value>, params: &SearchParams) {
    // Bugzilla XML-RPC requires field lists as arrays; the REST API accepts CSV strings.
    for (key, value) in [
        ("include_fields", &params.include_fields),
        ("exclude_fields", &params.exclude_fields),
    ] {
        if let Some(ref fields) = *value {
            let arr: Vec<Value> = fields.split(',').map(|f| Value::from(f.trim())).collect();
            rpc_params.insert(key.into(), Value::Array(arr));
        }
    }
}

fn extract_bugs(response: &Value) -> Result<Vec<Bug>> {
    let top = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct response".into()))?;

    let Some(bugs_val) = top.get("bugs") else {
        return Ok(Vec::new());
    };

    let bugs_arr = bugs_val
        .as_array()
        .ok_or_else(|| BzrError::XmlRpc("expected bugs array".into()))?;

    let mut bugs = Vec::with_capacity(bugs_arr.len());
    for bug_val in bugs_arr {
        bugs.push(value_to_bug(bug_val)?);
    }
    Ok(bugs)
}

fn value_to_bug(val: &Value) -> Result<Bug> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for bug".into()))?;

    let id = m
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| BzrError::XmlRpc("bug missing id field".into()))?;

    Ok(Bug {
        #[expect(clippy::cast_sign_loss, reason = "bug IDs are non-negative")]
        id: id as u64,
        summary: get_str(m, "summary").unwrap_or_default(),
        status: get_str(m, "status").unwrap_or_default(),
        resolution: get_nonempty_str(m, "resolution"),
        dupe_of: get_u64(m, "dupe_of"),
        product: get_nonempty_str(m, "product"),
        component: get_nonempty_str(m, "component"),
        version: get_nonempty_str(m, "version"),
        assigned_to: get_nonempty_str(m, "assigned_to"),
        priority: get_nonempty_str(m, "priority"),
        severity: get_nonempty_str(m, "severity"),
        creation_time: get_datetime_str(m, "creation_time"),
        last_change_time: get_datetime_str(m, "last_change_time"),
        creator: get_nonempty_str(m, "creator"),
        url: get_nonempty_str(m, "url"),
        whiteboard: get_nonempty_str(m, "whiteboard"),
        keywords: get_str_array(m, "keywords"),
        blocks: get_int_array(m, "blocks"),
        depends_on: get_int_array(m, "depends_on"),
        cc: get_str_array(m, "cc"),
        op_sys: get_nonempty_str(m, "op_sys"),
        rep_platform: get_nonempty_str(m, "rep_platform"),
    })
}

/// Navigate a `Bug.*` XML-RPC response from `top -> bugs -> {bug_id_str}`.
///
/// Returns `Ok(None)` when the server acknowledged the call but didn't
/// return data for this bug (caller should treat as empty result, not
/// error). Returns `Err` when the response shape is malformed.
fn lookup_bug_entry(response: &Value, bug_id: u64) -> Result<Option<&Value>> {
    let top = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct response".into()))?;

    let Some(bugs_val) = top.get("bugs") else {
        return Ok(None);
    };

    let bugs_struct = bugs_val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected bugs to be a struct keyed by bug ID".into()))?;

    // Bugzilla returns the inner key as a string even though the input is
    // an integer. Look up by string form.
    Ok(bugs_struct.get(&bug_id.to_string()))
}

fn extract_comments(response: &Value, bug_id: u64) -> Result<Vec<crate::types::Comment>> {
    let Some(bug_entry) = lookup_bug_entry(response, bug_id)? else {
        return Ok(Vec::new());
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

fn value_to_comment(val: &Value) -> Result<crate::types::Comment> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for comment".into()))?;

    let id = m
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| BzrError::XmlRpc("comment missing id field".into()))?;
    let bug_id = m.get("bug_id").and_then(Value::as_i64).unwrap_or(0);
    let count = m.get("count").and_then(Value::as_i64).unwrap_or(0);

    #[expect(clippy::cast_sign_loss, reason = "comment IDs are non-negative")]
    let id = id as u64;
    #[expect(clippy::cast_sign_loss, reason = "bug IDs are non-negative")]
    let bug_id = bug_id as u64;
    #[expect(clippy::cast_sign_loss, reason = "comment counts are non-negative")]
    let count = count as u64;
    Ok(crate::types::Comment {
        id,
        bug_id,
        text: get_str(m, "text").unwrap_or_default(),
        creator: get_nonempty_str(m, "creator"),
        creation_time: get_datetime_str(m, "creation_time"),
        count,
        is_private: get_bool_flag(m, "is_private"),
        attachment_id: m
            .get("attachment_id")
            .and_then(Value::as_i64)
            .and_then(|v| u64::try_from(v).ok()),
    })
}

fn extract_attachments(response: &Value, bug_id: u64) -> Result<Vec<crate::types::Attachment>> {
    let Some(bug_entry) = lookup_bug_entry(response, bug_id)? else {
        return Ok(Vec::new());
    };

    let attachments_arr = bug_entry
        .as_array()
        .ok_or_else(|| BzrError::XmlRpc("expected attachments array".into()))?;

    let mut attachments = Vec::with_capacity(attachments_arr.len());
    for a in attachments_arr {
        attachments.push(value_to_attachment(a)?);
    }
    Ok(attachments)
}

fn extract_attachment_by_id(
    response: &Value,
    attachment_id: u64,
) -> Result<crate::types::Attachment> {
    let top = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct response".into()))?;

    let attachments_struct = top
        .get("attachments")
        .and_then(Value::as_struct)
        .ok_or_else(|| {
            BzrError::XmlRpc("expected attachments to be a struct keyed by attachment ID".into())
        })?;

    let key = attachment_id.to_string();
    let entry = attachments_struct
        .get(&key)
        .ok_or_else(|| BzrError::NotFound {
            resource: "attachment",
            id: attachment_id.to_string(),
        })?;

    value_to_attachment(entry)
}

fn value_to_attachment(val: &Value) -> Result<crate::types::Attachment> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for attachment".into()))?;

    let id = m
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| BzrError::XmlRpc("attachment missing id field".into()))?;
    let bug_id = m.get("bug_id").and_then(Value::as_i64).unwrap_or(0);
    let size = m.get("size").and_then(Value::as_i64).unwrap_or(0);

    #[expect(clippy::cast_sign_loss, reason = "attachment IDs are non-negative")]
    let id = id as u64;
    #[expect(clippy::cast_sign_loss, reason = "bug IDs are non-negative")]
    let bug_id = bug_id as u64;
    #[expect(clippy::cast_sign_loss, reason = "attachment sizes are non-negative")]
    let size = size as u64;

    let data = match m.get("data") {
        Some(Value::Base64(bytes)) => Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        )),
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };

    Ok(crate::types::Attachment {
        id,
        bug_id,
        file_name: get_str(m, "file_name").unwrap_or_default(),
        summary: get_str(m, "summary").unwrap_or_default(),
        content_type: get_str(m, "content_type").unwrap_or_default(),
        creator: get_nonempty_str(m, "creator"),
        creation_time: get_datetime_str(m, "creation_time"),
        last_change_time: get_datetime_str(m, "last_change_time"),
        size,
        is_obsolete: get_bool_flag(m, "is_obsolete"),
        is_private: get_bool_flag(m, "is_private"),
        is_patch: get_bool_flag(m, "is_patch"),
        data,
    })
}

/// Returns a struct member as a bool, accepting either `<boolean>1</boolean>`
/// or `<int>1</int>` on the wire. Bugzilla 5.0.x XML-RPC responses use both
/// shapes interchangeably for the same flag depending on the field.
fn get_bool_flag(m: &BTreeMap<String, Value>, key: &str) -> bool {
    match m.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Int(n)) => *n != 0,
        _ => false,
    }
}

fn get_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(String::from)
}

fn get_nonempty_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    let val = m.get(key)?;
    match val {
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn get_datetime_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    let val = m.get(key)?;
    match val {
        Value::DateTime(s) => Some(s.clone()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn get_u64(m: &BTreeMap<String, Value>, key: &str) -> Option<u64> {
    m.get(key)
        .and_then(Value::as_i64)
        .and_then(|v| u64::try_from(v).ok())
}

fn get_str_array(m: &BTreeMap<String, Value>, key: &str) -> Vec<String> {
    m.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn get_int_array(m: &BTreeMap<String, Value>, key: &str) -> Vec<u64> {
    m.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_i64)
                .map(|v| {
                    #[expect(clippy::cast_sign_loss, reason = "bug IDs are non-negative")]
                    let id = v as u64;
                    id
                })
                .collect()
        })
        .unwrap_or_default()
}

fn value_to_group_info(val: &Value) -> Result<GroupInfo> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for group".into()))?;

    let id = m
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| BzrError::XmlRpc("group missing id".into()))?;

    #[expect(clippy::cast_sign_loss, reason = "group IDs are non-negative")]
    let id = id as u64;

    let membership = m
        .get("membership")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let member_map = v.as_struct()?;
                    let member_id = member_map.get("id").and_then(Value::as_i64)?;
                    #[expect(clippy::cast_sign_loss, reason = "user IDs are non-negative")]
                    let member_id = member_id as u64;
                    Some(GroupMember {
                        id: member_id,
                        name: get_str(member_map, "name").unwrap_or_default(),
                        real_name: get_nonempty_str(member_map, "real_name"),
                        email: get_nonempty_str(member_map, "email"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(GroupInfo {
        id,
        name: get_str(m, "name").unwrap_or_default(),
        description: get_str(m, "description").unwrap_or_default(),
        is_active: get_bool_flag(m, "is_active"),
        membership,
    })
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
