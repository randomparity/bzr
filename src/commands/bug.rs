use crate::cli::BugAction;
use crate::error::Result;
use crate::output::{self, ActionResult, BatchFailure, BatchResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateBugParams, IdListUpdate, SearchParams, UpdateBugParams};

#[expect(
    clippy::too_many_lines,
    reason = "single match dispatch over many action variants"
)]
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        BugAction::List {
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
        } => {
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
        }
        BugAction::View {
            id,
            fields,
            exclude_fields,
        } => {
            let bug = client
                .get_bug(id, fields.as_deref(), exclude_fields.as_deref())
                .await?;
            output::print_bug_detail(&bug, format);
        }
        BugAction::History { id, since } => {
            let history = client.get_bug_history_since(*id, since.as_deref()).await?;
            if history.is_empty() {
                #[expect(clippy::print_stdout)]
                {
                    println!("No history for bug #{id}.");
                }
            } else {
                output::print_history(&history, format);
            }
        }
        BugAction::Search {
            query,
            limit,
            fields,
            exclude_fields,
        } => {
            let params = SearchParams {
                quicksearch: Some(query.clone()),
                limit: Some(*limit),
                include_fields: fields.clone(),
                exclude_fields: exclude_fields.clone(),
                ..Default::default()
            };
            let bugs = client.search_bugs(&params).await?;
            output::print_bugs(&bugs, format);
        }
        BugAction::My {
            created,
            cc,
            all,
            status,
            limit,
            fields,
            exclude_fields,
        } => {
            let whoami = client.whoami().await?;
            let email = whoami.name;
            let mut all_bugs: Vec<crate::types::Bug> = Vec::new();
            let mut seen_ids = std::collections::HashSet::new();

            // Determine which searches to run
            let search_assigned = *all || (!created && !cc);
            let search_created = *all || *created;
            let search_cc = *all || *cc;

            if search_assigned {
                let params = SearchParams {
                    assigned_to: Some(email.clone()),
                    status: status.clone(),
                    limit: Some(*limit),
                    include_fields: fields.clone(),
                    exclude_fields: exclude_fields.clone(),
                    ..Default::default()
                };
                for bug in client.search_bugs(&params).await? {
                    if seen_ids.insert(bug.id) {
                        all_bugs.push(bug);
                    }
                }
            }
            if search_created {
                let params = SearchParams {
                    creator: Some(email.clone()),
                    status: status.clone(),
                    limit: Some(*limit),
                    include_fields: fields.clone(),
                    exclude_fields: exclude_fields.clone(),
                    ..Default::default()
                };
                for bug in client.search_bugs(&params).await? {
                    if seen_ids.insert(bug.id) {
                        all_bugs.push(bug);
                    }
                }
            }
            if search_cc {
                let params = SearchParams {
                    cc: Some(email.clone()),
                    status: status.clone(),
                    limit: Some(*limit),
                    include_fields: fields.clone(),
                    exclude_fields: exclude_fields.clone(),
                    ..Default::default()
                };
                for bug in client.search_bugs(&params).await? {
                    if seen_ids.insert(bug.id) {
                        all_bugs.push(bug);
                    }
                }
            }

            output::print_bugs(&all_bugs, format);
        }
        BugAction::Create {
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
        } => {
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
        }
        BugAction::Clone {
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
        } => {
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
        }
        BugAction::Update {
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
        } => {
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

            if ids.len() == 1 {
                // Single bug update — original behavior
                let id = ids[0];
                client.update_bug(id, &params).await?;
                output::print_result(
                    &ActionResult::updated(id, ResourceKind::Bug),
                    &format!("Updated bug #{id}"),
                    format,
                );
            } else {
                // Batch update — continue on failure
                let mut succeeded = Vec::new();
                let mut failed = Vec::new();
                for &id in ids {
                    match client.update_bug(id, &params).await {
                        Ok(()) => succeeded.push(id),
                        Err(e) => failed.push(BatchFailure {
                            id,
                            error: e.to_string(),
                        }),
                    }
                }
                let has_failures = !failed.is_empty();
                let batch = BatchResult::new(succeeded.clone(), failed);
                #[expect(clippy::print_stdout, clippy::print_stderr)]
                {
                    match format {
                        crate::types::OutputFormat::Json => {
                            output::print_result(&batch, "", format);
                        }
                        crate::types::OutputFormat::Table => {
                            if !succeeded.is_empty() {
                                let ids_str: Vec<String> =
                                    succeeded.iter().map(|id| format!("#{id}")).collect();
                                println!("Updated bugs: {}", ids_str.join(", "));
                            }
                            for f in &batch.failed {
                                eprintln!("Failed to update bug #{}: {}", f.id, f.error);
                            }
                        }
                    }
                }
                if has_failures {
                    return Err(crate::error::BzrError::BatchPartialFailure {
                        succeeded: succeeded.len(),
                        failed: batch.failed.len(),
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::{capture_stdout, setup_test_env};
    use crate::cli::BugAction;
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
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
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
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["summary"], "Test bug");
        assert_eq!(parsed["assigned_to"], "nobody@test.com");
    }

    #[tokio::test]
    async fn bug_update_sends_put() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("PUT"))
            .and(path("/rest/bug/42"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let action = BugAction::Update {
            ids: vec![42],
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
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed["action"], "updated");
        assert_eq!(parsed["id"], 42);
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
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
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
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
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
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
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
            status: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "bug my failed: {result:?}");
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
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
            status: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "bug my --all failed: {result:?}");
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
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
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
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
}
