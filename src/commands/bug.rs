use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::{self, ActionResult, BatchFailure, BatchResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateBugParams, IdListUpdate, SearchParams, UpdateBugParams};

/// Dispatch bug actions to their respective handlers.
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        BugAction::List { .. } => handle_list(&client, action, format).await,
        BugAction::View { .. } => handle_view(&client, action, format).await,
        BugAction::History { .. } => handle_history(&client, action, format).await,
        BugAction::Search { .. } => handle_search(action, server, format, api).await,
        BugAction::My { .. } => handle_my(&client, action, format).await,
        BugAction::Create { .. } => handle_create(&client, action, format).await,
        BugAction::Clone { .. } => handle_clone(&client, action, format).await,
        BugAction::Update { .. } => handle_update(&client, action, format).await,
    }
}

async fn handle_list(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::List {
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        id,
        alias,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let params = SearchParams {
        product: product.clone(),
        component: component.clone(),
        status: status.clone(),
        assigned_to: assignee.clone(),
        creator: creator.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        id: id.clone(),
        alias: alias.clone(),
        limit: Some(*limit),
        include_fields: fields.clone(),
        exclude_fields: exclude_fields.clone(),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
}

async fn handle_view(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::View {
        id,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let bug = client
        .get_bug(id, fields.as_deref(), exclude_fields.as_deref())
        .await?;
    output::print_bug_detail(&bug, format);
    Ok(())
}

async fn handle_history(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::History { id, since } = action else {
        unreachable!()
    };

    let history = client.get_bug_history_since(*id, since.as_deref()).await?;
    if history.is_empty() {
        #[expect(clippy::print_stdout)]
        {
            println!("No history for bug #{id}.");
        }
    } else {
        output::print_history(&history, format);
    }
    Ok(())
}

/// Handles bug search — builds its own client (unlike other handlers) because
/// `--from-url` may resolve a different server from the URL hostname.
async fn handle_search(
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
        let client = super::shared::connect_and_configure(effective_server, api).await?;

        let save_info = if let Some(raw_name) = save_as {
            let name = if raw_name.is_empty() {
                parsed.suggested_name.ok_or_else(|| {
                    crate::error::BzrError::InputValidation(
                        "no name provided for --save-as and URL has no known_name; \
                         specify a name explicitly: --save-as <name>"
                            .into(),
                    )
                })?
            } else {
                raw_name.clone()
            };
            Some((name, parsed.query.clone()))
        } else {
            None
        };

        let mut params = parsed.query.into_search_params();
        if params.limit.is_none() && limit.is_none() {
            params.limit = Some(50);
        }
        params.apply_overrides(*limit, fields.as_deref(), exclude_fields.as_deref());
        (client, params, save_info)
    } else {
        let query_str = query.as_deref().ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "either a search query or --from-url is required".into(),
            )
        })?;
        let client = super::shared::connect_and_configure(server, api).await?;
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

    // Save query to config after successful search
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

async fn handle_my(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::My {
        created,
        cc,
        all,
        status,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let whoami = client.whoami().await?;
    let email = whoami.name;
    let mut all_bugs: Vec<crate::types::Bug> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // Build search params for each enabled filter, varying one field.
    let base = SearchParams {
        status: status.clone(),
        limit: Some(*limit),
        include_fields: fields.clone(),
        exclude_fields: exclude_fields.clone(),
        ..Default::default()
    };
    let mut searches = Vec::new();
    if *all || (!created && !cc) {
        let mut p = base.clone();
        p.assigned_to = vec![email.clone()];
        searches.push(p);
    }
    if *all || *created {
        let mut p = base.clone();
        p.creator = vec![email.clone()];
        searches.push(p);
    }
    if *all || *cc {
        let mut p = base;
        p.cc = Some(email.clone());
        searches.push(p);
    }

    for params in &searches {
        for bug in client.search_bugs(params).await? {
            if seen_ids.insert(bug.id) {
                all_bugs.push(bug);
            }
        }
    }

    output::print_bugs(&all_bugs, format);
    Ok(())
}

async fn handle_create(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::Create {
        template: template_name,
        product,
        component,
        summary,
        version,
        description,
        priority,
        severity,
        assignee,
        op_sys,
        rep_platform,
        blocks,
        depends_on,
    } = action
    else {
        unreachable!()
    };

    // Load template defaults if specified
    let tmpl = if let Some(name) = template_name {
        let config = crate::config::Config::load()?;
        let t = config.templates.get(name.as_str()).ok_or_else(|| {
            crate::error::BzrError::config(format!("template '{name}' not found"))
        })?;
        Some(t.clone())
    } else {
        None
    };

    // Merge: CLI flags win over template defaults
    let resolved_product = product
        .clone()
        .or_else(|| tmpl.as_ref().and_then(|t| t.product.clone()))
        .ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--product is required (provide it directly or via a template)".into(),
            )
        })?;
    let resolved_component = component
        .clone()
        .or_else(|| tmpl.as_ref().and_then(|t| t.component.clone()))
        .ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--component is required (provide it directly or via a template)".into(),
            )
        })?;

    let params = CreateBugParams {
        product: resolved_product,
        component: resolved_component,
        summary: summary.clone(),
        version: version
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.version.clone()))
            .unwrap_or_else(|| "unspecified".to_string()),
        description: description
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.description.clone())),
        priority: priority
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.priority.clone())),
        severity: severity
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.severity.clone())),
        assigned_to: assignee
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.assignee.clone())),
        op_sys: op_sys
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.op_sys.clone())),
        rep_platform: rep_platform
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.rep_platform.clone())),
        blocks: blocks.clone(),
        depends_on: depends_on.clone(),
        cc: vec![],
        keywords: vec![],
    };
    let id = client.create_bug(&params).await?;
    output::print_result(
        &ActionResult::created(id, ResourceKind::Bug),
        &format!("Created bug #{id}"),
        format,
    );
    Ok(())
}

