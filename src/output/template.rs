use std::collections::HashMap;

use crate::config::BugTemplate;
use crate::types::OutputFormat;

use super::formatting::{print_field, print_formatted, print_json, print_optional_field};

pub fn print_template_saved(name: &str, verb: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            print_json(&serde_json::json!({"name": name, "action": verb.to_lowercase()}));
        }
        OutputFormat::Table => {
            println!("{verb} template '{name}'");
        }
    }
}

pub fn print_template_list(templates: &HashMap<String, BugTemplate>, format: OutputFormat) {
    print_formatted(templates, format, |templates| {
        if templates.is_empty() {
            println!("No templates configured.");
            return;
        }
        let mut names: Vec<&str> = templates.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let tmpl = &templates[name];
            let mut parts = Vec::new();
            if let Some(p) = &tmpl.product {
                parts.push(format!("product={p}"));
            }
            if let Some(c) = &tmpl.component {
                parts.push(format!("component={c}"));
            }
            if let Some(p) = &tmpl.priority {
                parts.push(format!("priority={p}"));
            }
            if let Some(s) = &tmpl.severity {
                parts.push(format!("severity={s}"));
            }
            let summary = if parts.is_empty() {
                String::new()
            } else {
                format!(" ({})", parts.join(", "))
            };
            println!("{name}{summary}");
        }
    });
}

pub fn print_template_detail(name: &str, template: &BugTemplate, format: OutputFormat) {
    #[derive(serde::Serialize)]
    struct TemplateView<'a> {
        name: &'a str,
        #[serde(flatten)]
        template: &'a BugTemplate,
    }

    let view = TemplateView { name, template };
    print_formatted(&view, format, |view| {
        print_field("Name", view.name);
        print_optional_field("Product", view.template.product.as_deref());
        print_optional_field("Component", view.template.component.as_deref());
        print_optional_field("Version", view.template.version.as_deref());
        print_optional_field("Priority", view.template.priority.as_deref());
        print_optional_field("Severity", view.template.severity.as_deref());
        print_optional_field("Assignee", view.template.assignee.as_deref());
        print_optional_field("OS", view.template.op_sys.as_deref());
        print_optional_field("Platform", view.template.rep_platform.as_deref());
        print_optional_field("Description", view.template.description.as_deref());
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_template() -> BugTemplate {
        BugTemplate {
            product: Some("Widget".into()),
            component: Some("Backend".into()),
            version: None,
            priority: Some("P1".into()),
            severity: Some("major".into()),
            assignee: None,
            op_sys: None,
            rep_platform: None,
            description: Some("Default description".into()),
        }
    }

    #[test]
    fn template_saved_json() {
        let json = serde_json::json!({"name": "my-tmpl", "action": "saved"});
        let parsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&json).unwrap()).unwrap();
        assert_eq!(parsed["name"], "my-tmpl");
        assert_eq!(parsed["action"], "saved");
    }

    #[test]
    fn template_list_json_serializes() {
        let mut templates: HashMap<String, BugTemplate> = HashMap::new();
        templates.insert("default".into(), make_template());
        let json = serde_json::to_string_pretty(&templates).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["default"].is_object());
        assert_eq!(parsed["default"]["product"], "Widget");
        assert_eq!(parsed["default"]["component"], "Backend");
    }

    #[test]
    fn template_detail_json_with_flatten() {
        let template = make_template();
        #[derive(serde::Serialize)]
        struct TemplateView<'a> {
            name: &'a str,
            #[serde(flatten)]
            template: &'a BugTemplate,
        }
        let view = TemplateView {
            name: "test-tmpl",
            template: &template,
        };
        let json = serde_json::to_string_pretty(&view).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "test-tmpl");
        assert_eq!(parsed["product"], "Widget");
        assert_eq!(parsed["priority"], "P1");
        assert!(parsed["version"].is_null());
    }

    #[test]
    fn template_empty_fields_omitted_in_json() {
        let template = BugTemplate {
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
        let json = serde_json::to_string(&template).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.as_object().unwrap().is_empty());
    }
}
