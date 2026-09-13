use clap::Args;

/// Arguments for updating a bug's personal tags.
#[derive(Args, Debug)]
pub(crate) struct TagArgs {
    /// Bug ID.
    pub id: u64,
    /// Tags to add (repeatable).
    #[arg(long)]
    pub add: Vec<String>,
    /// Tags to remove (repeatable).
    #[arg(long)]
    pub remove: Vec<String>,
}
