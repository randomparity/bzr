use std::collections::HashMap;
use std::io::{self, Write as _};

use crate::types::{BugTemplate, OutputFormat};

use super::formatting::{print_field, print_formatted, print_json, print_optional_field};

fn template_saved_message(name: &str, verb: &str) -> String {
    format!("{verb} template '{name}'")
}

fn template_summary_line(name: &str, tmpl: &BugTemplate) -> String {
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
    if parts.is_empty() {
        name.to_string()
    } else {
        format!("{name} ({})", parts.join(", "))
    }
}

pub fn print_template_saved(name: &str, verb: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            print_json(&serde_json::json!({"name": name, "action": verb.to_lowercase()}));
        }
        OutputFormat::Table => {
            let _ = writeln!(io::stdout(), "{}", template_saved_message(name, verb));
        }
    }
}

pub fn print_template_list(templates: &HashMap<String, BugTemplate>, format: OutputFormat) {
    print_formatted(templates, format, |templates| {
        if templates.is_empty() {
            let _ = writeln!(io::stdout(), "No templates configured.");
            return;
        }
        let mut names: Vec<&str> = templates.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let _ = writeln!(
                io::stdout(),
                "{}",
                template_summary_line(name, &templates[name])
            );
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
#[path = "template_tests.rs"]
mod tests;
