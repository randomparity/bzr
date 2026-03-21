use clap::Subcommand;

#[derive(Subcommand)]
pub enum CommentAction {
    /// List comments on a bug
    List {
        /// Bug ID
        bug_id: u64,
        /// Only show comments created after this date (ISO 8601)
        #[arg(long)]
        since: Option<String>,
    },
    /// Add a comment to a bug
    Add {
        /// Bug ID
        bug_id: u64,
        /// Comment text (opens $EDITOR if not provided)
        #[arg(long)]
        body: Option<String>,
    },
    /// Add or remove tags on a comment
    Tag {
        /// Comment ID
        comment_id: u64,
        /// Tags to add
        #[arg(long)]
        add: Vec<String>,
        /// Tags to remove
        #[arg(long)]
        remove: Vec<String>,
    },
    /// Search comments by tag
    SearchTags {
        /// Tag query
        query: String,
    },
}
