use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::{CreateBugParams, OutputFormat};

pub(super) async fn handle(
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

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, ResponseTemplate};

    use crate::cli::BugAction;
    use crate::test_helpers::{capture_stdout, setup_test_env};
    use crate::types::OutputFormat;

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

        // Mock get_comments (for description). Returns count=1 BEFORE
        // count=0 so that picking the wrong one produces the wrong
        // description text.
        Mock::given(method("GET"))
            .and(path("/rest/bug/100/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "100": {
                        "comments": [
                            {
                                "id": 2,
                                "count": 1,
                                "text": "Follow-up reply",
                                "creator": "dev@test.com",
                                "creation_time": "2025-01-02T00:00:00Z"
                            },
                            {
                                "id": 1,
                                "count": 0,
                                "text": "Original description",
                                "creator": "dev@test.com",
                                "creation_time": "2025-01-01T00:00:00Z"
                            }
                        ]
                    }
                }
            })))
            .mount(&mock)
            .await;

        // Mock create_bug — assert the cloned description is comment #0,
        // not the follow-up.
        Mock::given(method("POST"))
            .and(path("/rest/bug"))
            .and(body_partial_json(serde_json::json!({
                "description": "Original description"
            })))
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
        let (result, output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
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
        let (result, _output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok(), "bug clone --no-comment failed: {result:?}");
    }
}
