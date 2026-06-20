use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::{GroupInfo, GroupMember};
use crate::xmlrpc::client::XmlRpcClient;
use crate::xmlrpc::mappers::{
    get_bool_flag, get_nonempty_str, get_str, get_u64, require_u64, EXPECTED_STRUCT_RESPONSE,
};
use crate::xmlrpc::value::Value;

impl XmlRpcClient {
    pub async fn get_group(&self, name: &str) -> Result<GroupInfo> {
        let mut rpc_params = BTreeMap::new();
        rpc_params.insert("names".into(), Value::Array(vec![Value::from(name)]));
        rpc_params.insert("membership".into(), Value::Bool(true));

        let result = self.call("Group.get", rpc_params).await?;
        let top = result
            .as_struct()
            .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;
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
}

fn value_to_group_info(val: &Value) -> Result<GroupInfo> {
    let m = val
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc("expected struct for group".into()))?;

    let membership = m
        .get("membership")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let member_map = v.as_struct()?;
                    let member_id = get_u64(member_map, "id")?;
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
        id: require_u64(m, "id", "group")?,
        name: get_str(m, "name").unwrap_or_default(),
        description: get_str(m, "description").unwrap_or_default(),
        is_active: get_bool_flag(m, "is_active"),
        membership,
    })
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
