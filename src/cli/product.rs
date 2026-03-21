use clap::Subcommand;

use crate::types::ProductListType;

#[derive(Subcommand)]
pub enum ProductAction {
    /// List products
    List {
        /// Product type: accessible (default), selectable, or enterable
        #[arg(long, default_value = "accessible")]
        r#type: ProductListType,
    },
    /// View product details (components, versions, milestones)
    View {
        /// Product name
        name: String,
    },
    /// Create a new product
    Create {
        /// Product name
        #[arg(long)]
        name: String,
        /// Product description
        #[arg(long)]
        description: String,
        /// Initial version
        #[arg(long, default_value = "unspecified")]
        version: String,
        /// Whether the product is open for bugs
        #[arg(long, default_value = "true")]
        is_open: bool,
    },
    /// Update an existing product
    Update {
        /// Product name
        name: String,
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
