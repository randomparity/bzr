use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::{BugAdjacencyBug, BugAdjacencyError};
use crate::xmlrpc::protocol::{Value, XmlRpcClient};

const BUG_ADJACENCY_FIELDS: &[&str] = &[
    "id",
    "summary",
    "status",
    "resolution",
    "product",
    "version",
    "assigned_to",
    "last_change_time",
    "target_milestone",
    "blocks",
    "depends_on",
];

impl XmlRpcClient {
    pub(crate) async fn get_bug_adjacency(
        &self,
        requested: &str,
    ) -> Result<std::result::Result<BugAdjacencyBug, BugAdjacencyError>> {
        let identity = crate::client::parse_adjacency_numeric(requested)
            .map_or_else(|| Value::from(requested), Value::Int);
        let mut params = BTreeMap::new();
        params.insert("ids".into(), Value::Array(vec![identity]));
        params.insert("permissive".into(), Value::Bool(true));
        params.insert(
            "include_fields".into(),
            Value::Array(
                BUG_ADJACENCY_FIELDS
                    .iter()
                    .map(|field| Value::from(*field))
                    .collect(),
            ),
        );

        let response = self.call_strict("Bug.get", params).await?;
        parse_strict_response(&response, requested)
    }
}

fn parse_strict_response(
    response: &Value,
    requested: &str,
) -> Result<std::result::Result<BugAdjacencyBug, BugAdjacencyError>> {
    let top = response.as_struct().ok_or_else(|| {
        BzrError::DataIntegrity("strict XML-RPC Bug.get response must be a struct".into())
    })?;
    if top.keys().any(|key| key != "bugs" && key != "faults") {
        return Err(BzrError::DataIntegrity(
            "strict XML-RPC Bug.get response has unknown fields".into(),
        ));
    }
    let bugs = strict_array(top.get("bugs"), "bugs")?;
    let faults = strict_array(top.get("faults"), "faults")?;
    match (bugs, faults) {
        ([bug], []) => Ok(Ok(strict_bug(bug, requested)?)),
        ([], [fault]) => Ok(Err(strict_fault(fault, requested)?)),
        _ => Err(BzrError::DataIntegrity(format!(
            "strict XML-RPC Bug.get for '{requested}' must return exactly one bug or fault"
        ))),
    }
}

fn strict_array<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a [Value]> {
    value.map_or(Ok(&[]), |value| {
        value.as_array().ok_or_else(|| {
            BzrError::DataIntegrity(format!(
                "strict XML-RPC Bug.get {field} field must be an array"
            ))
        })
    })
}

fn strict_bug(value: &Value, requested: &str) -> Result<BugAdjacencyBug> {
    let bug = value.as_struct().ok_or_else(|| {
        BzrError::DataIntegrity("strict XML-RPC Bug.get bug must be a struct".into())
    })?;
    if bug
        .get("assigned_to_detail")
        .is_some_and(|detail| detail.as_struct().is_none())
    {
        return Err(BzrError::DataIntegrity(
            "strict XML-RPC Bug.get assigned_to_detail must be a struct".into(),
        ));
    }
    if bug
        .keys()
        .any(|key| key != "assigned_to_detail" && !BUG_ADJACENCY_FIELDS.contains(&key.as_str()))
    {
        return Err(BzrError::DataIntegrity(
            "strict XML-RPC Bug.get bug has unknown fields".into(),
        ));
    }
    let id = strict_id(bug.get("id"), "id")?;
    if let Some(requested_id) = crate::client::parse_adjacency_numeric(requested) {
        if id != u64::try_from(requested_id).unwrap_or(u64::MAX) {
            return Err(BzrError::DataIntegrity(format!(
                "strict XML-RPC Bug.get ID {id} does not match requested '{requested}'"
            )));
        }
    }

    Ok(BugAdjacencyBug {
        id,
        summary: strict_string_scalar(bug.get("summary"), "summary")?,
        status: strict_string_scalar(bug.get("status"), "status")?,
        resolution: strict_string_scalar(bug.get("resolution"), "resolution")?,
        product: strict_string_scalar(bug.get("product"), "product")?,
        version: strict_string_scalar(bug.get("version"), "version")?.map(|value| vec![value]),
        assigned_to: strict_string_scalar(bug.get("assigned_to"), "assigned_to")?,
        last_change_time: strict_time_scalar(bug.get("last_change_time"), "last_change_time")?,
        target_milestone: strict_string_scalar(bug.get("target_milestone"), "target_milestone")?,
        blocks: strict_ids(bug.get("blocks"), "blocks")?,
        depends_on: strict_ids(bug.get("depends_on"), "depends_on")?,
    })
}

