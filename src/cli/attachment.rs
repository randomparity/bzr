use clap::Subcommand;

#[derive(Subcommand)]
pub enum AttachmentAction {
    /// List attachments on a bug
    List {
        /// Bug ID
        bug_id: u64,
    },
    /// Download an attachment
    Download {
        /// Attachment ID
        id: u64,
        /// Output file path (defaults to original filename)
        #[arg(short = 'o', long = "out", id = "out_file")]
        out: Option<String>,
    },
    /// Upload an attachment to a bug
    Upload {
        /// Bug ID
        bug_id: u64,
        /// File to upload
        file: String,
        /// Attachment summary/description
        #[arg(long)]
        summary: Option<String>,
        /// MIME type (auto-detected if not provided)
        #[arg(long)]
        content_type: Option<String>,
        /// Set flags (e.g. "review?(user@example.com)")
        #[arg(long)]
        flag: Vec<String>,
    },
    /// Update an attachment
    Update {
        /// Attachment ID
        id: u64,
        /// New summary
        #[arg(long)]
        summary: Option<String>,
        /// New file name
        #[arg(long)]
        file_name: Option<String>,
        /// New content type
        #[arg(long)]
        content_type: Option<String>,
        /// Mark as obsolete
        #[arg(long)]
        obsolete: Option<bool>,
        /// Mark as patch
        #[arg(long)]
        is_patch: Option<bool>,
        /// Mark as private
        #[arg(long)]
        is_private: Option<bool>,
        /// Set flags (e.g. "review?(user@example.com)")
        #[arg(long)]
        flag: Vec<String>,
    },
}