async fn handle_clone(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::Clone {
        id,
        summary,
        product,
        component,
        version,
        description,
        priority,
        severity,
        assignee,
        op_sys,
        rep_platform,
        no_comment,
        add_depends_on,
        add_blocks,
        no_cc,
        no_keywords,
    } = action
    else {
        unreachable!()
    };

    // Fetch source bug with all fields needed for cloning
    let source = client.get_bug(id, None, None).await?;

    // Get description from comment #0
    let clone_description = if description.is_some() {
        description.clone()
    } else {
        let comments = client.get_comments_since(source.id, None).await?;
        comments.into_iter().find(|c| c.count == 0).map(|c| c.text)
    };

    let source_product = source.product.ok_or_else(|| {
        crate::error::BzrError::DataIntegrity("source bug missing product field".into())
    })?;
    let source_component = source.component.ok_or_else(|| {
        crate::error::BzrError::DataIntegrity("source bug missing component field".into())
    })?;

    let mut blocks = Vec::new();
    if *add_blocks {
        blocks.push(source.id);
    }
    let mut depends_on = Vec::new();
    if *add_depends_on {
        depends_on.push(source.id);
    }

    let params = CreateBugParams {
        product: product.clone().unwrap_or(source_product),
        component: component.clone().unwrap_or(source_component),
        summary: summary.clone().unwrap_or(source.summary),
        version: version
            .clone()
            .or(source.version)
            .unwrap_or_else(|| "unspecified".to_string()),
        description: clone_description,
        priority: priority.clone().or(source.priority),
        severity: severity.clone().or(source.severity),
        assigned_to: assignee.clone().or(source.assigned_to),
        op_sys: op_sys.clone().or(source.op_sys),
        rep_platform: rep_platform.clone().or(source.rep_platform),
        blocks,
        depends_on,
        cc: if *no_cc { vec![] } else { source.cc },
        keywords: if *no_keywords {
            vec![]
        } else {
            source.keywords
        },
    };

    let new_id = client.create_bug(&params).await?;

    if !*no_comment {
        client
            .add_comment(new_id, &format!("Cloned from bug #{}", source.id))
            .await?;
    }

    output::print_result(
        &ActionResult::created(new_id, ResourceKind::Bug),
        &format!("Cloned bug #{} → #{new_id}", source.id),
        format,
    );
    Ok(())
}

