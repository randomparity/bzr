use std::time::Duration;

use serde::Deserialize;

use super::BugzillaClient;
use crate::error::{BzrError, Result, BUGZILLA_INTERNAL_ERROR};
use crate::http::XMLRPC_FALLBACK_TIMEOUT;
use crate::types::bug::{
    partition_filters, Bug, CreateBugParams, HistoryEntry, SearchParams, UpdateBugParams,
    FIELD_MAPPINGS,
};
use crate::types::transport::ApiMode;

/// Default fields requested for Bug queries. Matches the fields in [`Bug`] and
/// avoids requesting server-side fields we don't use — some Bugzilla extensions
/// crash when serializing certain fields (e.g. group visibility) via the REST API.
const BUG_DEFAULT_FIELDS: &str = "id,summary,status,resolution,dupe_of,product,component,version,\
    assigned_to,priority,severity,creation_time,last_change_time,creator,\
    url,whiteboard,keywords,blocks,depends_on,cc,op_sys,rep_platform,deadline,\
    target_milestone,flags";

/// Ensure `id` is present in an include list and absent from an exclude list,
/// so the non-defaulted `Bug.id` always deserializes. `None` include is left
/// as-is (the REST sink injects `BUG_DEFAULT_FIELDS`, which leads with `id`;
/// XML-RPC returns `id` by server default). Returns owned strings only when a
/// change is needed.
fn force_id_fields(
    include: Option<&str>,
    exclude: Option<&str>,
) -> (Option<String>, Option<String>) {
    let has_id = |list: &str| list.split(',').any(|t| t.trim().eq_ignore_ascii_case("id"));

    let include = include.map(|list| {
        if has_id(list) {
            list.to_string()
        } else {
            format!("id,{list}")
        }
    });

    let exclude = exclude.and_then(|list| {
        if !has_id(list) {
            return Some(list.to_string());
        }
        let kept: Vec<&str> = list
            .split(',')
            .filter(|t| !t.trim().eq_ignore_ascii_case("id"))
            .collect();
        if kept.is_empty() {
            None
        } else {
            Some(kept.join(","))
        }
    });

    (include, exclude)
}

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

/// Appends positive (non-negated) values from multi-value `SearchParams`
/// fields as repeated query params (e.g. `&status=NEW&status=ASSIGNED`).
fn append_multi_value_params(
    mut builder: reqwest::RequestBuilder,
    params: &SearchParams,
) -> reqwest::RequestBuilder {
    for mapping in FIELD_MAPPINGS {
        let (positive, _) = partition_filters(params.get_field(mapping.field));
        for v in positive {
            builder = builder.query(&[(mapping.struct_field, v)]);
        }
    }
    builder
}

/// Appends negated values (prefixed with `!`) as Bugzilla boolean chart
/// parameters (`fN`, `oN`, `vN` triples with `notequals` operator).
///
/// Multiple negated values on the same field each get their own triple and
/// are combined with AND (Bugzilla default when no `j_top` join is set).
/// E.g. `--status '!CLOSED' --status '!VERIFIED'` produces
/// `f1=bug_status&o1=notequals&v1=CLOSED&f2=bug_status&o2=notequals&v2=VERIFIED`,
/// meaning "status != CLOSED AND status != VERIFIED" — the desired behavior.
fn append_negated_params(
    mut builder: reqwest::RequestBuilder,
    params: &SearchParams,
) -> reqwest::RequestBuilder {
    let mut idx = 1u32;
    for mapping in FIELD_MAPPINGS {
        let (_, negated) = partition_filters(params.get_field(mapping.field));
        for v in negated {
            let f_key = format!("f{idx}");
            let o_key = format!("o{idx}");
            let v_key = format!("v{idx}");
            builder = builder.query(&[
                (&f_key, mapping.internal_name),
                (&o_key, mapping.negation_operator.as_str()),
                (&v_key, v),
            ]);
            idx += 1;
        }
    }
    builder
}

