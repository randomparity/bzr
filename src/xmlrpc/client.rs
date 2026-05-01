use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::http::AUTH_QUERY_PARAM;
use crate::types::{
    partition_filters, Bug, CreateUserParams, GroupInfo, GroupMember, SearchParams, FIELD_MAPPINGS,
};
use crate::xmlrpc::{self, Value};

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

        let body = xmlrpc::build_request(method, params);
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

        xmlrpc::parse_response(&body_text)
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
        let (positive, negated) = partition_filters(params.get_field(mapping.struct_field));
        if !positive.is_empty() {
            let arr: Vec<Value> = positive.iter().map(|v| Value::from(*v)).collect();
            rpc_params.insert(mapping.struct_field.into(), Value::Array(arr));
        }
        for v in negated {
            rpc_params.insert(format!("f{chart_idx}"), Value::from(mapping.internal_name));
            rpc_params.insert(format!("o{chart_idx}"), Value::from("notequals"));
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
        is_active: m.get("is_active").and_then(Value::as_bool).unwrap_or(false),
        membership,
    })
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn test_http_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    use crate::test_helpers::xmlrpc_bug_response;

    fn xmlrpc_fault_response(code: i64, message: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse>
              <fault>
                <value>
                  <struct>
                    <member>
                      <name>faultCode</name>
                      <value><int>{code}</int></value>
                    </member>
                    <member>
                      <name>faultString</name>
                      <value><string>{message}</string></value>
                    </member>
                  </struct>
                </value>
              </fault>
            </methodResponse>"#
        )
    }

    #[tokio::test]
    async fn search_bugs_returns_results() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(42, "Test bug")),
            )
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let params = SearchParams {
            product: vec!["TestProduct".into()],
            limit: Some(10),
            ..Default::default()
        };

        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, 42);
        assert_eq!(bugs[0].summary, "Test bug");
        assert_eq!(bugs[0].status, "NEW");
        assert_eq!(bugs[0].product.as_deref(), Some("TestProduct"));
    }

    #[tokio::test]
    async fn search_bugs_empty_result() {
        let mock = MockServer::start().await;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse>
              <params>
                <param>
                  <value>
                    <struct>
                      <member>
                        <name>bugs</name>
                        <value><array><data></data></array></value>
                      </member>
                    </struct>
                  </value>
                </param>
              </params>
            </methodResponse>"#;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(xml))
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let params = SearchParams {
            product: vec!["Empty".into()],
            ..Default::default()
        };

        let bugs = client.search_bugs(&params).await.unwrap();
        assert!(bugs.is_empty());
    }

    #[tokio::test]
    async fn get_bug_by_id() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(xmlrpc_bug_response(100, "Specific bug")),
            )
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let bug = client.get_bug("100").await.unwrap();
        assert_eq!(bug.id, 100);
        assert_eq!(bug.summary, "Specific bug");
    }

    #[tokio::test]
    async fn fault_response_maps_to_error() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(xmlrpc_fault_response(102, "Access Denied")),
            )
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let err = client.get_bug("1").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("102"), "should contain fault code: {msg}");
        assert!(
            msg.contains("Access Denied"),
            "should contain message: {msg}"
        );
    }

    #[tokio::test]
    async fn get_bug_by_alias() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(55, "Alias bug")),
            )
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let bug = client.get_bug("my-alias").await.unwrap();
        assert_eq!(bug.id, 55);
        assert_eq!(bug.summary, "Alias bug");
    }

    #[tokio::test]
    async fn http_error_maps_to_xmlrpc_error() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let err = client.get_bug("1").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("500"), "should contain status code: {msg}");
    }

    #[tokio::test]
    async fn search_bugs_multi_value_sends_array() {
        let mock = MockServer::start().await;
        // Verify the XML body contains both status values as array members
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .and(body_string_contains("<string>NEW</string>"))
            .and(body_string_contains("<string>ASSIGNED</string>"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(1, "Multi bug")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let params = SearchParams {
            status: vec!["NEW".into(), "ASSIGNED".into()],
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
    }

    #[tokio::test]
    async fn search_bugs_negation_sends_boolean_chart() {
        let mock = MockServer::start().await;
        // Verify the XML body contains boolean chart params for negation
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .and(body_string_contains("<string>bug_status</string>"))
            .and(body_string_contains("<string>notequals</string>"))
            .and(body_string_contains("<string>CLOSED</string>"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(2, "Open bug")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let params = SearchParams {
            status: vec!["!CLOSED".into()],
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
    }

    #[tokio::test]
    async fn search_bugs_fields_and_ids_use_xmlrpc_arrays() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .and(body_string_contains("<name>ids</name>"))
            .and(body_string_contains("<int>42</int>"))
            .and(body_string_contains("<name>include_fields</name>"))
            .and(body_string_contains("<string>id</string>"))
            .and(body_string_contains("<string>summary</string>"))
            .and(body_string_contains("<name>exclude_fields</name>"))
            .and(body_string_contains("<string>cc</string>"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(42, "Field bug")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let params = SearchParams {
            id: vec![42],
            include_fields: Some("id, summary".into()),
            exclude_fields: Some("cc".into()),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, 42);
    }

    #[tokio::test]
    async fn get_bug_empty_result_is_not_found() {
        let mock = MockServer::start().await;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse>
              <params>
                <param>
                  <value>
                    <struct>
                      <member>
                        <name>bugs</name>
                        <value><array><data></data></array></value>
                      </member>
                    </struct>
                  </value>
                </param>
              </params>
            </methodResponse>"#;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200).set_body_string(xml))
            .expect(1)
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let err = client.get_bug("42").await.unwrap_err();
        assert!(matches!(
            err,
            BzrError::NotFound {
                resource: "bug",
                ..
            }
        ));
    }

    #[test]
    fn extract_id_requires_struct_with_integer_id() {
        let err = extract_id(&Value::String("oops".into())).unwrap_err();
        assert!(err.to_string().contains("expected struct response"));

        let err = extract_id(&Value::Struct(BTreeMap::new())).unwrap_err();
        assert!(err.to_string().contains("missing id field"));
    }

    #[test]
    fn extract_bugs_rejects_non_array_payload() {
        let mut payload = BTreeMap::new();
        payload.insert("bugs".into(), Value::String("wrong".into()));
        let err = extract_bugs(&Value::Struct(payload)).unwrap_err();
        assert!(err.to_string().contains("expected bugs array"));
    }

    #[test]
    fn value_to_group_info_parses_membership_and_optional_fields() {
        let mut member = BTreeMap::new();
        member.insert("id".into(), Value::Int(7));
        member.insert("name".into(), Value::String("alice@example.com".into()));
        member.insert("real_name".into(), Value::String("Alice".into()));
        member.insert("email".into(), Value::String("alice@example.com".into()));

        let mut group = BTreeMap::new();
        group.insert("id".into(), Value::Int(1));
        group.insert("name".into(), Value::String("admin".into()));
        group.insert("description".into(), Value::String("Administrators".into()));
        group.insert("is_active".into(), Value::Bool(true));
        group.insert(
            "membership".into(),
            Value::Array(vec![Value::Struct(member)]),
        );

        let info = value_to_group_info(&Value::Struct(group)).unwrap();
        assert_eq!(info.name, "admin");
        assert!(info.is_active);
        assert_eq!(info.membership.len(), 1);
        assert_eq!(info.membership[0].id, 7);
        assert_eq!(info.membership[0].real_name.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn create_user_returns_id_from_response() {
        let mock = MockServer::start().await;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse>
              <params>
                <param>
                  <value>
                    <struct>
                      <member>
                        <name>id</name>
                        <value><int>4242</int></value>
                      </member>
                    </struct>
                  </value>
                </param>
              </params>
            </methodResponse>"#;
        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .and(body_string_contains("User.create"))
            .and(body_string_contains("alice@example.com"))
            .respond_with(ResponseTemplate::new(200).set_body_string(xml))
            .expect(1)
            .mount(&mock)
            .await;

        let client = XmlRpcClient::new(test_http_client(), &mock.uri(), "test-key");
        let params = CreateUserParams {
            email: "alice@example.com".into(),
            login: Some("alice".into()),
            full_name: Some("Alice Example".into()),
            password: Some("hunter2".into()),
        };
        let id = client.create_user(&params).await.unwrap();
        assert_eq!(id, 4242);
    }

    #[test]
    fn add_vec_filters_increments_chart_index_per_negation() {
        let params = SearchParams {
            product: vec!["!Bad".into(), "!Worse".into()],
            ..Default::default()
        };
        let mut rpc = BTreeMap::new();
        add_vec_filters(&mut rpc, &params);

        assert_eq!(rpc.get("f1").and_then(Value::as_str), Some("product"));
        assert_eq!(rpc.get("o1").and_then(Value::as_str), Some("notequals"));
        assert_eq!(rpc.get("v1").and_then(Value::as_str), Some("Bad"));
        assert_eq!(rpc.get("f2").and_then(Value::as_str), Some("product"));
        assert_eq!(rpc.get("o2").and_then(Value::as_str), Some("notequals"));
        assert_eq!(rpc.get("v2").and_then(Value::as_str), Some("Worse"));
    }

    #[test]
    fn get_nonempty_str_filters_empty_and_non_string() {
        let mut m = BTreeMap::new();
        m.insert("empty".into(), Value::String(String::new()));
        m.insert("filled".into(), Value::String("x".into()));
        m.insert("not_string".into(), Value::Int(5));
        assert!(get_nonempty_str(&m, "empty").is_none());
        assert_eq!(get_nonempty_str(&m, "filled").as_deref(), Some("x"));
        assert!(get_nonempty_str(&m, "not_string").is_none());
        assert!(get_nonempty_str(&m, "missing").is_none());
    }

    #[test]
    fn get_datetime_str_covers_datetime_string_and_fallthrough() {
        let mut m = BTreeMap::new();
        m.insert("dt".into(), Value::DateTime("2024-01-01T00:00:00".into()));
        m.insert("s_full".into(), Value::String("2024-02-02".into()));
        m.insert("s_empty".into(), Value::String(String::new()));
        m.insert("other".into(), Value::Int(42));
        assert_eq!(
            get_datetime_str(&m, "dt").as_deref(),
            Some("2024-01-01T00:00:00")
        );
        assert_eq!(
            get_datetime_str(&m, "s_full").as_deref(),
            Some("2024-02-02")
        );
        assert!(get_datetime_str(&m, "s_empty").is_none());
        assert!(get_datetime_str(&m, "other").is_none());
        assert!(get_datetime_str(&m, "missing").is_none());
    }

    #[test]
    fn get_str_array_returns_strings_only() {
        let mut m = BTreeMap::new();
        m.insert(
            "tags".into(),
            Value::Array(vec![
                Value::String("alpha".into()),
                Value::String("beta".into()),
                Value::Int(99),
            ]),
        );
        m.insert("not_array".into(), Value::String("oops".into()));
        assert_eq!(
            get_str_array(&m, "tags"),
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert!(get_str_array(&m, "not_array").is_empty());
        assert!(get_str_array(&m, "missing").is_empty());
    }

    #[test]
    fn get_int_array_returns_ints_only() {
        let mut m = BTreeMap::new();
        m.insert(
            "blocks".into(),
            Value::Array(vec![
                Value::Int(42),
                Value::Int(100),
                Value::String("nope".into()),
            ]),
        );
        m.insert("not_array".into(), Value::Int(5));
        assert_eq!(get_int_array(&m, "blocks"), vec![42_u64, 100]);
        assert!(get_int_array(&m, "not_array").is_empty());
        assert!(get_int_array(&m, "missing").is_empty());
    }
}
