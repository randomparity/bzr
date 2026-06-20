use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::{partition_filters, Bug, SearchParams, FIELD_MAPPINGS};
use crate::xmlrpc::client::XmlRpcClient;
use crate::xmlrpc::mappers::{
    get_datetime_str, get_int_array, get_nonempty_str, get_str, get_str_array, get_u64,
    require_u64, xmlrpc_value_to_json, EXPECTED_STRUCT_RESPONSE,
};
use crate::xmlrpc::value::Value;

impl XmlRpcClient {
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
}

fn add_vec_filters(rpc_params: &mut BTreeMap<String, Value>, params: &SearchParams) {
    let mut chart_idx = 1u32;
    for mapping in FIELD_MAPPINGS {
        let (positive, negated) = partition_filters(params.get_field(mapping.field));
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
        .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;

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

    Ok(Bug {
        id: require_u64(m, "id", "bug")?,
        summary: get_str(m, "summary").unwrap_or_default(),
        status: get_str(m, "status").unwrap_or_default(),
        resolution: get_nonempty_str(m, "resolution"),
        dupe_of: get_u64(m, "dupe_of"),
        deadline: get_nonempty_str(m, "deadline"),
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
        custom_fields: custom_fields_from_xmlrpc(m),
    })
}

fn custom_fields_from_xmlrpc(m: &BTreeMap<String, Value>) -> BTreeMap<String, serde_json::Value> {
    m.iter()
        .filter(|(name, _)| name.starts_with("cf_"))
        .map(|(name, value)| (name.clone(), xmlrpc_value_to_json(value)))
        .collect()
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
