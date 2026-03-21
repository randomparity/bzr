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