fn build_update_params(action: &BugAction) -> Result<(Vec<u64>, UpdateBugParams)> {
    let BugAction::Update {
        ids,
        status,
        resolution,
        assignee,
        priority,
        severity,
        summary,
        whiteboard,
        flag,
        blocks_add,
        blocks_remove,
        depends_on_add,
        depends_on_remove,
    } = action
    else {
        unreachable!()
    };

    let flags = super::flags::parse_flags(flag)?;
    let params = UpdateBugParams {
        status: status.clone(),
        resolution: resolution.clone(),
        assigned_to: assignee.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        summary: summary.clone(),
        whiteboard: whiteboard.clone(),
        flags,
        blocks: IdListUpdate {
            add: blocks_add.clone(),
            remove: blocks_remove.clone(),
        },
        depends_on: IdListUpdate {
            add: depends_on_add.clone(),
            remove: depends_on_remove.clone(),
        },
    };
    Ok((ids.clone(), params))
}

async fn update_single(
    client: &BugzillaClient,
    id: u64,
    params: &UpdateBugParams,
    format: OutputFormat,
) -> Result<()> {
    client.update_bug(id, params).await?;
    output::print_result(
        &ActionResult::updated(id, ResourceKind::Bug),
        &format!("Updated bug #{id}"),
        format,
    );
    Ok(())
}

fn print_batch_result(batch: &BatchResult, format: OutputFormat) {
    use std::io::Write;
    match format {
        crate::types::OutputFormat::Json => {
            output::print_result(batch, "", format);
        }
        crate::types::OutputFormat::Table => {
            if !batch.succeeded.is_empty() {
                let ids_str: Vec<String> =
                    batch.succeeded.iter().map(|id| format!("#{id}")).collect();
                let _ = writeln!(std::io::stdout(), "Updated bugs: {}", ids_str.join(", "));
            }
            for f in &batch.failed {
                let _ = writeln!(
                    std::io::stderr(),
                    "Failed to update bug #{}: {}",
                    f.id,
                    f.error
                );
            }
        }
    }
}

async fn update_batch(
    client: &BugzillaClient,
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
) -> Result<()> {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for &id in ids {
        match client.update_bug(id, params).await {
            Ok(()) => succeeded.push(id),
            Err(e) => failed.push(BatchFailure {
                id,
                error: e.to_string(),
            }),
        }
    }
    let batch = BatchResult::new(succeeded, failed);
    print_batch_result(&batch, format);
    if !batch.failed.is_empty() {
        return Err(crate::error::BzrError::BatchPartialFailure {
            succeeded: batch.succeeded.len(),
            failed: batch.failed.len(),
        });
    }
    Ok(())
}

