use crate::types::template::BugTemplate;
use clap::{Args, Subcommand};

/// The default-field flags shared by `template save` and `template update`.
/// Grouped into one flattened struct so the set is defined once and the two
/// variants cannot drift apart.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct TemplateFields {
    /// Default product
    #[arg(long)]
    pub product: Option<String>,
    /// Default component
    #[arg(long)]
    pub component: Option<String>,
    /// Default version
    #[arg(long)]
    pub version: Option<String>,
    /// Default priority
    #[arg(long)]
    pub priority: Option<String>,
    /// Default severity
    #[arg(long)]
    pub severity: Option<String>,
    /// Default assignee
    #[arg(long)]
    pub assignee: Option<String>,
    /// Default operating system
    #[arg(long)]
    pub op_sys: Option<String>,
    /// Default hardware platform
    #[arg(long)]
    pub rep_platform: Option<String>,
    /// Default description
    #[arg(long)]
    pub description: Option<String>,
    /// Default URL
    #[arg(long)]
    pub url: Option<String>,
    /// Default whiteboard
    #[arg(long)]
    pub whiteboard: Option<String>,
    /// Default target milestone
    #[arg(long)]
    pub target_milestone: Option<String>,
    /// Default deadline (`YYYY-MM-DD`)
    #[arg(long, value_name = "DATE")]
    pub deadline: Option<String>,
    /// Default CC addresses
    #[arg(long, value_delimiter = ',')]
    pub cc: Vec<String>,
    /// Default keywords
    #[arg(long, value_delimiter = ',')]
    pub keywords: Vec<String>,
    /// Default groups
    #[arg(long, value_delimiter = ',')]
    pub groups: Vec<String>,
    /// Default Bugzilla flag updates (for example `review?`)
    #[arg(long)]
    pub flag: Vec<String>,
}

impl TemplateFields {
    /// Return true when no template-update field flags were supplied.
    #[must_use]
    pub fn is_empty_change(&self) -> bool {
        self.to_template().is_empty()
    }

    /// Build a `BugTemplate` from these flags (used by `template save`).
    #[must_use]
    pub fn to_template(&self) -> BugTemplate {
        BugTemplate {
            product: self.product.clone(),
            component: self.component.clone(),
            version: self.version.clone(),
            priority: self.priority.clone(),
            severity: self.severity.clone(),
            assignee: self.assignee.clone(),
            op_sys: self.op_sys.clone(),
            rep_platform: self.rep_platform.clone(),
            description: self.description.clone(),
            url: self.url.clone(),
            whiteboard: self.whiteboard.clone(),
            target_milestone: self.target_milestone.clone(),
            deadline: self.deadline.clone(),
            cc: self.cc.clone(),
            keywords: self.keywords.clone(),
            groups: self.groups.clone(),
            flags: self.flag.clone(),
        }
    }
}

/// Arguments for `template update`.
#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    /// Template name
    pub name: String,
    #[command(flatten)]
    pub fields: TemplateFields,
    /// Reset a field to unset (repeatable).
    #[arg(long, value_name = "FIELD")]
    pub clear: Vec<String>,
}

#[derive(Subcommand)]
pub(crate) enum TemplateAction {
    /// Save (or replace) a named bug-creation template.
    ///
    /// Templates store reusable defaults for `bzr bug create`. At
    /// least one default field must be supplied. Supported defaults
    /// include the routing fields (`--product`, `--component`,
    /// `--version`, `--priority`, `--severity`, `--assignee`,
    /// `--op-sys`, `--rep-platform`, and `--description`) plus
    /// create metadata (`--url`, `--whiteboard`,
    /// `--target-milestone`, `--deadline`, `--cc`, `--keywords`,
    /// `--groups`, and `--flag`). Empty templates are rejected with
    /// exit code 7 (input validation).
    ///
    /// Saving over an existing template name replaces it. Templates
    /// are stored locally in `~/.config/bzr/config.toml` and never
    /// sent to the server -- they are pure CLI ergonomics.
    ///
    /// Examples:
    ///
    ///   bzr template save security-bug --product Security \
    ///     --component Vulnerabilities --severity critical
    ///   bzr template save fedora-kernel --product Fedora \
    ///     --component kernel --priority high
    ///   bzr template save security-routing --product Security \
    ///     --component Triage --cc triage@example.com --flag review?
    ///
    /// See bzr-template-show(1) to inspect a template,
    /// bzr-template-delete(1) to remove one, and bzr-bug-create(1)
    /// for the `--template` flag that applies one.
    #[command(verbatim_doc_comment)]
    Save {
        /// Template name
        name: String,
        #[command(flatten)]
        fields: TemplateFields,
    },

    /// Update fields of an existing template in place.
    ///
    /// Merges the supplied flags into the named template without
    /// re-specifying the rest: a flag replaces that field, an
    /// omitted flag leaves it unchanged. Use `--clear <field>`
    /// (repeatable) to reset a field to unset. Valid names are
    /// `product`, `component`, `version`, `priority`, `severity`,
    /// `assignee`, `op-sys`, `rep-platform`, `description`, `url`,
    /// `whiteboard`, `target-milestone`, `deadline`, `cc`,
    /// `keywords`, `groups`, `flag`, and `flags`. At least one
    /// change (a field flag or `--clear`) is required, and the result
    /// must keep at least one field set (a fully-cleared template is
    /// rejected, exit 7). If a field is both set and cleared in one
    /// call, `--clear` wins.
    ///
    /// Examples:
    ///
    ///   bzr template update security-bug --severity blocker
    ///   bzr template update security-bug --clear assignee
    ///   bzr template update security-bug --cc triage@example.com
    ///
    /// See bzr-template-save(1) to create one and
    /// bzr-template-show(1) to inspect the result.
    #[command(verbatim_doc_comment)]
    Update(UpdateArgs),

    /// List all saved templates.
    ///
    /// Prints each template's name and a short summary of its
    /// stored defaults. Use `--json` for a structured listing.
    ///
    /// Examples:
    ///
    ///   bzr template list
    ///   bzr template list --json | jq -r '.data | keys[]'
    ///
    /// See bzr-template-show(1) for the full parameters of one
    /// template.
    #[command(verbatim_doc_comment)]
    List,

    /// Show the stored defaults of one saved template.
    ///
    /// Prints the template's name and every default field it
    /// stores. Useful for verifying what `bzr bug create
    /// --template <name>` will pre-fill.
    ///
    /// Examples:
    ///
    ///   bzr template show security-bug
    ///   bzr template show security-bug --json
    ///
    /// See bzr-template-list(1) for the inventory and
    /// bzr-bug-create(1) to apply a template.
    #[command(verbatim_doc_comment)]
    Show {
        /// Template name
        name: String,
    },

    /// Delete a saved template.
    ///
    /// Removes the named template from the local config. Does not
    /// prompt for confirmation -- if recovery is needed, restore
    /// the previous `~/.config/bzr/config.toml` from backup or
    /// re-run `bzr template save`.
    ///
    /// Examples:
    ///
    ///   bzr template delete security-bug
    ///
    /// See bzr-template-list(1) to verify the template exists
    /// first.
    #[command(verbatim_doc_comment)]
    Delete {
        /// Template name
        name: String,
    },
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
