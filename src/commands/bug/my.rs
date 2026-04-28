use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::{OutputFormat, SearchParams};

pub(super) async fn handle(
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

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use crate::cli::BugAction;
    use crate::test_helpers::{capture_stdout, setup_test_env};
    use crate::types::OutputFormat;

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
        let (result, output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
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
        let (result, output) = capture_stdout(crate::commands::bug::execute(
            &action,
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok(), "bug my --all failed: {result:?}");
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        let bugs = parsed.as_array().expect("expected JSON array");
        assert_eq!(bugs.len(), 1, "duplicate bug should be deduplicated");
        assert_eq!(bugs[0]["id"], 42);
    }
}
