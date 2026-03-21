use clap::Subcommand;

#[derive(Subcommand)]
#[expect(
    clippy::large_enum_variant,
    reason = "only constructed once from CLI args"
)]
pub enum TemplateAction {
    /// Save a named bug template
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
    /// List all saved templates
    List,
    /// Show details of a template
    Show {
        /// Template name
        name: String,
    },
    /// Delete a template
    Delete {
        /// Template name
        name: String,
    },
}