/// Appends the remaining single-value `Option` and scalar fields from
/// `SearchParams` as query parameters. These were previously handled by
/// serde `Serialize` on the struct; now all query encoding is explicit.
fn append_option_params(
    mut builder: reqwest::RequestBuilder,
    params: &SearchParams,
) -> reqwest::RequestBuilder {
    let option_fields: &[(&str, &Option<String>)] = &[
        ("cc", &params.cc),
        ("alias", &params.alias),
        ("summary", &params.summary),
        ("quicksearch", &params.quicksearch),
        ("include_fields", &params.include_fields),
        ("exclude_fields", &params.exclude_fields),
        ("creation_time", &params.creation_time),
        ("last_change_time", &params.last_change_time),
        ("order", &params.order),
    ];
    for &(key, value) in option_fields {
        if let Some(v) = value {
            builder = builder.query(&[(key, v.as_str())]);
        }
    }
    if let Some(limit) = params.limit {
        builder = builder.query(&[("limit", limit)]);
    }
    if let Some(offset) = params.offset {
        builder = builder.query(&[("offset", offset)]);
    }
    builder
}

/// Returns true if any multi-value filter field contains negated values (prefixed with `!`).
fn has_negated_filters(params: &SearchParams) -> bool {
    FIELD_MAPPINGS
        .iter()
        .any(|m| params.get_field(m.field).iter().any(|v| v.starts_with('!')))
}

/// Returns true if `raw_params` contains boolean chart parameters (`fN`, `oN`, `vN`
/// where N is a positive integer).
fn has_raw_boolean_chart_params(params: &SearchParams) -> bool {
    params.raw_params.iter().any(|(k, _)| {
        k.len() >= 2
            && matches!(k.as_bytes()[0], b'f' | b'o' | b'v')
            && k[1..].parse::<u32>().is_ok_and(|n| n >= 1)
    })
}

/// Appends raw key-value parameters to the request builder verbatim.
/// Used for URL-imported queries with boolean chart params that
/// `bzr` does not natively model.
fn append_raw_params(
    builder: reqwest::RequestBuilder,
    raw_params: &[(String, String)],
) -> reqwest::RequestBuilder {
    builder.query(raw_params)
}

impl BugzillaClient {
    pub async fn get_bug_history_since(
        &self,
        bug_id: u64,
        since: Option<&str>,
    ) -> Result<Vec<HistoryEntry>> {
        let data: HistoryResponse = if let Some(since) = since {
            self.get_json_query(&format!("bug/{bug_id}/history"), &[("new_since", since)])
                .await?
        } else {
            self.get_json(&format!("bug/{bug_id}/history")).await?
        };
        let history = data
            .bugs
            .into_iter()
            .next()
            .map_or_else(Vec::new, |b| b.history);
        Ok(history)
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        tracing::debug!(?params, %self.api_mode, "search parameters");
        // Guarantee `id` is fetched so the non-defaulted `Bug.id` deserializes,
        // even when the caller passed an id-less `--fields`. Only clone when
        // normalization actually changed something, keeping the common
        // no-`--fields` path allocation-free.
        let (inc, exc) = force_id_fields(
            params.include_fields.as_deref(),
            params.exclude_fields.as_deref(),
        );
        let normalized =
            (inc != params.include_fields || exc != params.exclude_fields).then(|| SearchParams {
                include_fields: inc,
                exclude_fields: exc,
                ..params.clone()
            });
        let params = normalized.as_ref().unwrap_or(params);
        // Raw params (boolean charts from URLs) only work with REST.
        if !params.raw_params.is_empty() && self.api_mode != ApiMode::Rest {
            tracing::warn!(
                "query contains raw URL parameters that require REST API; \
                 ignoring configured {} mode",
                self.api_mode
            );
            return self.search_bugs_rest(params).await;
        }
        match self.api_mode {
            ApiMode::Rest => self.search_bugs_rest(params).await,
            ApiMode::XmlRpc => self.xmlrpc_client().search_bugs(params).await,
            ApiMode::Hybrid => {
                self.search_bugs_hybrid(params, XMLRPC_FALLBACK_TIMEOUT)
                    .await
            }
        }
    }

