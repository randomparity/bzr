use std::collections::HashMap;
use std::io::Write;

use crate::types::{BugTemplate, OutputFormat};

use super::formatting::{write_field, write_formatted, write_json, write_optional_field};

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

pub fn write_template_saved<W: Write + ?Sized>(
    name: &str,
    verb: &str,
    format: OutputFormat,
    out: &mut W,
) {
    match format {
        OutputFormat::Json => {
            write_json(
                &serde_json::json!({"name": name, "action": verb.to_lowercase()}),
                out,
            );
        }
        OutputFormat::Table => {
            let _ = writeln!(out, "{}", template_saved_message(name, verb));
        }
    }
}

pub fn write_template_list<W: Write + ?Sized, S: ::std::hash::BuildHasher>(
    templates: &HashMap<String, BugTemplate, S>,
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(templates, format, out, |templates, out| {
        if templates.is_empty() {
            let _ = writeln!(out, "No templates configured.");
            return;
        }
        let mut names: Vec<&str> = templates.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            let _ = writeln!(out, "{}", template_summary_line(name, &templates[name]));
        }
    });
}

pub fn write_template_detail<W: Write + ?Sized>(
    name: &str,
    template: &BugTemplate,
    format: OutputFormat,
    out: &mut W,
) {
    #[derive(serde::Serialize)]
    struct TemplateView<'a> {
        name: &'a str,
        #[serde(flatten)]
        template: &'a BugTemplate,
    }

    let view = TemplateView { name, template };
    write_formatted(&view, format, out, |view, out| {
        write_field(out, "Name", view.name);
        write_optional_field(out, "Product", view.template.product.as_deref());
        write_optional_field(out, "Component", view.template.component.as_deref());
        write_optional_field(out, "Version", view.template.version.as_deref());
        write_optional_field(out, "Priority", view.template.priority.as_deref());
        write_optional_field(out, "Severity", view.template.severity.as_deref());
        write_optional_field(out, "Assignee", view.template.assignee.as_deref());
        write_optional_field(out, "OS", view.template.op_sys.as_deref());
        write_optional_field(out, "Platform", view.template.rep_platform.as_deref());
        write_optional_field(out, "Description", view.template.description.as_deref());
    });
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
