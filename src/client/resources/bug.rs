use std::time::Duration;

use serde::Deserialize;

use crate::client::BugzillaClient;
use crate::error::{BzrError, Result, BUGZILLA_INTERNAL_ERROR};
use crate::http::XMLRPC_FALLBACK_TIMEOUT;
use crate::types::bug::{
    partition_filters, Bug, BugLinksNode, CreateBugParams, HistoryEntry, SearchParams,
    UpdateBugParams, BUG_SEARCH_DEFAULT_FIELDS, FIELD_MAPPINGS, LINKS_ID_CHUNK,
    LINKS_INCLUDE_FIELDS,
};
use crate::types::transport::ApiMode;

const BUG_VIEW_DEFAULT_FIELDS: &str =
    "id,summary,status,resolution,dupe_of,product,component,version,\
    assigned_to,priority,severity,creation_time,last_change_time,creator,\
    url,whiteboard,keywords,blocks,depends_on,cc,op_sys,platform,deadline,\
    target_milestone,groups,estimated_time,remaining_time,flags";

/// Ensure `id` is present in an include list and absent from an exclude list,
/// so the non-defaulted `Bug.id` always deserializes. `None` include is left
/// as-is (the search sinks inject `BUG_SEARCH_DEFAULT_FIELDS`, which leads with
/// `id`). Returns owned strings only when a change is needed.
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
struct BugLinksResponse {
    bugs: Vec<BugLinksNode>,
}

#[derive(Deserialize)]
struct BugListResponse {
    bugs: Vec<Bug>,
}

/// Re-surface the error that forced the search fallback, recording that the
/// fallback itself came back empty. The code is preserved so the Hybrid arm's
/// `Api { code: 100500 }` match still routes on to XML-RPC — it is control
/// flow two frames up, not just display text.
///
/// Only `Api` reaches the caller today (the sole call site matches on
/// `Api { code: BUGZILLA_INTERNAL_ERROR }`); the other arm keeps the function
/// total so a future caller cannot silently lose its error.
fn annotate_search_fallback(original: BzrError, id: &str) -> BzrError {
    match original {
        BzrError::Api { code, message } => BzrError::Api {
            code,
            message: format!(
                "{message} (direct lookup failed; the search fallback returned \
                 no row for bug {id} accessible to this account)"
            ),
        },
        other => other,
    }
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
        ("savedsearch", &params.saved_search),
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
    if let Some(sharer_id) = params.sharer_id {
        builder = builder.query(&[("sharer_id", sharer_id)]);
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

pub(crate) struct BugSearch<'a> {
    client: &'a BugzillaClient,
    force_rest: bool,
    warning_pending: bool,
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

    pub(crate) fn begin_bug_search(&self, params: &SearchParams) -> BugSearch<'_> {
        let force_rest = !params.raw_params.is_empty() && self.api_mode != ApiMode::Rest;
        BugSearch {
            client: self,
            force_rest,
            warning_pending: force_rest,
        }
    }

    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let mut search = self.begin_bug_search(params);
        search.execute(params).await
    }

    async fn search_bugs_configured(&self, params: &SearchParams) -> Result<Vec<Bug>> {
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
}

fn validate_role_negations(params: &SearchParams) -> Result<()> {
    let Some((flag, value)) = params.invalid_role_negation() else {
        return Ok(());
    };
    Err(BzrError::input(format!(
        "{flag} negation '{value}' must contain a role substring after '!'"
    )))
}

impl BugSearch<'_> {
    pub(crate) async fn execute(&mut self, params: &SearchParams) -> Result<Vec<Bug>> {
        validate_role_negations(params)?;
        tracing::debug!(?params, %self.client.api_mode, "search parameters");
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
        if self.warning_pending {
            tracing::warn!(
                "query contains raw URL parameters that require REST API; \
                 ignoring configured {} mode",
                self.client.api_mode
            );
            self.warning_pending = false;
        }
        if self.force_rest {
            self.client.search_bugs_rest(params).await
        } else {
            self.client.search_bugs_configured(params).await
        }
    }
}

