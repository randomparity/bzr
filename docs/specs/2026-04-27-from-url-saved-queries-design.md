# Design: `--from-url` and Saved Query Enhancement

**Date:** 2026-04-27
**Status:** Draft

## Problem

Bugzilla's GUI lets users build complex advanced queries with boolean charts, field-change tracking, and multi-field conditions. These queries are shareable as URLs but cannot be used from the `bzr` CLI. Users must manually translate URL parameters into CLI flags, which is impractical for advanced queries.

## Goals

1. Accept a Bugzilla `buglist.cgi` URL via `--from-url` and execute the query directly.
2. Optionally save the parsed query as a named template for future reuse via `--save-as`.
3. Allow runtime overrides (limit, fields, server) when running saved queries.
4. Preserve the full fidelity of the original URL, including boolean chart parameters that `bzr` does not natively model.

## Non-Goals

- Generating Bugzilla GUI URLs from CLI queries (reverse direction).
- Parsing or validating boolean chart logic (we pass it through verbatim).
- XML-RPC support for URL-sourced queries (boolean charts are a REST/CGI concept).

## Design

### Data Model

Extend `SavedQuery` with three new fields:

```rust
pub struct SavedQuery {
    // ... existing fields (kind, product, component, status, etc.) ...

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
}
```

Add a new `QueryKind::Url` variant to indicate URL-sourced queries.

Extend `SearchParams` with a `raw_params: Vec<(String, String)>` field (defaults to empty, so existing callers are unaffected).

### URL Parsing

New module: `src/url_parser.rs`

**Entry point:** `parse_bugzilla_url(url: &str, config: &Config) -> Result<SavedQuery>`

**Steps:**

1. Parse and validate the URL using the `url` crate (transitive dep via `reqwest`).
2. Verify the path contains `buglist.cgi`; reject other pages.
3. Match the hostname against configured `ServerConfig` URLs to populate `server`.
4. Iterate query parameters, classifying each:

**Recognized parameters mapped to structured fields:**

| URL parameter | `SavedQuery` field |
|---|---|
| `product` | `product` (vec, may repeat) |
| `component` | `component` (vec) |
| `bug_status` | `status` (vec) |
| `assigned_to` | `assignee` (vec) |
| `reporter` | `creator` (vec) |
| `priority` | `priority` (vec) |
| `bug_severity` | `severity` (vec) |
| `limit` | `limit` |

**Ignored parameters (display/session metadata):**

| URL parameter | Reason |
|---|---|
| `columnlist` | Display preference, not a filter |
| `list_id` | Session-specific |
| `query_format` | Always "advanced" for these URLs |

**Name extraction:**

`known_name` and `query_based_on` are extracted as a suggested default name for `--save-as` when the user doesn't provide one explicitly.

**Everything else goes to `raw_params`:**

Boolean chart params (`f1`, `o1`, `v1`, `j3`, `OP`, `CP`), `chfield*` params, `classification`, and any other unrecognized keys are stored as ordered `(key, value)` pairs.

**Server matching:**

Compare the URL hostname against each `ServerConfig.url` hostname. If exactly one matches, associate the query with that server name. If none match but a default server is configured, leave `server` as `None` (the default server will be used at runtime) and warn that the URL hostname didn't match any configured server. If none match and no default server is configured, return an error advising the user to configure the server first.

### CLI Changes

#### `bug search` -- add `--from-url` and `--save-as`

```
bzr bug search --from-url "https://..." --save-as "my-query"
bzr bug search --from-url "https://..."
```

- `--from-url` is mutually exclusive with the positional `query` argument (enforced by clap).
- When present, parses the URL, builds `SearchParams` + raw params, and executes.
- `--save-as` optionally persists to config before executing.
- `--save-as` requires `--from-url`.

#### `query save` -- add `--from-url`

```
bzr query save --from-url "https://..." --name "my-query"
```

Saves without executing. Mutually exclusive with the existing manual filter flags.

#### `query run` -- add `--server` override

```
bzr query run "my-query" --server other-server --limit 50
```

