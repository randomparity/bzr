use clap::Subcommand;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum ComponentAction {
    /// List a product's components.
    ///
    /// Reads the product's component set (the same data shown by
    /// `bzr product view <product>`) and prints each component's ID,
    /// name, description, default assignee, and active flag. JSON output
    /// is the full component array.
    ///
    /// Examples:
    ///
    ///   bzr component list --product MyApp
    ///   bzr --json component list --product MyApp | jq '.[].name'
    ///
    /// See bzr-component-view(1) for a single component's detail.
    #[command(verbatim_doc_comment)]
    List {
        /// Product name
        #[arg(long)]
        product: String,
    },

    /// View a single component within a product.
    ///
    /// Looks the component up by exact name within the given product and
    /// prints its ID, description, default assignee, and active flag. JSON
    /// output is the `Component` object. Errors if the product has no
    /// component with that name.
    ///
    /// Examples:
    ///
    ///   bzr component view MyApp Backend
    ///   bzr --json component view MyApp Backend
    ///
    /// See bzr-component-list(1) to enumerate a product's components.
    #[command(verbatim_doc_comment)]
    View {
        /// Product name
        product: String,
        /// Component name (exact match)
        name: String,
    },

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
        /// Read component fields from a JSON object (`-` reads stdin)
        #[arg(long, value_name = "PATH")]
        from_json: Option<String>,
        /// Product name
        #[arg(long, required_unless_present = "from_json")]
        product: Option<String>,
        /// Component name
        #[arg(long, required_unless_present = "from_json")]
        name: Option<String>,
        /// Component description
        #[arg(long, required_unless_present = "from_json")]
        description: Option<String>,
        /// Default assignee email
        #[arg(long, required_unless_present = "from_json")]
        default_assignee: Option<String>,
    },

    /// Update an existing component by ID or product/name (admin only).
    ///
    /// Requires Bugzilla admin permissions. Pass any of the flags
    /// to change that property: `--name`, `--description`,
    /// `--default-assignee`. Only the supplied fields are modified.
    ///
    /// The numeric `<id>` is the component ID, not the name. As a
    /// human-oriented alternative, pass `--product <PRODUCT>` with
    /// `--component <COMPONENT>` to resolve the current component name
    /// exactly within that product. `--name` remains the new component
    /// name.
    ///
    /// Examples:
    ///
    ///   bzr component update 42 --description "Updated description"
    ///   bzr component update --product MyApp --component Backend \
    ///     --description "Updated description"
    ///   bzr component update 42 --default-assignee newowner@example.com
    ///
    /// See bzr-component-create(1) for new components and
    /// bzr-product-view(1) to inspect component IDs and names.
    #[command(verbatim_doc_comment)]
    Update {
        /// Read component update fields from a JSON object (`-` reads stdin)
        #[arg(long, value_name = "PATH")]
        from_json: Option<String>,
        /// Component ID
        id: Option<u64>,
        /// Product name for name-based targeting
        #[arg(long, value_name = "PRODUCT")]
        product: Option<String>,
        /// Current component name for name-based targeting
        #[arg(long, value_name = "COMPONENT")]
        component: Option<String>,
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

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
