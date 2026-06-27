use serde::{Deserialize, Serialize};

/// A named set of default field values for bug creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BugTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_sys: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rep_platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
}

const CLEARABLE_FIELDS: &[&str] = &[
    "product",
    "component",
    "version",
    "priority",
    "severity",
    "assignee",
    "op-sys",
    "rep-platform",
    "description",
    "url",
    "whiteboard",
    "target-milestone",
    "deadline",
    "cc",
    "keywords",
    "groups",
    "flag",
    "flags",
];

impl BugTemplate {
    /// Return true when the template has no stored bug-create defaults.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.product.is_none()
            && self.component.is_none()
            && self.version.is_none()
            && self.priority.is_none()
            && self.severity.is_none()
            && self.assignee.is_none()
            && self.op_sys.is_none()
            && self.rep_platform.is_none()
            && self.description.is_none()
            && self.url.is_none()
            && self.whiteboard.is_none()
            && self.target_milestone.is_none()
            && self.deadline.is_none()
            && self.cc.is_empty()
            && self.keywords.is_empty()
            && self.groups.is_empty()
            && self.flags.is_empty()
    }

    /// Merge supplied template fields into this template.
    ///
    /// Present scalar fields replace existing values. Non-empty list fields
    /// replace existing lists. Missing scalars and empty lists leave the target
    /// unchanged, matching CLI update semantics.
    pub(crate) fn merge_from(&mut self, fields: &Self) {
        merge_scalar(&mut self.product, fields.product.as_ref());
        merge_scalar(&mut self.component, fields.component.as_ref());
        merge_scalar(&mut self.version, fields.version.as_ref());
        merge_scalar(&mut self.priority, fields.priority.as_ref());
        merge_scalar(&mut self.severity, fields.severity.as_ref());
        merge_scalar(&mut self.assignee, fields.assignee.as_ref());
        merge_scalar(&mut self.op_sys, fields.op_sys.as_ref());
        merge_scalar(&mut self.rep_platform, fields.rep_platform.as_ref());
        merge_scalar(&mut self.description, fields.description.as_ref());
        merge_scalar(&mut self.url, fields.url.as_ref());
        merge_scalar(&mut self.whiteboard, fields.whiteboard.as_ref());
        merge_scalar(&mut self.target_milestone, fields.target_milestone.as_ref());
        merge_scalar(&mut self.deadline, fields.deadline.as_ref());
        merge_list(&mut self.cc, &fields.cc);
        merge_list(&mut self.keywords, &fields.keywords);
        merge_list(&mut self.groups, &fields.groups);
        merge_list(&mut self.flags, &fields.flags);
    }

    /// Clear one template field by its CLI field name.
    ///
    /// Returns false when the field name is not supported.
    pub(crate) fn clear_field(&mut self, field: &str) -> bool {
        match field {
            "product" => self.product = None,
            "component" => self.component = None,
            "version" => self.version = None,
            "priority" => self.priority = None,
            "severity" => self.severity = None,
            "assignee" => self.assignee = None,
            "op-sys" => self.op_sys = None,
            "rep-platform" => self.rep_platform = None,
            "description" => self.description = None,
            "url" => self.url = None,
            "whiteboard" => self.whiteboard = None,
            "target-milestone" => self.target_milestone = None,
            "deadline" => self.deadline = None,
            "cc" => self.cc.clear(),
            "keywords" => self.keywords.clear(),
            "groups" => self.groups.clear(),
            "flag" | "flags" => self.flags.clear(),
            _ => return false,
        }
        true
    }

    /// Return the clearable field names accepted by `template update --clear`.
    #[must_use]
    pub(crate) fn clearable_fields() -> &'static [&'static str] {
        CLEARABLE_FIELDS
    }
}

fn merge_scalar(target: &mut Option<String>, value: Option<&String>) {
    if let Some(value) = value {
        *target = Some(value.clone());
    }
}

fn merge_list(target: &mut Vec<String>, value: &[String]) {
    if !value.is_empty() {
        *target = value.to_vec();
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
