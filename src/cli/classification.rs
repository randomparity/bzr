use clap::Subcommand;

#[derive(Subcommand)]
pub enum ClassificationAction {
    /// View a classification by name or ID.
    ///
    /// Prints the classification's description and the products it
    /// contains. Classifications are an optional grouping above
    /// products in Bugzilla; on installations where they are
    /// disabled, the only entry is "Unclassified" and every product
    /// belongs to it.
    ///
    /// The positional argument can be either the classification name
    /// or its numeric ID.
    ///
    /// Examples:
    ///
    ///   bzr classification view Unclassified
    ///   bzr classification view "Red Hat" --json
    ///   bzr classification view 1 --json | jq '.products[].name'
    ///
    /// See bzr-product-list(1) for the flat product catalog and
    /// bzr-product-view(1) for one product's full details.
    #[command(verbatim_doc_comment)]
    View {
        /// Classification name or ID
        name: String,
    },
}
