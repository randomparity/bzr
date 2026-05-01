use crate::cli::BugAction;
use crate::error::Result;
use crate::output;
use crate::types::{ApiMode, OutputFormat, SavedQuery, SearchParams};

/// Determine the `save_as` name + query to persist after a successful URL-based
/// search. Returns None when --save-as wasn't passed; errors when --save-as=""
/// is passed but the URL has no `known_name`/`query_based_on` to fall back on.
fn resolve_save_info(
    save_as: Option<&String>,
    suggested_name: Option<String>,
    parsed_query: &SavedQuery,
) -> Result<Option<(String, SavedQuery)>> {
    let Some(raw_name) = save_as else {
        return Ok(None);
    };
    let name = if raw_name.is_empty() {
        suggested_name.ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "no name provided for --save-as and URL has no known_name; \
                 specify a name explicitly: --save-as <name>"
                    .into(),
            )
        })?
    } else {
        raw_name.clone()
    };
    Ok(Some((name, parsed_query.clone())))
}

/// Convert a parsed URL's query into `SearchParams`, applying CLI overrides
/// and a default limit of 50 when neither URL nor CLI specifies one.
fn build_params_from_url(
    parsed_query: SavedQuery,
    limit: Option<u32>,
    fields: Option<&str>,
    exclude_fields: Option<&str>,
) -> SearchParams {
    let mut params = parsed_query.into_search_params();
    if params.limit.is_none() && limit.is_none() {
        params.limit = Some(50);
    }
    params.apply_overrides(limit, fields, exclude_fields);
    params
}

/// Handles bug search — builds its own client (unlike other handlers) because
/// `--from-url` may resolve a different server from the URL hostname.
pub(super) async fn handle(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
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

    let (client, params, save_info) = if let Some(url_str) = from_url {
        let config = crate::config::Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
        let effective_server = server.or(parsed.query.server.as_deref());
        let client = crate::commands::shared::connect_and_configure(effective_server, api).await?;
        let save_info = resolve_save_info(save_as.as_ref(), parsed.suggested_name, &parsed.query)?;
        let params = build_params_from_url(
            parsed.query,
            *limit,
            fields.as_deref(),
            exclude_fields.as_deref(),
        );
        (client, params, save_info)
    } else {
        let query_str = query.as_deref().ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "either a search query or --from-url is required".into(),
            )
        })?;
        let client = crate::commands::shared::connect_and_configure(server, api).await?;
        let params = SearchParams {
            quicksearch: Some(query_str.to_string()),
            limit: Some(limit.unwrap_or(50)),
            include_fields: fields.clone(),
            exclude_fields: exclude_fields.clone(),
            ..Default::default()
        };
        (client, params, None)
    };

    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);

    if let Some((name, query)) = save_info {
        let mut config = crate::config::Config::load()?;
        let is_update = config.queries.contains_key(name.as_str());
        config.queries.insert(name.clone(), query);
        config.save()?;
        let verb = if is_update { "Updated" } else { "Saved" };
        crate::output::print_query_saved(&name, verb, format);
    }

    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, ResponseTemplate};

    use crate::cli::BugAction;
    use crate::test_helpers::{capture_stdout, extract_json, setup_test_env};
    use crate::types::OutputFormat;

    fn from_url_action(url: String, save_as: Option<String>) -> BugAction {
        BugAction::Search {
            query: None,
            from_url: Some(url),
            save_as,
            limit: None,
            fields: None,
            exclude_fields: None,
        }
    }

    #[tokio::test]
    async fn handle_search_from_url_executes() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("product", "TestProduct"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{"id": 1, "summary": "Test bug", "status": "NEW",
                          "product": "TestProduct", "component": "General"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let server_url = mock.uri();
        let url = format!("{server_url}/buglist.cgi?product=TestProduct&limit=10");
        let action = from_url_action(url, None);

        let (result, output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok(), "from-url search failed: {result:?}");
        let parsed: serde_json::Value = extract_json(&output);
        assert_eq!(parsed[0]["id"], 1);
    }

    #[tokio::test]
    async fn handle_search_from_url_preserves_url_limit_when_cli_unset() {
        // URL specifies limit=10 and CLI passes nothing — the URL limit
        // must reach the server unchanged, not be overwritten by the
        // 50-default.
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("product", "TestProduct"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        let url = format!("{}/buglist.cgi?product=TestProduct&limit=10", mock.uri());
        let action = from_url_action(url, None);

        let (result, _) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(
            result.is_ok(),
            "from-url with explicit limit failed: {result:?}"
        );
    }

    #[tokio::test]
    async fn handle_search_quicksearch_passes_limit_and_field_filters() {
        // Non-from-url path: limit / fields / exclude_fields from the
        // CLI must reach the server.
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("quicksearch", "crash"))
            .and(query_param("limit", "5"))
            .and(query_param("include_fields", "id,summary"))
            .and(query_param("exclude_fields", "comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        let action = BugAction::Search {
            query: Some("crash".into()),
            from_url: None,
            save_as: None,
            limit: Some(5),
            fields: Some("id,summary".into()),
            exclude_fields: Some("comments".into()),
        };
        let (result, _) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(
            result.is_ok(),
            "quicksearch with filters failed: {result:?}"
        );
    }

    #[tokio::test]
    async fn handle_search_from_url_passes_raw_params() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // Wiremock matcher verifying raw params appear in the request
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("product", "TestProduct"))
            .and(query_param("f1", "qa_contact"))
            .and(query_param("o1", "changedfrom"))
            .and(query_param("v1", "user@example.com"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;

        let server_url = mock.uri();
        let url = format!(
            "{server_url}/buglist.cgi?product=TestProduct&f1=qa_contact&o1=changedfrom&v1=user%40example.com"
        );
        let action = from_url_action(url, None);

        let (result, _) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(
            result.is_ok(),
            "from-url with raw params failed: {result:?}"
        );
    }

    #[tokio::test]
    async fn handle_search_from_url_saves_query() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .mount(&mock)
            .await;

        let server_url = mock.uri();
        let url = format!("{server_url}/buglist.cgi?product=TestProduct&known_name=my-query");
        let action = from_url_action(url, Some("my-query".into()));

        let (result, _output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok(), "from-url save failed: {result:?}");

        let config = crate::config::Config::load().unwrap();
        let saved = config.queries.get("my-query").unwrap();
        assert_eq!(saved.kind, crate::types::QueryKind::Url);
        assert_eq!(saved.product, vec!["TestProduct"]);
        assert!(saved.source_url.is_some());
    }

    #[tokio::test]
    async fn handle_search_from_url_auto_names_from_known_name() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .mount(&mock)
            .await;

        let server_url = mock.uri();
        let url =
            format!("{server_url}/buglist.cgi?product=TestProduct&known_name=my%20saved%20search");
        let action = from_url_action(url, Some(String::new()));
        let (result, _output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(
            result.is_ok(),
            "auto-name from known_name failed: {result:?}"
        );

        let config = crate::config::Config::load().unwrap();
        assert!(
            config.queries.contains_key("my saved search"),
            "query should be saved as 'my saved search'"
        );
    }

    #[tokio::test]
    async fn handle_search_save_as_no_name_no_known_name_errors() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = from_url_action(
            "https://bugzilla.example.com/buglist.cgi?product=Firefox".into(),
            Some(String::new()),
        );
        let (result, _output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no name provided for --save-as"),
            "unexpected error: {err}"
        );
    }
}