Existing `--limit`, `--fields`, `--exclude-fields` overrides continue to work. New `--server` flag overrides the stored server association.

#### `query show` -- enhanced display for URL queries

For URL-sourced queries, displays the original URL, recognized parsed fields, and a count of raw passthrough parameters. JSON format includes all raw params.

### Execution Flow

**`bug search --from-url`:**

1. Parse URL via `url_parser::parse_bugzilla_url()`.
2. If `--save-as` present, persist `SavedQuery` to config.
3. Convert recognized fields to `SearchParams` via `to_search_params()`.
4. Copy `raw_params` from `SavedQuery` into `SearchParams.raw_params`.
5. Apply CLI overrides (`--limit`, `--fields`, etc.).
6. Resolve server: CLI `--server` > parsed server > default server.
7. Execute via `search_bugs()`.

**`query run`:**

1. Load `SavedQuery` from config.
2. Convert to `SearchParams` (including `raw_params`).
3. Apply runtime overrides.
4. Resolve server: CLI `--server` > saved server > default server.
5. Execute via `search_bugs()`.

**Raw params in the client:**

New function in `client/bug.rs`:

```rust
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

Called in `search_bugs_rest()` after `append_option_params` and before `apply_auth`.

**XML-RPC interaction:** When `raw_params` is non-empty and `api_mode` is `XmlRpc` or `Hybrid`, force REST mode and log a warning. Boolean chart params are a REST/CGI concept with no XML-RPC equivalent.

### Files Changed

| File | Change |
|---|---|
| `src/url_parser.rs` | **New.** URL parsing logic. |
| `src/types/bug.rs` | Add `source_url`, `server`, `raw_params` to `SavedQuery`. Add `QueryKind::Url`. Add `raw_params` to `SearchParams`. |
| `src/cli/bug.rs` | Add `--from-url` and `--save-as` to `Search` variant. |
| `src/cli/query.rs` | Add `--from-url` to `Save`. Add `--server` to `Run`. |
| `src/commands/bug.rs` | Handle `--from-url` in `handle_search`. |
| `src/commands/query.rs` | Handle `--from-url` in `handle_save`. Pass `raw_params` through in `handle_run`. |
| `src/client/bug.rs` | Add `append_raw_params`. Call it in `search_bugs_rest`. Force REST when raw params present. |
| `src/output/query.rs` | Display `source_url` and raw param count in `query show`. |
| `src/lib.rs` | Add `mod url_parser`. |
| `docs/bzr-cli.md` | Document new flags. |

### Testing

**Unit tests (`url_parser.rs`):**
- Simple URL with product/status/limit: recognized fields populated, no raw params.
- Complex boolean chart URL (from the motivating example): `classification` and `f1/o1/v1` etc. in `raw_params`, `known_name` extracted.
- URL without `buglist.cgi` path: error.
- Malformed URL: error.
- Server hostname matching: match, no match (warn), not configured (error).
- Repeated params (multiple `product=`): accumulate into vec.
- URL-decoded values: `%20` becomes space, etc.

**Unit tests (`client/bug.rs`):**
- `append_raw_params` with empty vec: no change.
- `append_raw_params` with boolean chart params: params appear in request.
- `search_bugs_rest` with non-empty `raw_params`: wiremock verifies HTTP request.

**Integration tests (`commands/query.rs`):**
- `query save --from-url` parses and persists; `query show` displays original URL and raw params.
- `query run` on URL-sourced query sends raw params to server (wiremock).
- `query run` with `--limit` override replaces saved limit.
- `query run` with `--server` override targets different server.

**Integration tests (`commands/bug.rs`):**
- `bug search --from-url` executes parsed query (wiremock verifies params).
- `bug search --from-url --save-as` both executes and saves.
- `--save-as` name defaults to `known_name` from URL when not provided.
- `--from-url` mutually exclusive with positional query (clap enforces).

**Edge cases:**
- URL with only boolean chart params (nothing recognized): works, everything in `raw_params`.
- Non-empty `raw_params` forces REST when `api_mode` is Hybrid/XmlRpc: warning logged.