fn strict_fault(value: &Value, requested: &str) -> Result<BugAdjacencyError> {
    let fault = value.as_struct().ok_or_else(|| {
        BzrError::DataIntegrity("strict XML-RPC Bug.get fault must be a struct".into())
    })?;
    if fault
        .keys()
        .any(|key| !matches!(key.as_str(), "id" | "faultCode" | "faultString"))
    {
        return Err(BzrError::DataIntegrity(
            "strict XML-RPC Bug.get fault has unknown fields".into(),
        ));
    }
    validate_fault_identity(fault.get("id"), requested)?;
    let code = fault
        .get("faultCode")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            BzrError::DataIntegrity("strict XML-RPC Bug.get faultCode must be an integer".into())
        })?;
    if let Some(message) = fault.get("faultString") {
        message.as_str().ok_or_else(|| {
            BzrError::DataIntegrity("strict XML-RPC Bug.get faultString must be a string".into())
        })?;
    }
    let numeric = crate::client::parse_adjacency_numeric(requested).is_some();
    match code {
        100 if !numeric => Ok(BugAdjacencyError::NotFoundAlias),
        101 if numeric => Ok(BugAdjacencyError::NotFoundId),
        102 => Ok(BugAdjacencyError::Inaccessible),
        _ => Err(BzrError::DataIntegrity(format!(
            "strict XML-RPC Bug.get returned uncorrelated fault code {code} for '{requested}'"
        ))),
    }
}

fn validate_fault_identity(value: Option<&Value>, requested: &str) -> Result<()> {
    let matches = if let Some(requested_id) = crate::client::parse_adjacency_numeric(requested) {
        value.is_some_and(|value| {
            value.as_i64() == Some(requested_id)
                || value.as_str().and_then(|id| id.parse::<i64>().ok()) == Some(requested_id)
        })
    } else {
        value.and_then(Value::as_str) == Some(requested)
    };
    if matches {
        return Ok(());
    }
    Err(BzrError::DataIntegrity(format!(
        "strict XML-RPC Bug.get fault identity does not match requested '{requested}'"
    )))
}

fn strict_string_scalar(value: Option<&Value>, field: &str) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Value::String(text) = value else {
        return Err(BzrError::DataIntegrity(format!(
            "strict XML-RPC Bug.get {field} must be a string"
        )));
    };
    Ok((!text.is_empty()).then(|| text.clone()))
}

fn strict_time_scalar(value: Option<&Value>, field: &str) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let (Value::String(text) | Value::DateTime(text)) = value else {
        return Err(BzrError::DataIntegrity(format!(
            "strict XML-RPC Bug.get {field} must be a string or datetime"
        )));
    };
    Ok((!text.is_empty()).then(|| text.clone()))
}

fn strict_ids(value: Option<&Value>, field: &str) -> Result<Vec<u64>> {
    let values = value.and_then(Value::as_array).ok_or_else(|| {
        BzrError::DataIntegrity(format!(
            "strict XML-RPC Bug.get {field} must be a present array"
        ))
    })?;
    let mut ids = values
        .iter()
        .map(|value| strict_id(Some(value), field))
        .collect::<Result<Vec<_>>>()?;
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

fn strict_id(value: Option<&Value>, field: &str) -> Result<u64> {
    let id = value.and_then(Value::as_i64).ok_or_else(|| {
        BzrError::DataIntegrity(format!("strict XML-RPC Bug.get {field} must be an integer"))
    })?;
    u64::try_from(id).map_err(|_| {
        BzrError::DataIntegrity(format!(
            "strict XML-RPC Bug.get {field} must be non-negative"
        ))
    })
}

#[cfg(test)]
#[path = "bug_adjacency_tests.rs"]
mod tests;
