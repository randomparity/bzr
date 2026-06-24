//! Thin per-verb request helpers for [`BugzillaClient`] that compose the
//! transport (`apply_auth` + `send`) and response (`parse_*`) layers into
//! the GET/POST/PUT operations the per-resource modules call.

use crate::error::Result;

use super::{BugzillaClient, IdResponse};

impl BugzillaClient {
    /// Send a GET request and deserialize the JSON response.
    pub(super) async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let req = self.apply_auth(self.http.get(self.url(path)));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    /// Send a GET request with query parameters and deserialize the JSON response.
    pub(super) async fn get_json_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let req = self.apply_auth(self.http.get(self.url(path)).query(query));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    /// Send a POST request with a JSON body and return the created resource ID.
    pub(super) async fn post_json_id(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url(path)).json(body));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    /// Send a PUT request with a JSON body to a REST resource path.
    ///
    /// Inspects the response body for a Bugzilla HTTP-200 error envelope
    /// (`{"error":true,...}`) — some deployments report a rejected mutation
    /// with a 200 status, which a status-only check would treat as success.
    pub(super) async fn put_json(&self, path: &str, body: &impl serde::Serialize) -> Result<()> {
        let req = self.apply_auth(self.http.put(self.url(path)).json(body));
        let resp = self.send(req).await?;
        self.check_mutation_response(resp).await
    }

    /// Send a PUT request and deserialize the JSON response.
    pub(super) async fn put_json_response<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let req = self.apply_auth(self.http.put(self.url(path)).json(body));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    /// Send a GET request and return the parsed JSON body as a `Value`.
    pub(super) async fn get_json_value(&self, path: &str) -> Result<serde_json::Value> {
        let req = self.apply_auth(self.http.get(self.url(path)));
        let resp = self.send(req).await?;
        self.parse_json_value(resp).await
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
