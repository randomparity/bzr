use clap::{Args, Subcommand};

pub const LONG_ABOUT: &str = r"Manage RHBZ ExternalBugs links through its XML-RPC extension.

This command requires an API key and a server that advertises the ExternalBugs
extension. Stock Bugzilla is refused before a mutation request is sent.

Examples:

  bzr bug external-bug add 123 --tracker 7 --external-id EXT-1 --status NEW --description created
  bzr bug external-bug update 123 --tracker 7 --external-id EXT-1 --status ASSIGNED --description updated
  bzr bug external-bug remove 123 --tracker 7 --external-id EXT-1
";

#[derive(Args, Debug)]
pub(crate) struct ExternalBugArgs {
    #[command(subcommand)]
    pub action: ExternalBugAction,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ExternalBugAction {
    /// Add an external tracker link to a bug.
    #[command(
        long_about = "Add a configured RHBZ ExternalBugs link to one bug.\n\nRequires an API key and an advertised ExternalBugs extension. Use --tracker for the configured tracker ID and --external-id for the linked issue identifier."
    )]
    Add(AddExternalBugArgs),
    /// Update an existing external tracker link.
    #[command(
        long_about = "Update a configured RHBZ ExternalBugs link on one bug.\n\nRequires an API key and an advertised ExternalBugs extension. The target is identified by --tracker and --external-id."
    )]
    Update(UpdateExternalBugArgs),
    /// Remove an external tracker link.
    #[command(
        long_about = "Remove a configured RHBZ ExternalBugs link from one bug.\n\nRequires an API key and an advertised ExternalBugs extension. The target is identified by --tracker and --external-id."
    )]
    Remove(RemoveExternalBugArgs),
}

#[derive(Args, Debug)]
pub(crate) struct AddExternalBugArgs {
    /// Bug ID.
    pub id: u64,
    /// Configured external tracker ID.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub tracker: u64,
    /// External tracker bug ID.
    #[arg(long, value_name = "ID")]
    pub external_id: String,
    /// External bug status.
    #[arg(long)]
    pub status: String,
    /// External bug description.
    #[arg(long)]
    pub description: String,
}

#[derive(Args, Debug)]
pub(crate) struct UpdateExternalBugArgs {
    /// Bug ID.
    pub id: u64,
    /// Configured external tracker ID.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub tracker: u64,
    /// External tracker bug ID.
    #[arg(long, value_name = "ID")]
    pub external_id: String,
    /// New external bug status.
    #[arg(long)]
    pub status: String,
    /// New external bug description.
    #[arg(long)]
    pub description: String,
}

#[derive(Args, Debug)]
pub(crate) struct RemoveExternalBugArgs {
    /// Bug ID.
    pub id: u64,
    /// Configured external tracker ID.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub tracker: u64,
    /// External tracker bug ID.
    #[arg(long, value_name = "ID")]
    pub external_id: String,
}
