use clap::Subcommand;

#[derive(Subcommand)]
#[expect(
    clippy::large_enum_variant,
    reason = "only constructed once from CLI args"
)]
pub enum TemplateAction {
    /// Save (or replace) a named bug-creation template.
    ///
    /// Templates store reusable defaults for `bzr bug create`. At
    /// least one default field must be supplied (`--product`,
    /// `--component`, `--version`, `--priority`, `--severity`,
    /// `--assignee`, `--op-sys`, `--rep-platform`, or
    /// `--description`); empty templates are rejected with exit
    /// code 7 (input validation).
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
    ///
    /// See bzr-template-show(1) to inspect a template,
    /// bzr-template-delete(1) to remove one, and bzr-bug-create(1)
    /// for the `--template` flag that applies one.
    #[command(verbatim_doc_comment)]
    Save {
        /// Template name
        name: String,
        /// Default product
        #[arg(long)]
        product: Option<String>,
        /// Default component
        #[arg(long)]
        component: Option<String>,
        /// Default version
        #[arg(long)]
        version: Option<String>,
        /// Default priority
        #[arg(long)]
        priority: Option<String>,
        /// Default severity
        #[arg(long)]
        severity: Option<String>,
        /// Default assignee
        #[arg(long)]
        assignee: Option<String>,
        /// Default operating system
        #[arg(long)]
        op_sys: Option<String>,
        /// Default hardware platform
        #[arg(long)]
        rep_platform: Option<String>,
        /// Default description
        #[arg(long)]
        description: Option<String>,
    },

    /// List all saved templates.
    ///
    /// Prints each template's name and a short summary of its
    /// stored defaults. Use `--json` for a structured listing.
    ///
    /// Examples:
    ///
    ///   bzr template list
    ///   bzr template list --json | jq '.templates[].name'
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
