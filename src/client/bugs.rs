use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::types::{
    ApiMode, Bug, CreateBugParams, HistoryEntry, SearchParams, UpdateBugParams,
};

/// Default fields requested for Bug queries. Matches the fields in [`Bug`] and
/// avoids requesting server-side fields we don't use — some Bugzilla extensions
/// crash when serializing certain fields (e.g. group visibility) via the REST API.
const BUG_DEFAULT_FIELDS: &str = "id,summary,status,resolution,product,component,\
    assigned_to,priority,severity,creation_time,last_change_time,creator,\
    url,whiteboard,keywords,blocks,depends_on,cc";

#[derive(Deserialize)]
struct BugListResponse {
    bugs: Vec<Bug>,
}

#[derive(Deserialize)]
struct HistoryResponse {
    bugs: Vec<HistoryBugEntry>,
}

#[derive(Deserialize)]
struct HistoryBugEntry {
    history: Vec<HistoryEntry>,
}

impl BugzillaClient {
    pub async fn get_bug_history_since(
        &self,
        id: u64,
        since: Option<&str>,
    ) -> Result<Vec<HistoryEntry>> {
        let mut req_builder = self.http.get(self.url(&format!("bug/{id}/history")));
        if let Some(since) = since {
            req_builder = req_builder.query(&[("new_since", since)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: HistoryResponse = self.parse_json(resp).await?;
        Ok(data
            .bugs
            .into_iter()
            .next()
            .map(|b| b.history)
            .unwrap_or_default())
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        tracing::debug!(?params, %self.api_mode, "search parameters");
        match self.api_mode {
            ApiMode::Rest => self.search_bugs_rest(params).await,
            ApiMode::XmlRpc => self.xmlrpc_client()?.search_bugs(params).await,
            ApiMode::Hybrid => {
                // Hybrid search only retries on empty results with active filters,
                // not on REST errors. Unlike get_bug (which retries on HTTP/parse
                // errors), search results are less critical and REST errors likely
                // indicate a server issue that XML-RPC won't solve either.
                let rest_result = self.search_bugs_rest(params).await;
                match rest_result {
                    Ok(ref bugs) if !bugs.is_empty() => rest_result,
                    Ok(_) if params.has_filters() => {
                        tracing::info!(
                            "REST search returned empty with active filters, \
                             retrying via XML-RPC"
                        );
                        self.xmlrpc_client()?.search_bugs(params).await
                    }
                    other => other,
                }
            }
        }
    }

    async fn search_bugs_rest(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let mut req_builder = self.http.get(self.url("bug")).query(params);
        // Vec fields can't be serialized by reqwest's query serializer, so we
        // append them manually as repeated query params (e.g. &id=1&id=2).
        for id in &params.id {
            req_builder = req_builder.query(&[("id", id)]);
        }
        if params.include_fields.is_none() {
            req_builder = req_builder.query(&[("include_fields", BUG_DEFAULT_FIELDS)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        Ok(data.bugs)
    }

    /// Fetch a single bug by numeric ID or alias string.
    ///
    /// Unlike `get_bug_history_since`, `get_comments_since`, and `get_attachments`,
    /// this method accepts `&str` because Bugzilla supports alias lookup here.
    /// The returned `Bug.id` (u64) can be passed to those numeric-only methods.
    pub async fn get_bug(
        &self,
        id: &str,
        include_fields: Option<&str>,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        match self.api_mode {
            ApiMode::XmlRpc => self.xmlrpc_client()?.get_bug(id).await,
            ApiMode::Hybrid => {
                let rest_result = self.get_bug_rest(id, include_fields, exclude_fields).await;
                match &rest_result {
                    Err(
                        BzrError::Http(_)
                        | BzrError::HttpStatus { .. }
                        | BzrError::Deserialize(_)
                        | BzrError::XmlRpc(_),
                    ) => {
                        tracing::info!("REST bug lookup failed, retrying via XML-RPC");
                        self.xmlrpc_client()?.get_bug(id).await
                    }
                    Err(BzrError::Api { code: 100_500, .. }) => {
                        // get_bug_rest() already retries 100500 via the search
                        // endpoint; this arm catches the case where the search
                        // endpoint also fails with 100500.
                        tracing::info!(
                            "REST bug lookup returned 100500, \
                             retrying via XML-RPC"
                        );
                        self.xmlrpc_client()?.get_bug(id).await
                    }
                    _ => rest_result,
                }
            }
            ApiMode::Rest => self.get_bug_rest(id, include_fields, exclude_fields).await,
        }
    }

    async fn get_bug_rest(
        &self,
        id: &str,
        include_fields: Option<&str>,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        let fields = include_fields.unwrap_or(BUG_DEFAULT_FIELDS);
        let mut req_builder = self
            .http
            .get(self.url(&format!("bug/{id}")))
            .query(&[("include_fields", fields)]);
        if let Some(fields) = exclude_fields {
            req_builder = req_builder.query(&[("exclude_fields", fields)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let result: Result<BugListResponse> = self.parse_json(resp).await;

        // If the direct endpoint fails with a server internal error (100500),
        // retry via the search endpoint (/rest/bug?id=X). Some Bugzilla
        // extensions only hook into the direct lookup path and crash there.
        if let Err(BzrError::Api { code: 100_500, .. }) = &result {
            tracing::debug!("direct bug lookup returned 100500, retrying via search endpoint");
            return self.get_bug_via_search(id, fields, exclude_fields).await;
        }

        result?
            .bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            })
    }

    async fn get_bug_via_search(
        &self,
        id: &str,
        include_fields: &str,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        let mut req_builder = self
            .http
            .get(self.url("bug"))
            .query(&[("id", id), ("include_fields", include_fields)]);
        if let Some(fields) = exclude_fields {
            req_builder = req_builder.query(&[("exclude_fields", fields)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        data.bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            })
    }

    pub async fn create_bug(&self, params: &CreateBugParams) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url("bug")).json(params));
        let resp = self.send(req).await?;
        let data: super::IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    pub async fn update_bug(&self, id: u64, updates: &UpdateBugParams) -> Result<()> {
        let req = self.apply_auth(self.http.put(self.url(&format!("bug/{id}"))).json(updates));
        self.send(req).await?;
        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::client::test_helpers::{test_client, test_client_hybrid};

    #[tokio::test]
    async fn get_bug_history_returns_entries() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "alias": [],
                    "history": [
                        {
                            "who": "alice@example.com",
                            "when": "2025-01-15T10:30:00Z",
                            "changes": [
                                {
                                    "field_name": "status",
                                    "removed": "NEW",
                                    "added": "ASSIGNED"
                                },
                                {
                                    "field_name": "assigned_to",
                                    "removed": "",
                                    "added": "alice@example.com"
                                }
                            ]
                        },
                        {
                            "who": "bob@example.com",
                            "when": "2025-01-16T14:00:00Z",
                            "changes": [
                                {
                                    "field_name": "status",
                                    "removed": "ASSIGNED",
                                    "added": "RESOLVED"
                                }
                            ]
                        }
                    ]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client.get_bug_history_since(42, None).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].who, "alice@example.com");
        assert_eq!(history[0].changes.len(), 2);
        assert_eq!(history[0].changes[0].field_name, "status");
        assert_eq!(history[0].changes[0].removed, "NEW");
        assert_eq!(history[0].changes[0].added, "ASSIGNED");
        assert_eq!(history[1].changes.len(), 1);
    }

    #[tokio::test]
    async fn get_bug_history_empty() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/99/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{"id": 99, "alias": [], "history": []}]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client.get_bug_history_since(99, None).await.unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn get_bug_history_with_attachment_id() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/10/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 10,
                    "alias": [],
                    "history": [{
                        "who": "carol@example.com",
                        "when": "2025-02-01T09:00:00Z",
                        "changes": [{
                            "field_name": "attachments.isobsolete",
                            "removed": "0",
                            "added": "1",
                            "attachment_id": 555
                        }]
                    }]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client.get_bug_history_since(10, None).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].changes[0].attachment_id, Some(555));
    }

