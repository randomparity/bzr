use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::user::CreateUserParams;
use crate::xmlrpc::protocol::Value;
use crate::xmlrpc::protocol::XmlRpcClient;
use crate::xmlrpc::resources::mappers::{require_u64, EXPECTED_STRUCT_RESPONSE};

impl XmlRpcClient {
    pub async fn login(&self, email: &str, password: &str, restrict_login: bool) -> Result<String> {
        let mut params = BTreeMap::new();
        params.insert("login".into(), Value::from(email));
        params.insert("password".into(), Value::from(password));
        params.insert("restrict_login".into(), Value::from(restrict_login));
        let result = self.call("User.login", params).await?;
        let response = result
            .as_struct()
            .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;
        response
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| BzrError::XmlRpc("expected token in User.login response".into()))
    }

    pub async fn logout(&self, token: &str) -> Result<()> {
        let mut params = BTreeMap::new();
        params.insert("Bugzilla_token".into(), Value::from(token));
        self.call("User.logout", params).await?;
        Ok(())
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
}

fn extract_id(response: &Value) -> Result<u64> {
    let m = response
        .as_struct()
        .ok_or_else(|| BzrError::XmlRpc(EXPECTED_STRUCT_RESPONSE.into()))?;

    require_u64(m, "id", "response")
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
