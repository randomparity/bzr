use clap::{Args, Subcommand};

use crate::types::bug::{FilterField, SearchParams, FIELD_MAPPINGS};
use crate::types::output::SortDirection;
use crate::types::query::SavedQuery;

mod clone;
mod create;
mod history;
mod list;
mod my;
mod search;
mod update;
mod verbs;
mod view;

pub(crate) use clone::CloneArgs;
pub(crate) use create::CreateArgs;
pub(crate) use history::HistoryArgs;
pub(crate) use list::ListArgs;
pub(crate) use my::MyArgs;
pub(crate) use search::SearchArgs;
pub(crate) use update::UpdateArgs;
pub(crate) use verbs::{CloseArgs, DupArgs, ReopenArgs, ResolveArgs};
pub(crate) use view::ViewArgs;

/// Shared `--sort` / `--order` result ordering, flattened into the bug query
/// subcommands (`list`, `search`, `my`) and `query run`. Absent `--sort`,
/// results default to a stable `bug_id` order so identical runs are
/// reproducible.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct SortArgs {
    /// Sort results by this field (e.g. `last_change_time`, `priority`,
    /// `bug_id`). Field-name aliases (`id`, `severity`, `status`, ...) are
    /// accepted. Absent, results are ordered by `bug_id` for determinism.
    #[arg(long, value_name = "FIELD")]
    pub sort: Option<String>,
    /// Sort direction: `asc` (default) or `desc`. Only meaningful with
    /// `--sort`.
    #[arg(long, default_value = "asc", value_name = "DIR")]
    pub order: SortDirection,
}

/// Shared `--offset` / `--paginate` paging flags, flattened into the
/// search-backed bug commands (`list`, `search`, `my`) and `query run`.
/// Bugzilla returns no total, so `--paginate` (loop all pages) is the path to
/// full retrieval and `--offset` walks windows manually.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct PageArgs {
    /// Skip the first N matching bugs (manual paging past `--limit`).
    ///
    /// Page through a large result set by repeating with increasing offsets;
    /// an empty/short page means there are no more matches. Mutually exclusive
    /// with `--paginate`.
    #[arg(long, value_name = "N", conflicts_with = "paginate")]
    pub offset: Option<u32>,
    /// Retrieve every matching page, looping internally past `--limit`.
    ///
    /// `--limit` becomes the per-request page size; bzr fetches pages until the
    /// server returns a short page, then emits the full result set. Use this
    /// for "process all matching bugs" workflows.
    #[arg(long)]
    pub paginate: bool,
}

/// Create-time field flags shared into `bug create`, giving it parity with
/// `bug update` for the subset Bugzilla's `Bug.create` accepts in one call.
/// Grouped into one flattened struct so the `Create` variant stays readable
/// and the set is defined once.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct CreateFieldArgs {
    /// Set an alias for the new bug.
    #[arg(long)]
    pub alias: Option<String>,
    /// Set the URL field.
    #[arg(long)]
    pub url: Option<String>,
    /// Set the Status Whiteboard.
    #[arg(long)]
    pub whiteboard: Option<String>,
    /// Set the Target Milestone.
    #[arg(long)]
    pub target_milestone: Option<String>,
    /// Set the deadline date (`YYYY-MM-DD`).
    #[arg(long, value_name = "DATE")]
    pub deadline: Option<String>,
    /// Add CC entries (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',')]
    pub cc: Vec<String>,
    /// Add keywords (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',')]
    pub keywords: Vec<String>,
    /// Add the new bug to these groups (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',')]
    pub groups: Vec<String>,
    /// Set or request a flag using Bugzilla flag syntax (repeatable).
    ///
    /// Accepted forms: `name+` (granted), `name-` (denied), `name?`
    /// (request), or `name?(user@example.com)` (request a specific user).
    #[arg(long)]
    pub flag: Vec<String>,
}

/// Create-time metadata overrides accepted by `bug clone`.
///
/// This intentionally omits `--alias`: a clone must never copy the source
/// alias, and assigning a new unique alias while cloning is a narrower workflow
/// than the metadata fields users commonly need to adjust.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct CloneCreateFieldArgs {
    /// Override the URL field.
    #[arg(long)]
    pub url: Option<String>,
    /// Override the Status Whiteboard.
    #[arg(long)]
    pub whiteboard: Option<String>,
    /// Override the Target Milestone.
    #[arg(long)]
    pub target_milestone: Option<String>,
    /// Override the deadline date (`YYYY-MM-DD`).
    #[arg(long, value_name = "DATE")]
    pub deadline: Option<String>,
    /// Replace the copied CC list (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',', conflicts_with = "no_cc")]
    pub cc: Vec<String>,
    /// Replace the copied keywords (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',', conflicts_with = "no_keywords")]
    pub keywords: Vec<String>,
    /// Add the cloned bug to these groups (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',')]
    pub groups: Vec<String>,
    /// Set or request a flag using Bugzilla flag syntax (repeatable).
    ///
    /// Accepted forms: `name+` (granted), `name-` (denied), `name?`
    /// (request), or `name?(user@example.com)` (request a specific user).
    #[arg(long)]
    pub flag: Vec<String>,
}

