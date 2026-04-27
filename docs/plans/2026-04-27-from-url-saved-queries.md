# `--from-url` and Saved Query Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to pass Bugzilla GUI URLs to `bzr` via `--from-url`, execute them, and optionally save them as named queries for reuse.

**Architecture:** A new `url_parser` module parses `buglist.cgi` URLs into the existing `SavedQuery` struct (extended with `source_url`, `server`, and `raw_params` fields). Raw boolean chart parameters pass through to the REST API verbatim. CLI changes add `--from-url`/`--save-as` to `bug search` and `--from-url` to `query save`.

**Tech Stack:** Rust, clap (CLI), url crate (parsing), reqwest (HTTP), wiremock (testing), serde/TOML (config)

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `Cargo.toml` | Modify | Add `url` as direct dependency |
| `src/types/bug.rs` | Modify | Extend `QueryKind`, `SavedQuery`, `SearchParams` with new fields |
| `src/types/mod.rs` | Modify | Re-export new items if needed |
| `src/url_parser.rs` | Create | Parse `buglist.cgi` URLs into `SavedQuery` |
| `src/lib.rs` | Modify | Add `pub mod url_parser` |
| `src/client/bug.rs` | Modify | Add `append_raw_params`, call it in `search_bugs_rest`, force REST for raw params |
| `src/cli/bug.rs` | Modify | Add `--from-url` and `--save-as` to `Search` variant |
| `src/cli/query.rs` | Modify | Add `--from-url` to `Save`, `--server` to `Run` |
| `src/commands/bug.rs` | Modify | Handle `--from-url` path in `handle_search` |
| `src/commands/query.rs` | Modify | Handle `--from-url` in `handle_save`, pass raw params in `handle_run`, use server override |
| `src/output/query.rs` | Modify | Display `source_url`, `server`, raw param count in `query show` |
| `docs/bzr-cli.md` | Modify | Document new flags |

---

### Task 1: Add `url` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add the `url` crate to `[dependencies]`**

In `Cargo.toml`, add under `[dependencies]`:

```toml
url = "2"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add url crate as direct dependency for URL parsing"
```

---

### Task 2: Extend data model — `QueryKind`, `SavedQuery`, `SearchParams`

**Files:**
- Modify: `src/types/bug.rs:242-312`

- [ ] **Step 1: Write tests for new `QueryKind::Url` variant**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `src/types/bug.rs` (alongside the existing `saved_query_*` tests):

```rust
#[test]
fn query_kind_url_serializes() {
    let json = serde_json::to_string(&QueryKind::Url).unwrap();
    assert_eq!(json, r#""url""#);
}

#[test]
fn query_kind_url_deserializes() {
    let kind: QueryKind = serde_json::from_str(r#""url""#).unwrap();
    assert_eq!(kind, QueryKind::Url);
}

#[test]
fn saved_query_with_url_fields_roundtrips() {
    let query = SavedQuery {
        kind: QueryKind::Url,
        source_url: Some("https://bugzilla.example.com/buglist.cgi?product=Firefox".into()),
        server: Some("example".into()),
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "changedfrom".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        product: vec!["Firefox".into()],
        ..SavedQuery::default()
    };
    let json = serde_json::to_string(&query).unwrap();
    let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.kind, QueryKind::Url);
    assert_eq!(
        roundtripped.source_url.as_deref(),
        Some("https://bugzilla.example.com/buglist.cgi?product=Firefox")
    );
    assert_eq!(roundtripped.server.as_deref(), Some("example"));
    assert_eq!(roundtripped.raw_params.len(), 3);
    assert_eq!(roundtripped.raw_params[0], ("f1".into(), "qa_contact".into()));
    assert_eq!(roundtripped.product, vec!["Firefox"]);
}

#[test]
fn saved_query_without_url_fields_omits_them_in_json() {
    let query = SavedQuery {
        kind: QueryKind::List,
        product: vec!["Firefox".into()],
        ..SavedQuery::default()
    };
    let json = serde_json::to_string(&query).unwrap();
    assert!(!json.contains("source_url"));
    assert!(!json.contains("server"));
    assert!(!json.contains("raw_params"));
}

#[test]
fn saved_query_url_kind_to_search_params_includes_raw_params() {
    let query = SavedQuery {
        kind: QueryKind::Url,
        product: vec!["Firefox".into()],
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "changedfrom".into()),
        ],
        limit: Some(100),
        ..SavedQuery::default()
    };
    let params = query.to_search_params();
    assert_eq!(params.product, vec!["Firefox"]);
    assert_eq!(params.limit, Some(100));
    assert_eq!(params.raw_params.len(), 2);
    assert_eq!(params.raw_params[0], ("f1".into(), "qa_contact".into()));
}

#[test]
fn saved_query_url_kind_has_filters_with_only_raw_params() {
    let query = SavedQuery {
        kind: QueryKind::Url,
        raw_params: vec![("f1".into(), "qa_contact".into())],
        ..SavedQuery::default()
    };
    assert!(query.has_filters());
}

#[test]
fn search_params_has_filters_with_raw_params() {
    let params = SearchParams {
        raw_params: vec![("f1".into(), "qa_contact".into())],
        ..Default::default()
    };
    assert!(params.has_filters());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib types::bug::tests::query_kind_url -- --no-capture 2>&1 | head -20`
