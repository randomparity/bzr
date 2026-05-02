use clap::Subcommand;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum ComponentAction {
    /// Create a new component within a product (admin only).
    ///
    /// Requires Bugzilla admin permissions on the target product.
    /// All four flags are required: `--product`, `--name`,
    /// `--description`, and `--default-assignee`. The default
    /// assignee must be an existing user account; the component
    /// will appear in `bzr product view <product>` once created.
    ///
    /// Components belong to exactly one product -- moving a
    /// component across products is not supported by the Bugzilla
    /// REST API.
    ///
    /// Examples:
    ///
    ///   bzr component create --product MyApp --name Backend \
    ///     --description "Backend services" \
    ///     --default-assignee dev@example.com
    ///   bzr component create --product MyApp --name Frontend \
    ///     --description "UI / web client" \
    ///     --default-assignee ui-team@example.com
    ///
    /// See bzr-product-view(1) to verify the new component appears
    /// and bzr-component-update(1) to modify it later.
    #[command(verbatim_doc_comment)]
    Create {
        /// Product name
        #[arg(long)]
        product: String,
        /// Component name
        #[arg(long)]
        name: String,
        /// Component description
        #[arg(long)]
        description: String,
        /// Default assignee email
        #[arg(long)]
        default_assignee: String,
    },

    /// Update an existing component by ID (admin only).
    ///
    /// Requires Bugzilla admin permissions. Pass any of the flags
    /// to change that property: `--name`, `--description`,
    /// `--default-assignee`. Only the supplied fields are modified.
    ///
    /// The numeric `<id>` is the component ID, not the name.
    /// Discover IDs via `bzr product view <product>` (look for
    /// `components[].id`).
    ///
    /// Examples:
    ///
    ///   bzr component update 42 --description "Updated description"
    ///   bzr component update 42 --default-assignee newowner@example.com
    ///
    /// See bzr-component-create(1) for new components and
    /// bzr-product-view(1) to find component IDs.
    #[command(verbatim_doc_comment)]
    Update {
        /// Component ID
        id: u64,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New default assignee
        #[arg(long)]
        default_assignee: Option<String>,
    },
}
