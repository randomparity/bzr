use clap::Args;

pub const LONG_ABOUT: &str = r"Update personal tags on a bug through Bugzilla's XML-RPC API.

Use one or more `--add` or `--remove` values. Tags already present, or tags
already absent, are accepted by Bugzilla without changing the bug. This command
uses XML-RPC because Bugzilla exposes personal-tag mutation as `Bug.update_tags`.

Examples:

  bzr bug tag 12345 --add triage
  bzr bug tag 12345 --add needs-review --remove stale
";

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
