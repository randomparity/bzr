//! Template management commands.
//!
//! Template operations are pure local file I/O — no network client needed.

use crate::cli::TemplateAction;
use crate::config::{BugTemplate, Config};
use crate::error::{BzrError, Result};
use crate::output;
use crate::types::OutputFormat;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub async fn execute(action: &TemplateAction, format: OutputFormat) -> Result<()> {
    match action {
        TemplateAction::Save {
            name,
            product,
            component,
            version,
            priority,
            severity,
            assignee,
            op_sys,
            rep_platform,
            description,
        } => {
            let template = BugTemplate {
                product: product.clone(),
                component: component.clone(),
                version: version.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                assignee: assignee.clone(),
                op_sys: op_sys.clone(),
                rep_platform: rep_platform.clone(),
                description: description.clone(),
            };

            // Require at least one field to be set
            if template.product.is_none()
                && template.component.is_none()
                && template.version.is_none()
                && template.priority.is_none()
                && template.severity.is_none()
                && template.assignee.is_none()
                && template.op_sys.is_none()
                && template.rep_platform.is_none()
                && template.description.is_none()
            {
                return Err(BzrError::InputValidation(
                    "template must have at least one field set".into(),
                ));
            }

            let mut config = Config::load()?;
            let is_update = config.templates.contains_key(name.as_str());
            config.templates.insert(name.clone(), template);
            config.save()?;

            let verb = if is_update { "Updated" } else { "Saved" };
            output::print_template_saved(name, verb, format);
        }
        TemplateAction::List => {
            let config = Config::load()?;
            output::print_template_list(&config.templates, format);
        }
        TemplateAction::Show { name } => {
            let config = Config::load()?;
            let template = config
                .templates
                .get(name.as_str())
                .ok_or_else(|| BzrError::config(format!("template '{name}' not found")))?;
            output::print_template_detail(name, template, format);
        }
        TemplateAction::Delete { name } => {
            let mut config = Config::load()?;
            if config.templates.remove(name.as_str()).is_none() {
                return Err(BzrError::config(format!("template '{name}' not found")));
            }
            config.save()?;

            #[expect(clippy::print_stdout)]
            match format {
                OutputFormat::Json => {
                    output::print_result(
                        &serde_json::json!({"name": name, "action": "deleted"}),
                        "",
                        format,
                    );
                }
                OutputFormat::Table => {
                    println!("Deleted template '{name}'");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::cli::TemplateAction;
    use crate::test_helpers::{capture_stdout, setup_test_env};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn template_save_and_show() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        // Save a template
        let action = TemplateAction::Save {
            name: "test-tmpl".into(),
            product: Some("TestProduct".into()),
            component: Some("General".into()),
            version: None,
            priority: Some("P1".into()),
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            description: None,
        };
        let (result, _output) = capture_stdout(super::execute(&action, OutputFormat::Json)).await;
        assert!(result.is_ok(), "template save failed: {result:?}");

        // Show the saved template
        let action = TemplateAction::Show {
            name: "test-tmpl".into(),
        };
        let (result, output) = capture_stdout(super::execute(&action, OutputFormat::Json)).await;
        assert!(result.is_ok(), "template show failed: {result:?}");
        let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["name"], "test-tmpl");
        assert_eq!(parsed["product"], "TestProduct");
        assert_eq!(parsed["priority"], "P1");
    }

    #[tokio::test]
    async fn template_save_requires_field() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = TemplateAction::Save {
            name: "empty-tmpl".into(),
            product: None,
            component: None,
            version: None,
            priority: None,
            severity: None,
            assignee: None,
            op_sys: None,
            rep_platform: None,
            description: None,
        };
        let result = super::execute(&action, OutputFormat::Json).await;
        assert!(result.is_err(), "saving empty template should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("at least one field"),
            "expected validation error, got: {err}"
        );
    }

    #[tokio::test]
    async fn template_delete_unknown_errors() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = TemplateAction::Delete {
            name: "nonexistent".into(),
        };
        let result = super::execute(&action, OutputFormat::Json).await;
        assert!(result.is_err(), "deleting unknown template should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found"),
            "expected not-found error, got: {err}"
        );
    }

    #[tokio::test]
    async fn template_list_empty() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = TemplateAction::List;
        let (result, _output) = capture_stdout(super::execute(&action, OutputFormat::Json)).await;
        assert!(result.is_ok(), "template list failed: {result:?}");
    }
}