/// Shared comment flags for the convenience verbs (`resolve`, `close`,
/// `reopen`, `dup`), which post a comment atomically with the state change —
/// the same `--comment` / `--comment-file` / `--comment-private` set `bug
/// update` accepts.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct CommentArgs {
    /// Post a comment atomically with the change.
    ///
    /// A value of `-` reads the comment from stdin. Mutually exclusive
    /// with `--comment-file`. Empty / whitespace-only bodies exit 7.
    #[arg(long, value_name = "BODY", conflicts_with = "comment_file")]
    pub comment: Option<String>,
    /// Read the comment body from a UTF-8 file.
    ///
    /// A path of `-` reads from stdin. Mutually exclusive with `--comment`.
    #[arg(long, value_name = "PATH", conflicts_with = "comment")]
    pub comment_file: Option<std::path::PathBuf>,
    /// Mark the comment private (requires `--comment` or `--comment-file`).
    #[arg(long)]
    pub comment_private: bool,
}

/// Shared `--fields` / `--exclude-fields` selection, flattened into the bug
/// query subcommands (`list`, `view`, `search`, `my`) so the pair is defined
/// once instead of repeated per variant.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct FieldArgs {
    /// Fields to request from the server (comma-separated). In table output,
    /// selects which columns (or detail rows) to show, in order; under --json,
    /// the object contains only the selected fields (id only if requested).
    /// Built-in fields and Bugzilla custom fields named `cf_*` are valid.
    #[arg(long)]
    pub fields: Option<String>,
    /// Fields to drop from the server request (comma-separated). In table
    /// output, removes those columns/rows; under --json, the object omits the
    /// dropped fields (including id, if excluded).
    #[arg(long)]
    pub exclude_fields: Option<String>,
}

/// Bug filter flags with the same meaning across `bug list`, `bug my`, and
/// saved-query commands.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct BugFilterArgs {
    /// Filter by product (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub product: Vec<String>,
    /// Filter by component (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub component: Vec<String>,
    /// Filter by status (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub status: Vec<String>,
    /// Filter by priority (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub priority: Vec<String>,
    /// Filter by severity (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub severity: Vec<String>,
    /// Filter by Status Whiteboard substring (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub whiteboard: Vec<String>,
    /// Filter by Target Milestone (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub target_milestone: Vec<String>,
    /// Filter by Version (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub version: Vec<String>,
    /// Filter by Operating System (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub op_sys: Vec<String>,
    /// Filter by Platform (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub platform: Vec<String>,
    /// Filter by Resolution (repeatable for OR; prefix with ! to exclude); empty matches open bugs
    #[arg(long)]
    pub resolution: Vec<String>,
    /// Filter by QA Contact login (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub qa_contact: Vec<String>,
    /// Filter by URL field substring (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub url: Vec<String>,
}

impl BugFilterArgs {
    fn values_for(&self, field: FilterField) -> Option<&[String]> {
        match field {
            FilterField::Product => Some(self.product.as_slice()),
            FilterField::Component => Some(self.component.as_slice()),
            FilterField::Status => Some(self.status.as_slice()),
            FilterField::Priority => Some(self.priority.as_slice()),
            FilterField::Severity => Some(self.severity.as_slice()),
            FilterField::Whiteboard => Some(self.whiteboard.as_slice()),
            FilterField::TargetMilestone => Some(self.target_milestone.as_slice()),
            FilterField::Version => Some(self.version.as_slice()),
            FilterField::OpSys => Some(self.op_sys.as_slice()),
            FilterField::Platform => Some(self.platform.as_slice()),
            FilterField::Resolution => Some(self.resolution.as_slice()),
            FilterField::QaContact => Some(self.qa_contact.as_slice()),
            FilterField::Url => Some(self.url.as_slice()),
            FilterField::AssignedTo | FilterField::Creator => None,
        }
    }

    pub(crate) fn write_search_filters(&self, params: &mut SearchParams) {
        write_search_filter_values(params, |field| self.values_for(field));
    }