Expected: compilation errors (fields/variant don't exist yet)

- [ ] **Step 3: Add `Url` variant to `QueryKind`**

In `src/types/bug.rs`, add the variant to the `QueryKind` enum (after `Search`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QueryKind {
    #[default]
    List,
    Search,
    /// Query imported from a Bugzilla URL (may contain raw passthrough params)
    Url,
}
```

- [ ] **Step 4: Add new fields to `SavedQuery`**

In `src/types/bug.rs`, add these fields to the `SavedQuery` struct after `exclude_fields`:

```rust
    /// The original Bugzilla URL this query was parsed from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Server name (from config) this query is associated with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Raw query parameters not mapped to structured fields.
    /// Passed through verbatim to the Bugzilla REST API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_params: Vec<(String, String)>,
```

- [ ] **Step 5: Add `raw_params` field to `SearchParams`**

In `src/types/bug.rs`, add this field to `SearchParams` after `exclude_fields`:

```rust
    /// Raw query parameters passed through verbatim to the REST API.
    /// Used for URL-imported queries with boolean chart params.
    pub raw_params: Vec<(String, String)>,
```

- [ ] **Step 6: Update `SavedQuery::to_search_params` to include `raw_params`**

In the `to_search_params` method, add `raw_params` to the constructed `SearchParams`:

```rust
    pub fn to_search_params(&self) -> SearchParams {
        SearchParams {
            product: self.product.clone(),
            component: self.component.clone(),
            status: self.status.clone(),
            assigned_to: self.assignee.clone(),
            creator: self.creator.clone(),
            priority: self.priority.clone(),
            severity: self.severity.clone(),
            quicksearch: self.quicksearch.clone(),
            limit: self.limit,
            include_fields: self.fields.clone(),
            exclude_fields: self.exclude_fields.clone(),
            raw_params: self.raw_params.clone(),
            ..Default::default()
        }
    }
```

- [ ] **Step 7: Update `SavedQuery::has_filters` to check `raw_params`**

Add `|| !self.raw_params.is_empty()` to the condition chain in `has_filters()`.

- [ ] **Step 8: Update `SearchParams::has_filters` to check `raw_params`**

Add `|| !self.raw_params.is_empty()` to the condition chain in `has_filters()`.

- [ ] **Step 9: Update `output/query.rs` — add `Url` arm to match expressions**

In `src/output/query.rs`, find the two `match q.kind` / `match view.query.kind` expressions and add:

```rust
QueryKind::Url => "url",
```

- [ ] **Step 10: Run all tests**

Run: `cargo test`
Expected: all tests pass (including the new ones and existing ones that were unaffected by `..Default::default()`)

- [ ] **Step 11: Commit**

```bash
git add src/types/bug.rs src/output/query.rs
git commit -m "feat: extend SavedQuery and SearchParams with raw_params, source_url, server fields"
```

---

### Task 3: Create URL parser module

**Files:**
- Create: `src/url_parser.rs`
- Modify: `src/lib.rs:21` (add module declaration)

- [ ] **Step 1: Create `src/url_parser.rs` with tests first**

Create the file with the module doc, imports, the public function signature that returns `Err` (to make tests compile but fail), and the test module:

```rust
//! Parse Bugzilla `buglist.cgi` URLs into `SavedQuery` structs.

use url::Url;

use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::types::{QueryKind, SavedQuery};

/// Parameters ignored during URL parsing (display/session metadata).
const IGNORED_PARAMS: &[&str] = &["columnlist", "list_id", "query_format"];

/// Parameters extracted as the suggested query name, not stored as filters.
const NAME_PARAMS: &[&str] = &["known_name", "query_based_on"];

/// Maps Bugzilla URL parameter names to `SavedQuery` vec field names.
/// Each entry is (url_param, field_name) where field_name matches
/// the `SavedQuery` struct field.
const RECOGNIZED_VEC_PARAMS: &[(&str, &str)] = &[
    ("product", "product"),
    ("component", "component"),
    ("bug_status", "status"),
    ("assigned_to", "assignee"),
    ("reporter", "creator"),
    ("priority", "priority"),
    ("bug_severity", "severity"),
];

/// Result of parsing a Bugzilla URL.
pub struct ParsedUrl {
    /// The parsed query ready for storage.
    pub query: SavedQuery,
    /// Suggested name extracted from `known_name`/`query_based_on` params.
    pub suggested_name: Option<String>,
}

/// Parse a Bugzilla `buglist.cgi` URL into a `SavedQuery`.
///
/// Recognized parameters are mapped to structured `SavedQuery` fields.
/// Unrecognized parameters are stored in `raw_params` for verbatim
/// passthrough to the REST API. Display/session params are ignored.
///
/// The `config` is used to match the URL hostname against configured
/// servers.
pub fn parse_bugzilla_url(url_str: &str, config: &Config) -> Result<ParsedUrl> {
    let url = Url::parse(url_str)
        .map_err(|e| BzrError::InputValidation(format!("invalid URL: {e}")))?;

    // Verify this is a buglist.cgi URL
    if !url.path().contains("buglist.cgi") {
        return Err(BzrError::InputValidation(
            "URL must be a Bugzilla buglist.cgi URL".into(),
        ));
    }

    // Match hostname against configured servers
    let url_host = url
        .host_str()
        .ok_or_else(|| BzrError::InputValidation("URL has no hostname".into()))?;

    let server = find_server_by_hostname(config, url_host);
    if server.is_none() && config.default_server.is_none() {
        return Err(BzrError::config(format!(
            "URL hostname '{url_host}' does not match any configured server \
             and no default server is set. Run `bzr config set-server` first."
        )));
    }
    if server.is_none() {
        tracing::warn!(
            "URL hostname '{url_host}' does not match any configured server; \
             using default server"
        );
    }

    let mut query = SavedQuery {
        kind: QueryKind::Url,
        source_url: Some(url_str.to_string()),
        server: server.map(String::from),
        ..SavedQuery::default()
    };

    let mut suggested_name: Option<String> = None;

    for (key, value) in url.query_pairs() {
        let key = key.as_ref();
        let value = value.as_ref();

        // Skip ignored params
        if IGNORED_PARAMS.contains(&key) {
            continue;
        }

        // Extract suggested name
        if NAME_PARAMS.contains(&key) {
            if suggested_name.is_none() && !value.is_empty() {
                suggested_name = Some(value.to_string());
            }
            continue;
        }

        // Handle limit
        if key == "limit" {
            if let Ok(n) = value.parse::<u32>() {
                query.limit = Some(n);
            }
            continue;
        }

        // Check recognized vec params
        if let Some(&(_, field_name)) = RECOGNIZED_VEC_PARAMS
            .iter()
            .find(|&&(url_key, _)| url_key == key)
        {
            let target = match field_name {
                "product" => &mut query.product,
                "component" => &mut query.component,
                "status" => &mut query.status,
                "assignee" => &mut query.assignee,
                "creator" => &mut query.creator,
                "priority" => &mut query.priority,
                "severity" => &mut query.severity,
                _ => unreachable!(),
            };
            target.push(value.to_string());
            continue;
        }

        // Everything else -> raw_params
        query.raw_params.push((key.to_string(), value.to_string()));
    }

    Ok(ParsedUrl {
        query,
        suggested_name,
    })
}

/// Find a configured server whose URL hostname matches the given hostname.
fn find_server_by_hostname<'a>(config: &'a Config, hostname: &str) -> Option<&'a str> {
    for (name, srv) in &config.servers {
        if let Ok(srv_url) = Url::parse(&srv.url) {
            if srv_url.host_str() == Some(hostname) {
                return Some(name.as_str());
            }
        }
    }
    None
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::{Config, ServerConfig};
    use std::collections::HashMap;

    fn make_config(server_url: &str) -> Config {
        let mut servers = HashMap::new();
        servers.insert(
            "test".into(),
            ServerConfig {
                url: server_url.into(),
                api_key: None,
                api_key_env: None,
                api_key_keyring: None,
                email: None,
                auth_method: None,
                api_mode: None,
                server_version: None,
                tls_insecure: false,
            },
        );
        Config {
            default_server: Some("test".into()),
            servers,
            ..Config::default()
        }
    }

    #[test]
    fn parse_simple_url_with_recognized_params() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=Firefox&product=Thunderbird&bug_status=NEW&limit=50";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.kind, QueryKind::Url);
        assert_eq!(parsed.query.product, vec!["Firefox", "Thunderbird"]);
        assert_eq!(parsed.query.status, vec!["NEW"]);
        assert_eq!(parsed.query.limit, Some(50));
        assert!(parsed.query.raw_params.is_empty());
        assert_eq!(parsed.query.server.as_deref(), Some("test"));
        assert!(parsed.query.source_url.is_some());
    }

    #[test]
    fn parse_complex_boolean_chart_url() {
        let config = make_config("https://bugzilla.linux.ibm.com");
        let url = "https://bugzilla.linux.ibm.com/buglist.cgi?\
            chfield=qa_contact&chfieldfrom=-1w&chfieldto=Now&\
            classification=BugsAgainstDistros&\
            f1=qa_contact&o1=changedfrom&v1=user%40example.com&\
            known_name=my%20saved%20search&query_format=advanced&\
            list_id=12345&columnlist=qa_contact%2Cproduct";

        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.kind, QueryKind::Url);
        assert!(parsed.query.product.is_empty());

        // Boolean chart and chfield params should be in raw_params
        let raw_keys: Vec<&str> = parsed
            .query
            .raw_params
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(raw_keys.contains(&"f1"));
        assert!(raw_keys.contains(&"o1"));
        assert!(raw_keys.contains(&"v1"));
        assert!(raw_keys.contains(&"chfield"));
        assert!(raw_keys.contains(&"chfieldfrom"));
        assert!(raw_keys.contains(&"classification"));

        // Ignored params should not appear
        assert!(!raw_keys.contains(&"columnlist"));
        assert!(!raw_keys.contains(&"list_id"));
        assert!(!raw_keys.contains(&"query_format"));

        // URL-decoded value
        let v1 = parsed
            .query
            .raw_params
            .iter()
            .find(|(k, _)| k == "v1")
            .unwrap();
        assert_eq!(v1.1, "user@example.com");

        // Suggested name extracted and decoded
        assert_eq!(parsed.suggested_name.as_deref(), Some("my saved search"));
    }

    #[test]
    fn parse_url_without_buglist_cgi_errors() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/show_bug.cgi?id=123";
        let result = parse_bugzilla_url(url, &config);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("buglist.cgi"));
    }

    #[test]
    fn parse_malformed_url_errors() {
        let config = make_config("https://bugzilla.example.com");
        let result = parse_bugzilla_url("not a url", &config);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid URL"));
    }

    #[test]
    fn parse_url_hostname_matches_configured_server() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?product=Test";
        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.server.as_deref(), Some("test"));
    }

    #[test]
    fn parse_url_hostname_no_match_uses_default() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://other.example.com/buglist.cgi?product=Test";
        let parsed = parse_bugzilla_url(url, &config).unwrap();
        // No match, but default server exists so server is None (will use default)
        assert!(parsed.query.server.is_none());
    }

    #[test]
    fn parse_url_hostname_no_match_no_default_errors() {
        let config = Config {
            default_server: None,
            servers: HashMap::new(),
            ..Config::default()
        };
        let url = "https://other.example.com/buglist.cgi?product=Test";
        let result = parse_bugzilla_url(url, &config);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not match"));
    }

    #[test]
    fn parse_url_repeated_product_params_accumulate() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=Firefox&product=Thunderbird&product=Core";
        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(
            parsed.query.product,
            vec!["Firefox", "Thunderbird", "Core"]
        );
    }

    #[test]
    fn parse_url_decodes_percent_encoded_values() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=PPC64%20Development&assigned_to=user%40example.com";
        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.product, vec!["PPC64 Development"]);
        assert_eq!(parsed.query.assignee, vec!["user@example.com"]);
    }

    #[test]
    fn parse_url_all_recognized_fields() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            product=Firefox&component=General&bug_status=NEW&\
            assigned_to=dev%40test.com&reporter=creator%40test.com&\
            priority=P1&bug_severity=critical&limit=25";
        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert_eq!(parsed.query.product, vec!["Firefox"]);
        assert_eq!(parsed.query.component, vec!["General"]);
        assert_eq!(parsed.query.status, vec!["NEW"]);
        assert_eq!(parsed.query.assignee, vec!["dev@test.com"]);
        assert_eq!(parsed.query.creator, vec!["creator@test.com"]);
        assert_eq!(parsed.query.priority, vec!["P1"]);
        assert_eq!(parsed.query.severity, vec!["critical"]);
        assert_eq!(parsed.query.limit, Some(25));
        assert!(parsed.query.raw_params.is_empty());
    }

    #[test]
    fn parse_url_only_raw_params() {
        let config = make_config("https://bugzilla.example.com");
        let url = "https://bugzilla.example.com/buglist.cgi?\
            f1=qa_contact&o1=changedfrom&v1=user%40example.com";
        let parsed = parse_bugzilla_url(url, &config).unwrap();
        assert!(parsed.query.product.is_empty());
        assert_eq!(parsed.query.raw_params.len(), 3);
        assert!(parsed.query.has_filters());
    }

    #[test]
    fn find_server_by_hostname_matches() {
        let config = make_config("https://bugzilla.example.com");
        assert_eq!(
            find_server_by_hostname(&config, "bugzilla.example.com"),
            Some("test")
        );
    }

    #[test]
    fn find_server_by_hostname_no_match() {
        let config = make_config("https://bugzilla.example.com");
        assert_eq!(find_server_by_hostname(&config, "other.example.com"), None);
    }
}
```

- [ ] **Step 2: Add module declaration to `src/lib.rs`**

In `src/lib.rs`, add after the `pub mod types;` line:

```rust
pub mod url_parser;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test url_parser`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add src/url_parser.rs src/lib.rs
git commit -m "feat: add url_parser module to parse buglist.cgi URLs into SavedQuery"
```

