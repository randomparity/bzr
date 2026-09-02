use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum ClassificationAction {
    /// List the server's classifications.
    ///
    /// Enumerates every classification with its ID, name, description, and
    /// product count. Bugzilla has no bulk classification endpoint, so bzr
    /// reads the names from the `classification` field's legal values and
    /// fetches each one's detail.
    ///
    /// Classifications are an optional Bugzilla feature. Disabled servers
    /// either expose only "Unclassified" or return API error 900 to
    /// unprivileged users. Table output writes the disabled note to stdout.
    /// JSON-family output writes an empty collection to stdout and the note
    /// to stderr.
    ///
    /// Examples:
    ///
    ///   bzr classification list
    ///   bzr classification list --json | jq -r '.data[].name'
    ///
    /// See bzr-classification-view(1) for one classification's full detail.
    #[command(verbatim_doc_comment)]
    List {
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },

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
    ///   bzr classification view 1 --json | jq -r '.data.products[].name'
    ///
    /// See bzr-product-list(1) for the flat product catalog and
    /// bzr-product-view(1) for one product's full details.
    #[command(verbatim_doc_comment)]
    View {
        /// Classification name or ID
        name: String,
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