    pub(crate) fn write_saved_query_filters(&self, query: &mut SavedQuery) {
        write_saved_query_filter_values(query, |field| self.values_for(field));
    }

    pub(crate) fn merge_saved_query_filters(&self, query: &mut SavedQuery) -> bool {
        merge_saved_query_filter_values(query, |field| self.values_for(field))
    }
}

/// Bug actor filters shared by `bug list` and saved-query commands.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct BugActorFilterArgs {
    /// Filter by assignee (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub assignee: Vec<String>,
    /// Filter by creator (repeatable for OR; prefix with ! to exclude)
    #[arg(long)]
    pub creator: Vec<String>,
}

impl BugActorFilterArgs {
    fn values_for(&self, field: FilterField) -> Option<&[String]> {
        match field {
            FilterField::AssignedTo => Some(self.assignee.as_slice()),
            FilterField::Creator => Some(self.creator.as_slice()),
            _ => None,
        }
    }

    pub(crate) fn write_search_filters(&self, params: &mut SearchParams) {
        write_search_filter_values(params, |field| self.values_for(field));
    }

    pub(crate) fn write_saved_query_filters(&self, query: &mut SavedQuery) {
        write_saved_query_filter_values(query, |field| self.values_for(field));
    }

    pub(crate) fn merge_saved_query_filters(&self, query: &mut SavedQuery) -> bool {
        merge_saved_query_filter_values(query, |field| self.values_for(field))
    }
}

fn write_search_filter_values<'a>(
    params: &mut SearchParams,
    mut values_for: impl FnMut(FilterField) -> Option<&'a [String]>,
) {
    for mapping in FIELD_MAPPINGS {
        let Some(values) = values_for(mapping.field) else {
            continue;
        };
        *params.get_field_mut(mapping.field) = values.to_vec();
    }
}

fn write_saved_query_filter_values<'a>(
    query: &mut SavedQuery,
    mut values_for: impl FnMut(FilterField) -> Option<&'a [String]>,
) {
    for mapping in FIELD_MAPPINGS {
        let Some(values) = values_for(mapping.field) else {
            continue;
        };
        *query.get_field_mut(mapping.field) = values.to_vec();
    }
}

fn merge_saved_query_filter_values<'a>(
    query: &mut SavedQuery,
    mut values_for: impl FnMut(FilterField) -> Option<&'a [String]>,
) -> bool {
    let mut changed = false;
    for mapping in FIELD_MAPPINGS {
        let Some(values) = values_for(mapping.field) else {
            continue;
        };
        if !values.is_empty() {
            *query.get_field_mut(mapping.field) = values.to_vec();
            changed = true;
        }
    }
    changed
}

#[derive(Subcommand)]
pub(crate) enum BugAction {
    /// List bugs that match the given filters.
    #[command(long_about = list::LONG_ABOUT)]
    List(ListArgs),
    /// View one or more bugs by ID or alias.
    #[command(long_about = view::LONG_ABOUT)]
    View(ViewArgs),
    /// Search bugs using Bugzilla quicksearch or by parsing a Bugzilla URL.
    #[command(long_about = search::LONG_ABOUT)]
    Search(SearchArgs),
    /// Show the change history for a single bug.
    #[command(long_about = history::LONG_ABOUT)]
    History(HistoryArgs),
    /// Create a new bug under a product and component.
    #[command(long_about = create::LONG_ABOUT)]
    Create(CreateArgs),
    /// Show bugs related to the authenticated user.
    #[command(long_about = my::LONG_ABOUT)]
    My(MyArgs),
    /// Clone an existing bug, optionally overriding fields.
    #[command(long_about = clone::LONG_ABOUT)]
    Clone(CloneArgs),
    /// Update one or more bugs with the same set of changes.
    #[command(long_about = update::LONG_ABOUT)]
    Update(UpdateArgs),
    /// Resolve one or more bugs (sets status RESOLVED + a resolution).
    #[command(long_about = verbs::RESOLVE_LONG_ABOUT)]
    Resolve(ResolveArgs),
    /// Close one or more bugs (sets status VERIFIED by default).
    #[command(long_about = verbs::CLOSE_LONG_ABOUT)]
    Close(CloseArgs),
    /// Reopen one or more bugs (sets status CONFIRMED by default).
    #[command(long_about = verbs::REOPEN_LONG_ABOUT)]
    Reopen(ReopenArgs),
    /// Mark a bug as a duplicate of another bug.
    #[command(long_about = verbs::DUP_LONG_ABOUT)]
    Dup(DupArgs),
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
