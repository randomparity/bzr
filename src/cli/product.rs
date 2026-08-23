use clap::Subcommand;

use crate::types::product::ProductListType;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub(crate) enum ProductAction {
    /// List products visible to the caller.
    ///
    /// `--type` selects which slice of the product catalog to list:
    /// `accessible` (default) -- products the caller can see;
    /// `selectable` -- products the caller can file bugs against;
    /// `enterable` -- a strict subset of `selectable` that excludes
    /// products marked closed for new bugs. Most users want
    /// `accessible`.
    ///
    /// Examples:
    ///
    ///   bzr product list
    ///   bzr product list --type selectable
    ///   bzr product list --json | jq -r '.data[].name'
    ///
    /// See bzr-product-view(1) for one product's full details.
    #[command(verbatim_doc_comment)]
    List {
        /// Product type: accessible (default), selectable, or enterable
        #[arg(long, default_value = "accessible")]
        r#type: ProductListType,
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },

    /// View a product's full details: components, versions, milestones.
    ///
    /// Prints the product's description, classification, default
    /// milestone, the list of components (with default assignees
    /// and CC lists), the list of versions, and the list of target
    /// milestones. Useful for discovering valid `--component` and
    /// `--version` values to pass to `bzr bug create`.
    ///
    /// Examples:
    ///
    ///   bzr product view Firefox
    ///   bzr product view Firefox --json | jq -r '.data.components[].name'
    ///
    /// See bzr-product-list(1) for the catalog and bzr-bug-create(1)
    /// for filing a bug against a product.
    #[command(verbatim_doc_comment)]
    View {
        /// Product name
        name: String,
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },

    /// Create a new Bugzilla product (admin only).
    ///
    /// Requires Bugzilla admin permissions. Both `--name` and
    /// `--description` are required. `--version` defaults to
    /// "unspecified" -- this becomes the product's initial version
    /// entry, which can be extended later via the Bugzilla web UI.
    /// Products are open for bugs by default; pass `--is-open false`
    /// to create a closed product.
    ///
    /// New products have no components -- add at least one with
    /// `bzr component create` before bugs can be filed.
    ///
    /// Examples:
    ///
    ///   bzr product create --name MyApp --description "My Application"
    ///   bzr product create --name Legacy --description "Archived" \
    ///     --is-open false
    ///
    /// See bzr-product-update(1) to modify a product and
    /// bzr-component-create(1) to add components.
    #[command(verbatim_doc_comment)]
    Create {
        /// Read product fields from a JSON object (`-` reads stdin)
        #[arg(long, value_name = "PATH")]
        from_json: Option<String>,
        /// Product name
        #[arg(long, required_unless_present = "from_json")]
        name: Option<String>,
        /// Product description
        #[arg(long, required_unless_present = "from_json")]
        description: Option<String>,
        /// Initial version
        #[arg(long)]
        version: Option<String>,
        /// Whether the product is open for bugs
        #[arg(long)]
        is_open: Option<bool>,
    },

    /// Update an existing product (admin only).
    ///
    /// Requires Bugzilla admin permissions. Pass any of the flags
    /// to change that property: `--description`,
    /// `--default-milestone`, `--is-open`. Only the supplied fields
    /// are modified. Renaming a product is not supported by the
    /// Bugzilla REST API.
    ///
    /// `--default-milestone` must reference a milestone that
    /// already exists on the product (visible via
    /// `bzr product view`).
    ///
    /// Examples:
    ///
    ///   bzr product update Firefox --description "Web browser"
    ///   bzr product update Legacy --is-open false
    ///   bzr product update MyApp --default-milestone v2.0
    ///
    /// See bzr-product-view(1) to inspect current state.
    #[command(verbatim_doc_comment)]
    Update {
        /// Read product update fields from a JSON object (`-` reads stdin)
        #[arg(long, value_name = "PATH")]
        from_json: Option<String>,
        /// Product name
        #[arg(required_unless_present = "from_json")]
        name: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// Default milestone
        #[arg(long)]
        default_milestone: Option<String>,
        /// Whether the product is open for bugs
        #[arg(long)]
        is_open: Option<bool>,
    },
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