async fn handle_update(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let (ids, params) = build_update_params(action)?;
    if ids.len() == 1 {
        update_single(client, ids[0], &params, format).await
    } else {
        update_batch(client, &ids, &params, format).await
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, ResponseTemplate};

    use crate::cli::BugAction;
    use crate::test_helpers::{capture_stdout, extract_json, setup_test_env};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn bug_list_returns_bugs() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 1,
                    "summary": "Test bug",
                    "status": "NEW",
                    "resolution": "",
                    "assigned_to": "nobody@test.com",
                    "priority": "P1",
                    "severity": "normal",
                    "product": "TestProduct",
                    "component": "General",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "last_change_time": "2025-01-01T00:00:00Z"
                }]
            })))
            .mount(&mock)
            .await;

        let action = BugAction::List {
            product: vec![],
            component: vec![],
            status: vec![],
            assignee: vec![],
            creator: vec![],
            priority: vec![],
            severity: vec![],
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["summary"], "Test bug");
        assert_eq!(parsed[0]["status"], "NEW");
        assert_eq!(parsed[0]["product"], "TestProduct");
    }

    #[tokio::test]
    async fn bug_view_returns_detail() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "summary": "Test bug",
                    "status": "NEW",
                    "resolution": "",
                    "assigned_to": "nobody@test.com",
                    "priority": "P1",
                    "severity": "normal",
                    "product": "TestProduct",
                    "component": "General",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "last_change_time": "2025-01-01T00:00:00Z"
                }]
            })))
            .mount(&mock)
            .await;

        let action = BugAction::View {
            id: "42".to_string(),
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["summary"], "Test bug");
        assert_eq!(parsed["assigned_to"], "nobody@test.com");
    }

    #[tokio::test]
    async fn bug_update_sends_put() {
        let (_lock, mock, _tmp) = setup_test_env().await;
        mock_put_bug_ok(&mock, 42).await;

        let action = make_update_action(vec![42]);
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["action"], "updated");
        assert_eq!(parsed["id"], 42);
    }

    fn make_update_action(ids: Vec<u64>) -> BugAction {
        BugAction::Update {
            ids,
            status: Some("RESOLVED".into()),
            resolution: Some("FIXED".into()),
            assignee: None,
            priority: None,
            severity: None,
            summary: None,
            whiteboard: None,
            flag: vec![],
            blocks_add: vec![],
            blocks_remove: vec![],
            depends_on_add: vec![],
            depends_on_remove: vec![],
        }
    }

    async fn mock_put_bug_ok(mock: &wiremock::MockServer, id: u64) {
        Mock::given(method("PUT"))
            .and(path(format!("/rest/bug/{id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"bugs": [{"id": id, "changes": {}}]})),
            )
            .expect(1)
            .mount(mock)
            .await;
    }

    #[tokio::test]
    async fn bug_update_batch_mixed_results() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // First id succeeds, second id fails — exercises update_batch and
        // print_batch_result, including the BatchPartialFailure path.
        mock_put_bug_ok(&mock, 1).await;
        Mock::given(method("PUT"))
            .and(path("/rest/bug/2"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .expect(1)
            .mount(&mock)
            .await;

        let action = make_update_action(vec![1, 2]);
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(matches!(
            result,
            Err(crate::error::BzrError::BatchPartialFailure {
                succeeded: 1,
                failed: 1,
            })
        ));
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["succeeded"], serde_json::json!([1]));
        assert_eq!(parsed["failed"][0]["id"], 2);
    }

    #[tokio::test]
    async fn bug_update_batch_table_format_all_succeed() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // Table format path through print_batch_result with no failures.
        mock_put_bug_ok(&mock, 1).await;
        mock_put_bug_ok(&mock, 2).await;

        let action = make_update_action(vec![1, 2]);
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Table, None)).await;
        assert!(result.is_ok());
        assert!(output.contains("Updated bugs:"));
        assert!(output.contains("#1"));
        assert!(output.contains("#2"));
    }

    #[tokio::test]
    async fn bug_create_sends_post() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("POST"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 99})))
            .expect(1)
            .mount(&mock)
            .await;

        let action = BugAction::Create {
            template: None,
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            summary: "New bug".into(),
            version: Some("unspecified".into()),
            description: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            blocks: vec![],
            depends_on: vec![],
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["action"], "created");
        assert_eq!(parsed["id"], 99);
    }

    #[tokio::test]
    async fn bug_list_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = BugAction::List {
            product: vec![],
            component: vec![],
            status: vec![],
            assignee: vec![],
            creator: vec![],
            priority: vec![],
            severity: vec![],
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("Internal Server Error"),
            "expected HTTP 500 error, got: {err}"
        );
    }

    #[tokio::test]
    async fn bug_view_not_found_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/999999"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 101,
                "message": "Bug #999999 does not exist."
            })))
            .mount(&mock)
            .await;

        let action = BugAction::View {
            id: "999999".to_string(),
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does not exist") || err.contains("101"),
            "expected not-found error, got: {err}"
        );
    }

    #[tokio::test]
    async fn bug_list_malformed_json_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock)
            .await;

        let action = BugAction::List {
            product: vec![],
            component: vec![],
            status: vec![],
            assignee: vec![],
            creator: vec![],
            priority: vec![],
            severity: vec![],
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bug_my_returns_assigned_by_default() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // Mock whoami
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "name": "dev@test.com",
                "real_name": "Dev User",
                "id": 1
            })))
            .mount(&mock)
            .await;

        // Mock assigned-to search
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 10,
                    "summary": "Assigned bug",
                    "status": "NEW",
                    "assigned_to": "dev@test.com",
                    "product": "TestProduct",
                    "component": "General"
                }]
            })))
            .mount(&mock)
            .await;

        let action = BugAction::My {
            created: false,
            cc: false,
            all: false,
            status: vec![],
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "bug my failed: {result:?}");
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed[0]["id"], 10);
        assert_eq!(parsed[0]["summary"], "Assigned bug");
    }

    #[tokio::test]
    async fn bug_my_all_deduplicates() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "name": "dev@test.com",
                "real_name": "Dev User",
                "id": 1
            })))
            .mount(&mock)
            .await;

        // All three searches return the same bug — should appear only once
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "summary": "Shared bug",
                    "status": "NEW",
                    "assigned_to": "dev@test.com",
                    "product": "TestProduct",
                    "component": "General"
                }]
            })))
            .mount(&mock)
            .await;

        let action = BugAction::My {
            created: false,
            cc: false,
            all: true,
            status: vec![],
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "bug my --all failed: {result:?}");
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        let bugs = parsed.as_array().expect("expected JSON array");
        assert_eq!(bugs.len(), 1, "duplicate bug should be deduplicated");
        assert_eq!(bugs[0]["id"], 42);
    }

    #[tokio::test]
    async fn bug_clone_copies_fields() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // Mock get_bug
        Mock::given(method("GET"))
            .and(path("/rest/bug/100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 100,
                    "summary": "Original bug",
                    "status": "NEW",
                    "product": "TestProduct",
                    "component": "General",
                    "version": "2.0",
                    "priority": "P1",
                    "severity": "major",
                    "assigned_to": "dev@test.com",
                    "op_sys": "Linux",
                    "rep_platform": "x86_64",
                    "cc": ["watcher@test.com"],
                    "keywords": ["regression"]
                }]
            })))
            .mount(&mock)
            .await;

        // Mock get_comments (for description)
        Mock::given(method("GET"))
            .and(path("/rest/bug/100/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "100": {
                        "comments": [{
                            "id": 1,
                            "count": 0,
                            "text": "Original description",
                            "creator": "dev@test.com",
                            "creation_time": "2025-01-01T00:00:00Z"
                        }]
                    }
                }
            })))
            .mount(&mock)
            .await;

        // Mock create_bug
        Mock::given(method("POST"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 200})))
            .expect(1)
            .mount(&mock)
            .await;

        // Mock add_comment (for "Cloned from" comment)
        Mock::given(method("POST"))
            .and(path("/rest/bug/200/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 300})))
            .expect(1)
            .mount(&mock)
            .await;

        let action = BugAction::Clone {
            id: "100".to_string(),
            summary: None,
            product: None,
            component: None,
            version: None,
            description: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            no_comment: false,
            add_depends_on: false,
            add_blocks: false,
            no_cc: false,
            no_keywords: false,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "bug clone failed: {result:?}");
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 200);
        assert_eq!(parsed["action"], "created");
    }

    #[tokio::test]
    async fn bug_clone_no_comment_skips_comment() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 100,
                    "summary": "Original bug",
                    "status": "NEW",
                    "product": "TestProduct",
                    "component": "General",
                    "version": "1.0",
                    "cc": [],
                    "keywords": []
                }]
            })))
            .mount(&mock)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/100/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "100": {
                        "comments": [{
                            "id": 1,
                            "count": 0,
                            "text": "Description",
                            "creator": "dev@test.com",
                            "creation_time": "2025-01-01T00:00:00Z"
                        }]
                    }
                }
            })))
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 201})))
            .expect(1)
            .mount(&mock)
            .await;

        // No comment mock — if comment is posted, the test will fail because there's no mock
        Mock::given(method("POST"))
            .and(path("/rest/bug/201/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 301})))
            .expect(0)
            .mount(&mock)
            .await;

        let action = BugAction::Clone {
            id: "100".to_string(),
            summary: None,
            product: None,
            component: None,
            version: None,
            description: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            no_comment: true,
            add_depends_on: false,
            add_blocks: false,
            no_cc: false,
            no_keywords: false,
        };
        let (result, _output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "bug clone --no-comment failed: {result:?}");
    }

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

        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "from-url search failed: {result:?}");
        let parsed: serde_json::Value = extract_json(&output);
        assert_eq!(parsed[0]["id"], 1);
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

        let (result, _) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
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

        let (result, _output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
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
        let (result, _output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
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
        let (result, _output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no name provided for --save-as"),
            "unexpected error: {err}"
        );
    }
}
