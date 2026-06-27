use clap::Args;

/// `--fields` / `--exclude-fields` projection flags shared by list/view verbs.
/// Field names are the verb's `--json` keys. These only affect `--json` and
/// `--output ndjson`; with table output they are ignored (with a warning).
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct ProjectionArgs {
    /// Comma-separated JSON keys to keep (only affects --json/--output ndjson).
    #[arg(long)]
    pub fields: Option<String>,
    /// Comma-separated JSON keys to drop (only affects --json/--output ndjson).
    #[arg(long)]
    pub exclude_fields: Option<String>,
}