    /// Hybrid-mode search: try REST, fall back to XML-RPC only when the REST
    /// result is empty AND structured filters are present.
    ///
    /// The retry exists to paper over Bugzilla extensions whose REST handlers
    /// misbehave for structured filters but whose XML-RPC handlers do not.
    /// Free-text predicates (quicksearch, summary) are evaluated by the same
    /// server-side parser regardless of transport, so empty results for those
    /// are authoritative and skip the retry (issue #152).
    ///
    /// The XML-RPC retry is capped at `fallback_timeout`; on cap-out the empty
    /// REST result is returned and a warning is logged. The cap is
    /// parameterized so tests can supply a short value without slowing the
    /// suite. Production callers pass [`XMLRPC_FALLBACK_TIMEOUT`].
    pub(crate) async fn search_bugs_hybrid(
        &self,
        params: &SearchParams,
        fallback_timeout: Duration,
    ) -> Result<Vec<Bug>> {
        let rest_bugs = self.search_bugs_rest(params).await?;
        if !rest_bugs.is_empty() || !params.has_structured_filters() {
            return Ok(rest_bugs);
        }
        tracing::info!(
            "REST search returned empty with active structured filters, \
             retrying via XML-RPC"
        );
        let xmlrpc = self.xmlrpc_client();
        if let Ok(result) = tokio::time::timeout(fallback_timeout, xmlrpc.search_bugs(params)).await
        {
            result
        } else {
            tracing::warn!(
                "XML-RPC search fallback timed out after {}s — returning the \
                 empty REST result. To skip future fallbacks for this server, \
                 pass --api rest or set api_mode = \"rest\" in config.",
                fallback_timeout.as_secs()
            );
            Ok(rest_bugs)
        }
    }

    async fn search_bugs_rest(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        if has_negated_filters(params) && has_raw_boolean_chart_params(params) {
            return Err(crate::error::BzrError::InputValidation(
                "cannot combine negated filters (e.g. --status '!CLOSED') with a \
                 URL-imported query containing boolean chart parameters; the chart \
                 indices would collide"
                    .into(),
            ));
        }

        let mut req_builder = self.http.get(self.url("bug"));
        req_builder = append_multi_value_params(req_builder, params);
        req_builder = append_negated_params(req_builder, params);
        req_builder = append_option_params(req_builder, params);

        for id in &params.id {
            req_builder = req_builder.query(&[("id", id)]);
        }

        req_builder = append_raw_params(req_builder, &params.raw_params);

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
    ///
    /// In Hybrid mode, the retry chain is: REST direct → REST search (on 100500)
    /// → XML-RPC. The first two steps happen inside `get_bug_rest`; the XML-RPC
    /// fallback here catches transport failures and residual 100500 errors.
    pub async fn get_bug(
        &self,
        id: &str,
        include_fields: Option<&str>,
        exclude_fields: Option<&str>,
    ) -> Result<Bug> {
        // Guarantee `id` is fetched so the non-defaulted `Bug.id` deserializes.
        // XML-RPC ignores field lists, so this is a no-op there.
        let (inc, exc) = force_id_fields(include_fields, exclude_fields);
        let (include_fields, exclude_fields) = (inc.as_deref(), exc.as_deref());
        match self.api_mode {
            ApiMode::XmlRpc => self.xmlrpc_client().get_bug(id).await,
            ApiMode::Hybrid => {
                let rest_result = self.get_bug_rest(id, include_fields, exclude_fields).await;
                match &rest_result {
                    Err(e) if e.is_transport_failure() => {
                        tracing::info!("REST bug lookup failed, retrying via XML-RPC");
                        self.xmlrpc_client().get_bug(id).await
                    }
                    Err(BzrError::Api {
                        code: BUGZILLA_INTERNAL_ERROR,
                        ..
                    }) => {
                        // get_bug_rest() already retries 100500 via the search
                        // endpoint; this arm catches the case where the search
                        // endpoint also fails with 100500.
                        tracing::info!(
                            "REST bug lookup returned 100500, \
                             retrying via XML-RPC"
                        );
                        self.xmlrpc_client().get_bug(id).await
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
        if let Err(BzrError::Api {
            code: BUGZILLA_INTERNAL_ERROR,
            ..
        }) = &result
        {
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

    /// Create a new bug. Always uses REST (XML-RPC mutation support is not implemented).
    pub async fn create_bug(&self, params: &CreateBugParams) -> Result<u64> {
        self.post_json_id("bug", params).await
    }

    /// Update a bug. Always uses REST (XML-RPC mutation support is not implemented).
    pub async fn update_bug(&self, id: u64, updates: &UpdateBugParams) -> Result<()> {
        self.put_json(&format!("bug/{id}"), updates).await
    }
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