    #[tokio::test]
    async fn get_bug_history_since_filters_by_date() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/history"))
            .and(query_param("new_since", "2025-06-01T00:00:00Z"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "alias": [],
                    "history": [{
                        "who": "alice@example.com",
                        "when": "2025-06-15T10:00:00Z",
                        "changes": [{"field_name": "status", "removed": "NEW", "added": "ASSIGNED"}]
                    }]
                }]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let history = client
            .get_bug_history_since(42, Some("2025-06-01T00:00:00Z"))
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn get_bug_passes_params() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/1"))
            .and(query_param("include_fields", "id,summary"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"bugs": [{"id": 1, "summary": "test", "status": "NEW"}]}),
            ))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let bug = client.get_bug("1", Some("id,summary"), None).await.unwrap();
        assert_eq!(bug.id, 1);
    }

    #[tokio::test]
    async fn get_bug_falls_back_on_100500() {
        let mock = MockServer::start().await;

        // Direct endpoint returns 100500 (server extension crash)
        Mock::given(method("GET"))
            .and(path("/rest/bug/99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 100_500,
                "message": "Extension crash"
            })))
            .mount(&mock)
            .await;

        // Search endpoint returns the bug successfully
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("id", "99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"bugs": [{"id": 99, "summary": "fallback bug", "status": "NEW"}]}),
            ))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let bug = client.get_bug("99", None, None).await.unwrap();
        assert_eq!(bug.id, 99);
        assert_eq!(bug.summary, "fallback bug");
    }

    #[test]
    fn search_params_serialization_product_only() {
        let params = SearchParams {
            product: Some("Product".into()),
            limit: Some(50),
            ..Default::default()
        };
        let qs = serde_urlencoded::to_string(&params).unwrap();
        assert!(qs.contains("product=Product"));
        assert!(qs.contains("limit=50"));
    }

    #[tokio::test]
    async fn search_bugs_sends_product_filter() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("product", "Product"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 217_630,
                    "summary": "Test bug",
                    "status": "WORKING",
                    "product": "Product",
                    "component": "Triage",
                    "assigned_to": "test@example.com",
                    "priority": "P1",
                    "severity": "high",
                    "creation_time": "2026-03-09T09:33:08Z",
                    "last_change_time": "2026-03-18T05:49:05Z"
                }]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let params = SearchParams {
            product: Some("Product".into()),
            limit: Some(50),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, 217_630);
    }

    fn xmlrpc_bug_response(id: i64, summary: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse><params><param><value><struct>
              <member><name>bugs</name><value><array><data>
                <value><struct>
                  <member><name>id</name><value><int>{id}</int></value></member>
                  <member><name>summary</name><value><string>{summary}</string></value></member>
                  <member><name>status</name><value><string>NEW</string></value></member>
                  <member><name>keywords</name><value><array><data></data></array></value></member>
                  <member><name>blocks</name><value><array><data></data></array></value></member>
                  <member><name>depends_on</name><value><array><data></data></array></value></member>
                  <member><name>cc</name><value><array><data></data></array></value></member>
                </struct></value>
              </data></array></value></member>
            </struct></value></param></params></methodResponse>"#
        )
    }

    #[tokio::test]
    async fn hybrid_search_rest_has_results_no_xmlrpc_call() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{"id": 1, "summary": "REST bug", "status": "NEW"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let params = SearchParams {
            product: Some("P".into()),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].summary, "REST bug");
    }

    #[tokio::test]
    async fn hybrid_search_rest_empty_with_filters_falls_back_to_xmlrpc() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(xmlrpc_bug_response(99, "XML-RPC bug")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let params = SearchParams {
            product: Some("P".into()),
            ..Default::default()
        };
        let bugs = client.search_bugs(&params).await.unwrap();
        assert_eq!(bugs.len(), 1);
        assert_eq!(bugs[0].id, 99);
        assert_eq!(bugs[0].summary, "XML-RPC bug");
    }

    #[tokio::test]
    async fn hybrid_search_rest_empty_without_filters_no_fallback() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let params = SearchParams::default();
        let bugs = client.search_bugs(&params).await.unwrap();
        assert!(bugs.is_empty());
    }

    #[tokio::test]
    async fn hybrid_get_bug_rest_500_falls_back_to_xmlrpc() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(500).set_body_string("error"))
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(xmlrpc_bug_response(42, "XML-RPC result")),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let bug = client.get_bug("42", None, None).await.unwrap();
        assert_eq!(bug.id, 42);
        assert_eq!(bug.summary, "XML-RPC result");
    }

    #[tokio::test]
    async fn hybrid_get_bug_rest_401_does_not_fall_back() {
        let mock = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 102,
                "message": "Invalid API key"
            })))
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/xmlrpc.cgi"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock)
            .await;

        let client = test_client_hybrid(&mock.uri());
        let err = client.get_bug("42", None, None).await.unwrap_err();
        assert!(
            err.to_string().contains("Invalid API key"),
            "should propagate auth error, got: {err}"
        );
    }

    #[test]
    fn search_params_has_filters() {
        let empty = SearchParams::default();
        assert!(!empty.has_filters());

        let with_product = SearchParams {
            product: Some("P".into()),
            ..Default::default()
        };
        assert!(with_product.has_filters());

        let with_quicksearch = SearchParams {
            quicksearch: Some("crash".into()),
            ..Default::default()
        };
        assert!(with_quicksearch.has_filters());
    }
}
