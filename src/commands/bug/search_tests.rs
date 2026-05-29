#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn from_url_action(url: String, save_as: Option<String>) -> BugAction {
    BugAction::Search {
        query: None,
        from_url: Some(url),
        save_as,
        limit: None,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
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

    let mut __io = crate::test_helpers::CapturedIo::new();

    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;

    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "from-url search failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
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
        field_args: crate::cli::FieldArgs {
            fields: Some("id,summary".into()),
            exclude_fields: Some("comments".into()),
        },
    };
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;
    let _ = __io3.out_str().to_string();
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

    let mut __io4 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io4.writers(),
    )
    .await;

    let _ = __io4.out_str().to_string();
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

    let mut __io5 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;

    let _output = __io5.out_str().to_string();
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
    let mut __io6 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io6.writers(),
    )
    .await;
    let _output = __io6.out_str().to_string();
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
    let mut __io7 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io7.writers(),
    )
    .await;
    let _output = __io7.out_str().to_string();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("no name provided for --save-as"),
        "unexpected error: {err}"
    );
}