impl BugzillaClient {
    async fn search_bugs_rest(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        if has_negated_filters(params) && has_raw_boolean_chart_params(params) {
            return Err(crate::error::BzrError::input(
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
            req_builder = req_builder.query(&[("include_fields", BUG_SEARCH_DEFAULT_FIELDS)]);
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
        let fields = include_fields.unwrap_or(BUG_VIEW_DEFAULT_FIELDS);
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
        // The original error travels with the retry: the search path filters
        // rows the caller cannot see into an empty result rather than
        // faulting, so an empty retry must not be reported as "not found".
        let data = match result {
            Err(
                original @ BzrError::Api {
                    code: BUGZILLA_INTERNAL_ERROR,
                    ..
                },
            ) => {
                tracing::debug!("direct bug lookup returned 100500, retrying via search endpoint");
                return self
                    .get_bug_via_search(id, fields, exclude_fields, original)
                    .await;
            }
            other => other?,
        };

        data.bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            })
    }

    /// Retry a failed direct lookup through the search endpoint.
    ///
    /// `original` is the error that forced the retry. When the search returns
    /// no row it is re-surfaced instead of `NotFound`: Bugzilla's search path
    /// omits bugs the caller cannot see rather than faulting, so an empty
    /// result here means "the fallback could not answer", not "no such bug"
    /// (issue #504, ADR 0015).
    async fn get_bug_via_search(
        &self,
        id: &str,
        include_fields: &str,
        exclude_fields: Option<&str>,
        original: BzrError,
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
        data.bugs.into_iter().next().ok_or_else(|| {
            tracing::debug!("search fallback returned no accessible row; surfacing original error");
            annotate_search_fallback(original, id)
        })
    }

    /// Fetch the isolated link node for a graph's **root** id.
    ///
    /// Unlike [`Self::get_bug_links_nodes`], this reads Bugzilla's direct
    /// endpoint, which faults on a bug the caller may not see. The search
    /// endpoint that batches the related ids answers an inaccessible id with a
    /// filtered 200 carrying no error at all, so a root read there could not
    /// tell "omitted because invisible" from "omitted because absent" and
    /// reported a permission denial as `NotFound` (issue #719). ADR 0015
    /// reserves `NotFound` for the direct path returning an empty result with
    /// no error payload, which is the one case still mapped to it here.
    ///
    /// This reads the same direct endpoint as [`Self::get_bug`] but does not
    /// call it, and the difference is load-bearing rather than incidental:
    /// `get_bug` yields a [`Bug`], and the only way to a `BugLinksNode` from
    /// there is `BugLinksNode::from_bug`, which can express just the three core
    /// relations and leaves `duplicates`, `regressed_by`, and `regressions`
    /// empty. Those are the BMO relations, and this node seeds the traversal —
    /// so routing through `get_bug` would silently truncate the root's adjacency
    /// on Red Hat and Mozilla deployments. Deserializing `BugLinksResponse`
    /// keeps all six. Do not "simplify" this into a `get_bug` call (ADR 0060).
    ///
    /// That warning is about the **REST** arm only. The `XmlRpc` arm below does
    /// go through `from_bug` and does accept the truncation, because XML-RPC
    /// answers with a `Bug`, which has no field for the three BMO relations in
    /// the first place — there is nothing there to lose. So the two arms can
    /// disagree on a root's BMO adjacency on a deployment that populates those
    /// fields, and that predates this change.
    pub(crate) async fn get_bug_links_root_node(&self, id: u64) -> Result<BugLinksNode> {
        match self.api_mode {
            ApiMode::XmlRpc => self
                .xmlrpc_client()
                .get_bug(&id.to_string())
                .await
                .map(|bug| BugLinksNode::from_bug(&bug)),
            ApiMode::Rest | ApiMode::Hybrid => self.get_bug_links_root_node_rest(id).await,
        }
    }

    async fn get_bug_links_root_node_rest(&self, id: u64) -> Result<BugLinksNode> {
        let req_builder = self
            .http
            .get(self.url(&format!("bug/{id}")))
            .query(&[("include_fields", LINKS_INCLUDE_FIELDS)]);
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;

        // The direct endpoint is the one some Bugzilla extensions hook and
        // crash on with 100500, which is why `get_bug_rest` retries through
        // search. Moving the root read here carries that retry with it, or
        // `bug links` stops working on those deployments while the related-id
        // half of the walk keeps succeeding. The original error travels with
        // the retry: search omits rows the caller cannot see, so an empty
        // retry is "the fallback could not answer", never `NotFound`
        // (issue #504, ADR 0015).
        let data: BugLinksResponse = match self.parse_json(resp).await {
            Err(
                original @ BzrError::Api {
                    code: BUGZILLA_INTERNAL_ERROR,
                    ..
                },
            ) => {
                tracing::debug!(
                    "direct links root lookup returned 100500, retrying via search endpoint"
                );
                return self
                    .get_bug_links_nodes_rest(&[id])
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| annotate_search_fallback(original, &id.to_string()));
            }
            other => other?,
        };

        data.bugs
            .into_iter()
            .next()
            .ok_or_else(|| BzrError::NotFound {
                resource: "bug",
                id: id.to_string(),
            })
    }

    /// Fetch isolated link nodes for `ids`. Inaccessible/nonexistent ids are
    /// omitted from the result; the caller decides whether an omission is fatal
    /// (root not found) or skippable (a related bug). REST/Hybrid batch the
    /// request in `LINKS_ID_CHUNK`-sized chunks; XML-RPC fetches one id per call.
    pub(crate) async fn get_bug_links_nodes(&self, ids: &[u64]) -> Result<Vec<BugLinksNode>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        match self.api_mode {
            ApiMode::XmlRpc => {
                let mut nodes = Vec::with_capacity(ids.len());
                for &id in ids {
                    match self.xmlrpc_client().get_bug(&id.to_string()).await {
                        Ok(bug) => nodes.push(BugLinksNode::from_bug(&bug)),
                        Err(BzrError::NotFound { .. }) => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(nodes)
            }
            ApiMode::Rest | ApiMode::Hybrid => self.get_bug_links_nodes_rest(ids).await,
        }
    }

    async fn get_bug_links_nodes_rest(&self, ids: &[u64]) -> Result<Vec<BugLinksNode>> {
        let mut nodes = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(LINKS_ID_CHUNK) {
            let mut req_builder = self.http.get(self.url("bug"));
            for &id in chunk {
                req_builder = req_builder.query(&[("id", id)]);
            }
            req_builder = req_builder.query(&[("include_fields", LINKS_INCLUDE_FIELDS)]);
            let req = self.apply_auth(req_builder);
            let resp = self.send(req).await?;
            let data: BugLinksResponse = self.parse_json(resp).await?;
            nodes.extend(data.bugs);
        }
        Ok(nodes)
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