---

### Task 4: Wire raw params into client

**Files:**
- Modify: `src/client/bug.rs:98-190`

- [ ] **Step 1: Write test for `append_raw_params`**

Add to the existing `#[cfg(test)] mod tests` in `src/client/bug.rs`:

```rust
#[test]
fn append_raw_params_empty_is_noop() {
    let client = reqwest::Client::new();
    let builder = client.get("https://example.com/rest/bug");
    let result = super::append_raw_params(builder, &[]);
    // If it compiles and doesn't panic, the noop case works.
    // We verify the full behavior via the wiremock integration test below.
    let _ = result;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib client::bug::tests::append_raw_params_empty -- --no-capture`
Expected: compilation error (function doesn't exist yet)

- [ ] **Step 3: Implement `append_raw_params`**

In `src/client/bug.rs`, add this function after `append_option_params` (around line 119):

```rust
/// Appends raw key-value parameters to the request builder verbatim.
/// Used for URL-imported queries with boolean chart params that
/// `bzr` does not natively model.
fn append_raw_params(
    mut builder: reqwest::RequestBuilder,
    raw_params: &[(String, String)],
) -> reqwest::RequestBuilder {
    for (key, value) in raw_params {
        builder = builder.query(&[(key, value)]);
    }
    builder
}
```

- [ ] **Step 4: Call `append_raw_params` in `search_bugs_rest`**

In the `search_bugs_rest` method, add the call after `append_option_params` and the `id` loop, but before the `include_fields` default check:

```rust
    async fn search_bugs_rest(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        let mut req_builder = self.http.get(self.url("bug"));
        req_builder = append_multi_value_params(req_builder, params);
        req_builder = append_negated_params(req_builder, params);
        req_builder = append_option_params(req_builder, params);

        for id in &params.id {
            req_builder = req_builder.query(&[("id", id)]);
        }

        // Append raw passthrough params (from URL-imported queries)
        req_builder = append_raw_params(req_builder, &params.raw_params);

        if params.include_fields.is_none() {
            req_builder = req_builder.query(&[("include_fields", BUG_DEFAULT_FIELDS)]);
        }
        let req = self.apply_auth(req_builder);
        let resp = self.send(req).await?;
        let data: BugListResponse = self.parse_json(resp).await?;
        Ok(data.bugs)
    }
```

- [ ] **Step 5: Force REST mode when raw params are present**

In the `search_bugs` method, add a check at the top:

```rust
    pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
        tracing::debug!(?params, %self.api_mode, "search parameters");

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
            // ... existing match arms unchanged ...
        }
    }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/client/bug.rs
git commit -m "feat: add raw_params passthrough in search_bugs_rest, force REST when present"
```

---

### Task 5: CLI changes — `bug search --from-url --save-as`

**Files:**
- Modify: `src/cli/bug.rs:55-68`

- [ ] **Step 1: Modify the `Search` variant in `BugAction`**

Replace the current `Search` variant with:

```rust
    /// Search bugs by text query or Bugzilla URL
    Search {
        /// Search query (mutually exclusive with --from-url)
        #[arg(conflicts_with = "from_url")]
        query: Option<String>,
        /// Execute a search from a Bugzilla buglist.cgi URL
        #[arg(long)]
        from_url: Option<String>,
        /// Save this URL query with the given name for future reuse (requires --from-url)
        #[arg(long, requires = "from_url")]
        save_as: Option<String>,
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compilation errors in `commands/bug.rs` where `handle_search` destructures the old `Search` variant. This is expected — we fix it in Task 7.

- [ ] **Step 3: Commit**

```bash
git add src/cli/bug.rs
git commit -m "feat: add --from-url and --save-as flags to bug search CLI"
```

---

### Task 6: CLI changes — `query save --from-url`, `query run --server`

**Files:**
- Modify: `src/cli/query.rs`

- [ ] **Step 1: Add `--from-url` to `Save` and `--server` to `Run`**

Replace the full `QueryAction` enum:

```rust
use clap::Subcommand;

#[derive(Subcommand)]
pub enum QueryAction {
    /// Save a named query
    Save {
        /// Query name
        name: String,
        /// Import query from a Bugzilla buglist.cgi URL (mutually exclusive with filter flags)
        #[arg(long, conflicts_with_all = ["search", "product", "component", "status", "assignee", "creator", "priority", "severity"])]
        from_url: Option<String>,
        /// Free-text search (creates a "search" kind query)
        #[arg(long)]
        search: Option<String>,
        /// Filter by product (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        product: Vec<String>,
        /// Filter by component (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        component: Vec<String>,
        /// Filter by status (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        status: Vec<String>,
        /// Filter by assignee (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        assignee: Vec<String>,
        /// Filter by creator (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        creator: Vec<String>,
        /// Filter by priority (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        priority: Vec<String>,
        /// Filter by severity (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        severity: Vec<String>,
        /// Max number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
    /// List all saved queries
    List,
    /// Show details of a saved query
    Show {
        /// Query name
        name: String,
    },
    /// Delete a saved query
    Delete {
        /// Query name
        name: String,
    },
    /// Run a saved query
    Run {
        /// Query name
        name: String,
        /// Override the saved limit
        #[arg(long)]
        limit: Option<u32>,
        /// Override the saved fields selection
        #[arg(long)]
        fields: Option<String>,
        /// Override the saved exclude-fields selection
        #[arg(long)]
        exclude_fields: Option<String>,
        /// Override the server to run against
        #[arg(long)]
        server: Option<String>,
    },
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compilation errors in `commands/query.rs` where `handle_save` and `handle_run` destructure the old variants. Expected — we fix in Task 8.

- [ ] **Step 3: Commit**

```bash
git add src/cli/query.rs
git commit -m "feat: add --from-url to query save and --server to query run CLI"
```

---

### Task 7: Command handler — `bug search --from-url`

**Files:**
- Modify: `src/commands/bug.rs:115-140`

- [ ] **Step 1: Write test for `--from-url` execution**

Add to the existing test module in `src/commands/bug.rs`:

```rust
#[tokio::test]
async fn handle_search_from_url_executes() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "TestProduct"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{"id": 1, "summary": "Test bug", "status": "NEW",
                          "product": "TestProduct", "component": "General"}]
            })),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let server_url = mock.uri();
    let url = format!(
        "{}/buglist.cgi?product=TestProduct&limit=10",
        server_url
    );
    let action = BugAction::Search {
        query: None,
        from_url: Some(url),
        save_as: None,
        limit: 50,
        fields: None,
        exclude_fields: None,
    };

    let (result, output) = capture_stdout(
        super::execute(&action, None, OutputFormat::Json, None),
    )
    .await;
    assert!(result.is_ok(), "from-url search failed: {result:?}");
    let parsed: serde_json::Value = extract_json(&output);
    assert_eq!(parsed[0]["id"], 1);
}

#[tokio::test]
async fn handle_search_from_url_saves_query() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": []})),
        )
        .mount(&mock)
        .await;

    let server_url = mock.uri();
    let url = format!(
        "{}/buglist.cgi?product=TestProduct&known_name=my-query",
        server_url
    );
    let action = BugAction::Search {
        query: None,
        from_url: Some(url),
        save_as: Some("my-query".into()),
        limit: 50,
        fields: None,
        exclude_fields: None,
    };

    let (result, _output) = capture_stdout(
        super::execute(&action, None, OutputFormat::Json, None),
    )
    .await;
    assert!(result.is_ok(), "from-url save failed: {result:?}");

    // Verify query was saved to config
    let config = crate::config::Config::load().unwrap();
    let saved = config.queries.get("my-query").unwrap();
    assert_eq!(saved.kind, crate::types::QueryKind::Url);
    assert_eq!(saved.product, vec!["TestProduct"]);
    assert!(saved.source_url.is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib commands::bug::tests::handle_search_from_url -- --no-capture 2>&1 | head -20`
Expected: compilation errors (the `handle_search` function doesn't handle the new fields yet)

- [ ] **Step 3: Update `handle_search` to support `--from-url`**

Replace the `handle_search` function in `src/commands/bug.rs`:

```rust
async fn handle_search(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::Search {
        query,
        from_url,
        save_as,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let params = if let Some(url_str) = from_url {
        let config = crate::config::Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;

        // Save if requested
        if let Some(name) = save_as {
            let mut config = config;
            let is_update = config.queries.contains_key(name.as_str());
            config.queries.insert(name.clone(), parsed.query.clone());
            config.save()?;
            let verb = if is_update { "Updated" } else { "Saved" };
            tracing::info!("{verb} query '{name}'");
        }

        let mut params = parsed.query.to_search_params();
        // CLI overrides
        if *limit != 50 {
            params.limit = Some(*limit);
        } else if params.limit.is_none() {
            params.limit = Some(*limit);
        }
        if let Some(f) = fields {
            params.include_fields = Some(f.clone());
        }
        if let Some(ef) = exclude_fields {
            params.exclude_fields = Some(ef.clone());
        }
        params
    } else {
        let query_str = query.as_deref().ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "either a search query or --from-url is required".into(),
            )
        })?;
        SearchParams {
            quicksearch: Some(query_str.to_string()),
            limit: Some(*limit),
            include_fields: fields.clone(),
            exclude_fields: exclude_fields.clone(),
            ..Default::default()
        }
    };

    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib commands::bug::tests::handle_search`
Expected: all search tests pass

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/commands/bug.rs
git commit -m "feat: handle --from-url in bug search command"
```

---

### Task 8: Command handler — `query save --from-url`, `query run --server`

**Files:**
- Modify: `src/commands/query.rs`

- [ ] **Step 1: Write test for `query save --from-url`**

Add to the test module in `src/commands/query.rs`:

```rust
#[tokio::test]
async fn query_save_from_url() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let server_url = mock.uri();
    let url = format!(
        "{}/buglist.cgi?product=TestProduct&f1=qa_contact&o1=changedfrom&v1=user%40example.com",
        server_url
    );
    let action = QueryAction::Save {
        name: "url-query".into(),
        from_url: Some(url),
        search: None,
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: None,
        fields: None,
        exclude_fields: None,
    };
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query save --from-url failed: {result:?}");

    let config = Config::load().unwrap();
    let saved = &config.queries["url-query"];
    assert_eq!(saved.kind, crate::types::QueryKind::Url);
    assert_eq!(saved.product, vec!["TestProduct"]);
    assert!(!saved.raw_params.is_empty());
    assert!(saved.source_url.is_some());
}

#[tokio::test]
async fn query_run_with_server_override() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Save a query with server association
    let save_action = save_action("server-test");
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": []})),
        )
        .mount(&mock)
        .await;

    // Run with --server override (uses the test server from setup_test_env)
    let run_action = QueryAction::Run {
        name: "server-test".into(),
        limit: None,
        fields: None,
        exclude_fields: None,
        server: Some("test".into()),
    };
    let (result, _) =
        capture_stdout(super::execute(&run_action, Some("test"), OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query run with server override failed: {result:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib commands::query::tests::query_save_from_url -- --no-capture 2>&1 | head -20`
Expected: compilation error (destructuring doesn't include `from_url`)

- [ ] **Step 3: Update `handle_save` to support `--from-url`**

Replace the `handle_save` function:

```rust
fn handle_save(action: &QueryAction, format: OutputFormat) -> Result<()> {
    let QueryAction::Save {
        name,
        from_url,
        search,
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let query = if let Some(url_str) = from_url {
        let config = Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
        let mut query = parsed.query;
        // Apply any explicit overrides
        if let Some(limit) = limit {
            query.limit = Some(*limit);
        }
        if let Some(f) = fields {
            query.fields = Some(f.clone());
        }
        if let Some(ef) = exclude_fields {
            query.exclude_fields = Some(ef.clone());
        }
        query
    } else {
        let kind = if search.is_some() {
            QueryKind::Search
        } else {
            QueryKind::List
        };

        SavedQuery {
            kind,
            product: product.clone(),
            component: component.clone(),
            status: status.clone(),
            assignee: assignee.clone(),
            creator: creator.clone(),
            priority: priority.clone(),
            severity: severity.clone(),
            quicksearch: search.clone(),
            limit: *limit,
            fields: fields.clone(),
            exclude_fields: exclude_fields.clone(),
            ..SavedQuery::default()
        }
    };

    if !query.has_filters() {
        return Err(BzrError::InputValidation(
            "query must have at least one filter set".into(),
        ));
    }

    let mut config = Config::load()?;
    let is_update = config.queries.contains_key(name.as_str());
    config.queries.insert(name.clone(), query);
    config.save()?;

    let verb = if is_update { "Updated" } else { "Saved" };
    output::print_query_saved(name, verb, format);
    Ok(())
}
```

- [ ] **Step 4: Update `handle_run` to support `--server` override**

Replace the `handle_run` function:

```rust
async fn handle_run(
    action: &QueryAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
) -> Result<()> {
    let QueryAction::Run {
        name,
        limit,
        fields,
        exclude_fields,
        server: server_override,
    } = action
    else {
        unreachable!()
    };

    let config = Config::load()?;
    let query = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;

    let mut params = query.to_search_params();

    // Apply runtime overrides
    if let Some(limit) = limit {
        params.limit = Some(*limit);
    }
    if let Some(fields) = fields {
        params.include_fields = Some(fields.clone());
    }
    if let Some(exclude_fields) = exclude_fields {
        params.exclude_fields = Some(exclude_fields.clone());
    }

    // Server resolution: CLI --server > query run --server > saved server > default
    let effective_server = server
        .or(server_override.as_deref())
        .or(query.server.as_deref());

    let client = super::shared::connect_and_configure(effective_server, api).await?;
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
}
```

- [ ] **Step 5: Update the `save_action` test helper to include `from_url`**

In the test module, update the `save_action` helper:

```rust
fn save_action(name: &str) -> QueryAction {
    QueryAction::Save {
        name: name.into(),
        from_url: None,
        search: None,
        product: vec!["Firefox".into()],
        component: vec![],
        status: vec!["NEW".into()],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: Some(25),
        fields: None,
        exclude_fields: None,
    }
}
```

- [ ] **Step 6: Update all existing `QueryAction::Save` test constructions**

In the test module, every manual `QueryAction::Save { .. }` construction must include `from_url: None`. Update these tests:
- `query_save_search_kind`
- `query_save_requires_filter`
- `query_run_executes_saved_query`
- `query_run_with_limit_override`
- `query_save_existing_entry_reports_updated` (both the save and the update action)
- `query_delete_removes_saved_query`
- `query_run_applies_field_overrides`

- [ ] **Step 7: Update all existing `QueryAction::Run` test constructions**

Every manual `QueryAction::Run { .. }` construction must include `server: None`. Update these tests:
- `query_run_executes_saved_query`
- `query_run_with_limit_override`
- `query_run_applies_field_overrides`
- `query_run_unknown_errors`

- [ ] **Step 8: Run tests**

Run: `cargo test --lib commands::query`
Expected: all query command tests pass

- [ ] **Step 9: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 10: Commit**

```bash
git add src/commands/query.rs
git commit -m "feat: handle --from-url in query save, --server override in query run"
```

---

### Task 9: Enhanced `query show` output for URL queries

**Files:**
- Modify: `src/output/query.rs:59-89`

- [ ] **Step 1: Write test for URL query display**

Add to the test module in `src/output/query.rs`:

```rust
fn make_url_query() -> SavedQuery {
    SavedQuery {
        kind: QueryKind::Url,
        source_url: Some("https://bugzilla.example.com/buglist.cgi?product=Firefox&f1=qa_contact".into()),
        server: Some("example".into()),
        product: vec!["Firefox".into()],
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "changedfrom".into()),
        ],
        limit: Some(100),
        ..SavedQuery::default()
    }
}

#[test]
fn query_detail_json_includes_url_fields() {
    #[derive(serde::Serialize)]
    struct QueryView<'a> {
        name: &'a str,
        #[serde(flatten)]
        query: &'a SavedQuery,
    }
    let query = make_url_query();
    let view = QueryView {
        name: "url-q",
        query: &query,
    };
    let json = serde_json::to_string_pretty(&view).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["kind"], "url");
    assert_eq!(
        parsed["source_url"],
        "https://bugzilla.example.com/buglist.cgi?product=Firefox&f1=qa_contact"
    );
    assert_eq!(parsed["server"], "example");
    assert_eq!(parsed["raw_params"].as_array().unwrap().len(), 2);
}

#[test]
fn query_summary_line_renders_url_query() {
    let line = query_summary_line("url-q", &make_url_query());
    assert!(line.starts_with("url-q (kind=url"));
    assert!(line.contains("product=Firefox"));
    assert!(line.contains("2 raw params"));
}

#[cfg(unix)]
#[tokio::test]
async fn print_query_detail_table_renders_url_fields() {
    let _lock = crate::ENV_LOCK.lock().await;
    let query = make_url_query();

    let ((), output) = crate::test_helpers::capture_stdout(async {
        print_query_detail("url-q", &query, OutputFormat::Table);
    })
    .await;

    assert!(output.contains("Source URL"));
    assert!(output.contains("bugzilla.example.com"));
    assert!(output.contains("Server"));
    assert!(output.contains("example"));
    assert!(output.contains("Raw params"));
    assert!(output.contains("2"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib output::query::tests::query_summary_line_renders_url -- --no-capture`
Expected: fails (no "raw params" in output yet)

- [ ] **Step 3: Update `query_summary_line` for URL queries**

In `src/output/query.rs`, update the `query_summary_line` function:

```rust
fn query_summary_line(name: &str, q: &SavedQuery) -> String {
    let kind_label = match q.kind {
        QueryKind::List => "list",
        QueryKind::Search => "search",
        QueryKind::Url => "url",
    };
    let mut parts = vec![format!("kind={kind_label}")];
    if !q.product.is_empty() {
        parts.push(format!("product={}", q.product.join(",")));
    }
    if !q.status.is_empty() {
        parts.push(format!("status={}", q.status.join(",")));
    }
    if let Some(qs) = &q.quicksearch {
        parts.push(format!("search=\"{qs}\""));
    }
    if let Some(limit) = q.limit {
        parts.push(format!("limit={limit}"));
    }
    if !q.raw_params.is_empty() {
        parts.push(format!("{} raw params", q.raw_params.len()));
    }
    format!("{name} ({})", parts.join(", "))
}
```

- [ ] **Step 4: Update `print_query_detail` for URL queries**

In `src/output/query.rs`, update the table rendering in `print_query_detail`:

```rust
pub fn print_query_detail(name: &str, query: &SavedQuery, format: OutputFormat) {
    #[derive(serde::Serialize)]
    struct QueryView<'a> {
        name: &'a str,
        #[serde(flatten)]
        query: &'a SavedQuery,
    }

    let view = QueryView { name, query };
    print_formatted(&view, format, |view| {
        let kind_label = match view.query.kind {
            QueryKind::List => "list",
            QueryKind::Search => "search",
            QueryKind::Url => "url",
        };
        print_field("Name", view.name);
        print_field("Kind", kind_label);
        print_optional_field("Source URL", view.query.source_url.as_deref());
        print_optional_field("Server", view.query.server.as_deref());
        print_list_field("Product", &view.query.product);
        print_list_field("Component", &view.query.component);
        print_list_field("Status", &view.query.status);
        print_list_field("Assignee", &view.query.assignee);
        print_list_field("Creator", &view.query.creator);
        print_list_field("Priority", &view.query.priority);
        print_list_field("Severity", &view.query.severity);
        print_optional_field("Search", view.query.quicksearch.as_deref());
        if let Some(limit) = view.query.limit {
            print_field("Limit", &limit.to_string());
        }
        print_optional_field("Fields", view.query.fields.as_deref());
        print_optional_field("Exclude", view.query.exclude_fields.as_deref());
        if !view.query.raw_params.is_empty() {
            print_field("Raw params", &view.query.raw_params.len().to_string());
        }
    });
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib output::query`
Expected: all output query tests pass

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/output/query.rs
git commit -m "feat: display source_url, server, and raw_params in query show output"
```

---

### Task 10: Fix integration tests and dispatch tests

**Files:**
- Modify: `tests/integration.rs` (if any query-related tests exist)
- Modify: `src/lib.rs` (dispatch test for query save uses old CLI parse)

- [ ] **Step 1: Check for integration test breakage**

Run: `cargo test --test integration 2>&1 | head -40`
Expected: may show compilation errors if integration tests construct `QueryAction` variants

- [ ] **Step 2: Fix any broken integration test constructions**

If integration tests construct `QueryAction::Save` or `QueryAction::Run` directly, add the new fields (`from_url: None`, `server: None`). If they parse CLI strings via `Cli::try_parse_from`, they should be fine.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings

- [ ] **Step 5: Run fmt check**

Run: `cargo fmt --check`
Expected: no formatting issues

- [ ] **Step 6: Commit (if any changes were needed)**

```bash
git add -A
git commit -m "fix: update integration tests for new QueryAction fields"
```

---

### Task 11: Update CLI documentation

**Files:**
- Modify: `docs/bzr-cli.md`

- [ ] **Step 1: Add `--from-url` and `--save-as` to the `bug search` section**

Find the `bug search` section in `docs/bzr-cli.md` and add documentation for:
- `--from-url <URL>` — Execute a search from a Bugzilla buglist.cgi URL
- `--save-as <NAME>` — Save this URL query for future reuse (requires `--from-url`)

Include example:

```
bzr bug search --from-url "https://bugzilla.example.com/buglist.cgi?product=Firefox&bug_status=NEW"
bzr bug search --from-url "https://bugzilla.example.com/buglist.cgi?..." --save-as "my-query"
```

- [ ] **Step 2: Add `--from-url` to the `query save` section**

Document: `--from-url <URL>` — Import query from a Bugzilla URL (mutually exclusive with filter flags)

Include example:

```
bzr query save my-query --from-url "https://bugzilla.example.com/buglist.cgi?..."
```

- [ ] **Step 3: Add `--server` to the `query run` section**

Document: `--server <NAME>` — Override the server to run the query against

Include example:

```
bzr query run my-query --server other-server --limit 50
```

- [ ] **Step 4: Commit**

```bash
git add docs/bzr-cli.md
git commit -m "docs: document --from-url, --save-as, and --server flags"
```
