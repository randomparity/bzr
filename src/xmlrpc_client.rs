use std::collections::BTreeMap;

use crate::error::{BzrError, Result};
use crate::types::{Bug, SearchParams};
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
    async fn call(&self, method: &str, mut params: BTreeMap<String, Value>) -> Result<Value> {
        params.insert(
            "Bugzilla_api_key".into(),
            Value::from(self.api_key.as_str()),
        );

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
            tracing::debug!(%status, "XML-RPC HTTP error");
            return Err(BzrError::XmlRpc(format!("HTTP {status}")));
        }

        let body_text = resp.text().await?;
        tracing::trace!(body_len = body_text.len(), "XML-RPC response received");

        xmlrpc::parse_response(&body_text)
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let mut rpc_params = BTreeMap::new();

        // Map string filter fields to XML-RPC params.
        let string_fields: &[(&str, &Option<String>)] = &[
            ("product", &params.product),
            ("component", &params.component),
            ("status", &params.status),
            ("assigned_to", &params.assigned_to),
            ("creator", &params.creator),
            ("priority", &params.priority),
            ("severity", &params.severity),
            ("alias", &params.alias),
            ("summary", &params.summary),
            ("quicksearch", &params.quicksearch),
        ];
        for &(key, value) in string_fields {
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
        // Comma-separated field lists become XML-RPC arrays.
        for (key, value) in [
            ("include_fields", &params.include_fields),
            ("exclude_fields", &params.exclude_fields),
        ] {
            if let Some(ref fields) = *value {
                let arr: Vec<Value> = fields.split(',').map(|f| Value::from(f.trim())).collect();
                rpc_params.insert(key.into(), Value::Array(arr));
            }
        }

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
        resolution: get_str_opt(m, "resolution"),
        product: get_str_opt(m, "product"),
        component: get_str_opt(m, "component"),
        assigned_to: get_str_opt(m, "assigned_to"),
        priority: get_str_opt(m, "priority"),
        severity: get_str_opt(m, "severity"),
        creation_time: get_datetime_str(m, "creation_time"),
        last_change_time: get_datetime_str(m, "last_change_time"),
        creator: get_str_opt(m, "creator"),
        url: get_str_opt(m, "url"),
        whiteboard: get_str_opt(m, "whiteboard"),
        keywords: get_str_array(m, "keywords"),
        blocks: get_int_array(m, "blocks"),
        depends_on: get_int_array(m, "depends_on"),
        cc: get_str_array(m, "cc"),
    })
}

fn get_str(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(String::from)
}

fn get_str_opt(m: &BTreeMap<String, Value>, key: &str) -> Option<String> {
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

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn test_http_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    fn xmlrpc_bug_response(id: i64, summary: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse>
              <params>
                <param>
                  <value>
                    <struct>
                      <member>
                        <name>bugs</name>
                        <value>
                          <array>
                            <data>
                              <value>
                                <struct>
                                  <member>
                                    <name>id</name>
                                    <value><int>{id}</int></value>
                                  </member>
                                  <member>
                                    <name>summary</name>
                                    <value><string>{summary}</string></value>
                                  </member>
                                  <member>
                                    <name>status</name>
                                    <value><string>NEW</string></value>
                                  </member>
                                  <member>
                                    <name>product</name>
                                    <value><string>TestProduct</string></value>
                                  </member>
                                  <member>
                                    <name>component</name>
                                    <value><string>General</string></value>
                                  </member>
                                  <member>
                                    <name>priority</name>
                                    <value><string>P1</string></value>
                                  </member>
                                  <member>
                                    <name>severity</name>
                                    <value><string>normal</string></value>
                                  </member>
                                  <member>
                                    <name>assigned_to</name>
                                    <value><string>user@example.com</string></value>
                                  </member>
                                  <member>
                                    <name>keywords</name>
                                    <value><array><data></data></array></value>
                                  </member>
                                  <member>
                                    <name>blocks</name>
                                    <value><array><data></data></array></value>
                                  </member>
                                  <member>
                                    <name>depends_on</name>
                                    <value><array><data></data></array></value>
                                  </member>
                                  <member>
                                    <name>cc</name>
                                    <value><array><data></data></array></value>
                                  </member>
                                </struct>
                              </value>
                            </data>
                          </array>
                        </value>
                      </member>
                    </struct>
                  </value>
                </param>
              </params>
            </methodResponse>"#
        )
    }

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
            product: Some("TestProduct".into()),
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
            product: Some("Empty".into()),
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
}
