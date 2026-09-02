use clap::Args;

pub const LONG_ABOUT: &str = r#"Update one or more bugs with the same set of changes.

Accepts one or more bug IDs as positional args. All field
changes (`--status`, `--resolution`, `--dupe-of`, `--assignee`,
`--platform`, `--priority`, `--severity`, `--summary`, `--whiteboard`) are
applied to every bug in the list. When closing a bug, both
`--status` and `--resolution` typically need to be set
together (e.g. `--status RESOLVED --resolution FIXED`).

`--dupe-of <ID>` marks the bug as a duplicate. Bugzilla sets
status/resolution automatically; do not set status/resolution
just to make the duplicate transition.

`--flag` accepts Bugzilla flag syntax: `name?`, `name+`,
`name-`, `name?(user@example.com)`, or `name?,!` to clear.
Repeatable.

`--comment <BODY>` (or `--comment-file <PATH>`) posts a comment
atomically with the field changes — a single `Bug.update`
round-trip rather than a separate `bzr comment add` call. A
value of `-` for either flag reads the comment from stdin.
`--comment-private` marks it private. Empty / whitespace-only
bodies are rejected (exit 7).

List-typed fields support `*-add` / `*-remove` pairs for
incremental edits: `--blocks`, `--depends-on`, `--keywords`,
`--cc`, `--groups`, and `--see-also`. The first five accept
comma-separated values. `--see-also-add` / `--see-also-remove`
do not split on commas — repeat the flag to pass multiple URLs.
Additional update fields include `--alias`, `--deadline`,
`--estimated-time`, `--remaining-time`, `--work-time`,
`--reset-assigned-to`, and `--reset-qa-contact`.

On batch updates, partial failures (some bugs updated,
others rejected) exit with code 11 (BatchPartialFailure) and
the JSON output enumerates per-bug results.

Examples:

  bzr bug update 100 --status RESOLVED --resolution FIXED
  bzr bug update 100 --dupe-of 200
  bzr bug update 100 200 300 --priority high --flag review+
  bzr bug update 100 --status RESOLVED --resolution FIXED \
    --comment "Fixed by patch in #200"
  bzr bug update 100 --blocks-add 200,201 \
    --depends-on-remove 99
  bzr bug update 100 --keywords-add fix-needed,regression \
    --cc-add alice@example.com \
    --see-also-add <https://example.com/issue/42>

See bzr-bug-create(1) for new bugs, bzr-bug-clone(1) for
cloning, and bzr-comment-add(1) for adding a comment as part
of a status change."#;

/// Arguments for `bug update`.
#[derive(Args, Debug, Default)]
pub(crate) struct UpdateArgs {
    /// Apply one or more structured bug updates from JSON.
    ///
    /// A value of `-` reads the JSON from stdin; otherwise it is a
    /// file path. A top-level object applies one edit to the
    /// positional IDs, or to its own `id` when no positional ID is
    /// given. A top-level array applies one independent edit per
    /// element; each element must include `id` and returns the
    /// existing batch result shape (exit 11 if any element fails).
    /// Unknown keys are rejected, and explicit CLI flags override
    /// corresponding JSON fields.
    #[arg(long, value_name = "PATH")]
    pub from_json: Option<String>,
    /// Bug ID(s).
    ///
    /// One or more IDs. When more than one is supplied, the
    /// same field changes are applied to every bug; partial
    /// failures (some bugs updated, others rejected) exit with
    /// code 11 and the JSON output enumerates per-bug results.
    #[arg(required_unless_present = "from_json", num_args = 1..)]
    pub ids: Vec<u64>,
    /// New status (e.g. `NEW`, `ASSIGNED`, `RESOLVED`, `CLOSED`).
    ///
    /// When closing a bug, `--resolution` must usually be set
    /// in the same call. Discover valid values via
    /// `bzr field list status`.
    #[arg(long, conflicts_with = "dupe_of")]
    pub status: Option<String>,
    /// Resolution to set when closing a bug.
    ///
    /// Required by most workflows when `--status` transitions
    /// to a closed state (e.g. `RESOLVED`, `VERIFIED`).
    /// Discover valid values via `bzr field list resolution`.
    #[arg(long, conflicts_with = "dupe_of")]
    pub resolution: Option<String>,
    /// Mark this bug as a duplicate of another bug.
    ///
    /// Forwards Bugzilla's `dupe_of` field. Bugzilla handles the
    /// status/resolution transition to RESOLVED/DUPLICATE.
    #[arg(long, value_name = "ID")]
    pub dupe_of: Option<u64>,
    /// Set this bug's alias.
    ///
    /// Bugzilla only allows alias updates for a single bug at a time.
    #[arg(long, value_name = "ALIAS")]
    pub alias: Option<String>,
    /// Set the deadline date (`YYYY-MM-DD`).
    #[arg(long, value_name = "DATE")]
    pub deadline: Option<String>,
    /// Set the total estimated time in hours.
    #[arg(long, value_name = "HOURS")]
    pub estimated_time: Option<f64>,
    /// Set the remaining time in hours.
    #[arg(long, value_name = "HOURS")]
    pub remaining_time: Option<f64>,
    /// Add work time in hours for this update.
    #[arg(long, value_name = "HOURS")]
    pub work_time: Option<f64>,
    /// Reset assignee to the component default.
    #[arg(long)]
    pub reset_assigned_to: bool,
    /// Reset QA contact to the component default.
    #[arg(long)]
    pub reset_qa_contact: bool,
    /// Reassign
    #[arg(long)]
    pub assignee: Option<String>,
    /// Set this bug's hardware platform.
    #[arg(long)]
    pub platform: Option<String>,
    /// Priority
    #[arg(long)]
    pub priority: Option<String>,
    /// Severity
    #[arg(long)]
    pub severity: Option<String>,
    /// Summary
    #[arg(long)]
    pub summary: Option<String>,
    /// Whiteboard
    #[arg(long)]
    pub whiteboard: Option<String>,
    /// Set this bug's URL field.
    #[arg(long)]
    pub url: Option<String>,
    /// Set this bug's target milestone.
    #[arg(long, value_name = "MILESTONE")]
    pub target_milestone: Option<String>,
    /// Post a comment atomically with the field changes.
    ///
    /// A value of `-` reads the comment from stdin. Mutually
    /// exclusive with `--comment-file`. Use `--comment-private`
    /// to mark the comment private. Empty / whitespace-only
    /// bodies are rejected (exit 7).
    #[arg(long, value_name = "BODY", conflicts_with = "comment_file")]
    pub comment: Option<String>,
    /// Read the comment body from a UTF-8 file.
    ///
    /// A path of `-` reads from stdin. Mutually exclusive with
    /// `--comment`. The file must exist and be readable;
    /// non-existent paths or non-UTF-8 contents fail with exit
    /// code 7. Empty / whitespace-only contents are also
    /// rejected.
    #[arg(long, value_name = "PATH", conflicts_with = "comment")]
    pub comment_file: Option<std::path::PathBuf>,
    /// Mark the comment private (visible only to users with
    /// elevated permissions on the server).
    ///
    /// Requires `--comment` or `--comment-file`; using
    /// `--comment-private` alone is a usage error (exit 7).
    #[arg(long)]
    pub comment_private: bool,
    /// Set, request, or clear a flag using Bugzilla flag syntax.
    ///
    /// Repeatable. Accepted forms:
    /// `name+` (granted), `name-` (denied), `name?` (request),
    /// `name?(user@example.com)` (request a specific user), or
    /// `name?,!` to clear an existing flag.
    #[arg(long)]
    pub flag: Vec<String>,
    /// Add bug IDs to the blocks list (comma-separated).
    ///
    /// Combine with `--blocks-remove` for incremental edits.
    /// To replace the list entirely, the bug must be edited
    /// through the Bugzilla web UI.
    #[arg(long, value_delimiter = ',')]
    pub blocks_add: Vec<u64>,
    /// Remove bug IDs from the blocks list (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub blocks_remove: Vec<u64>,
    /// Add bug IDs to the depends-on list (comma-separated).
    ///
    /// Combine with `--depends-on-remove` for incremental
    /// edits.
    #[arg(long, value_delimiter = ',')]
    pub depends_on_add: Vec<u64>,
    /// Remove bug IDs from the depends-on list (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub depends_on_remove: Vec<u64>,
    /// Add keywords (comma-separated).
    ///
    /// Combine with `--keywords-remove` for incremental edits.
    #[arg(long, value_delimiter = ',')]
    pub keywords_add: Vec<String>,
    /// Remove keywords (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub keywords_remove: Vec<String>,
    /// Add CC entries (comma-separated).
    ///
    /// Accepts usernames or email addresses; format is
    /// server-defined.
    #[arg(long, value_delimiter = ',')]
    pub cc_add: Vec<String>,
    /// Remove CC entries (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub cc_remove: Vec<String>,
    /// Add groups (comma-separated).
    ///
    /// Group operations require permission; failures surface
    /// from the server.
    #[arg(long, value_delimiter = ',')]
    pub groups_add: Vec<String>,
    /// Remove groups (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub groups_remove: Vec<String>,
    /// Add a see-also URL.
    ///
    /// Repeat the flag to add multiple URLs (URLs may contain
    /// commas, so no comma-list parsing is performed).
    #[arg(long)]
    pub see_also_add: Vec<String>,
    /// Remove a see-also URL.
    ///
    /// Repeat the flag to remove multiple URLs.
    #[arg(long)]
    pub see_also_remove: Vec<String>,
    /// Only apply the update if the bug has not changed since this time
    /// (optimistic concurrency).
    ///
    /// Pass the `last_change_time` value from a preceding `bug view` (an
    /// ISO-8601 timestamp). Before writing, bzr re-reads each target bug
    /// and refuses the update if its current `last_change_time` differs,
    /// exiting 14 (collision) without writing — so a read-modify-write
    /// agent will not silently clobber a concurrent edit. The check is
    /// client-side (Bugzilla's REST `Bug.update` has no atomic
    /// compare-and-set), so a narrow window remains between the re-read
    /// and the write. With multiple IDs, all are checked first and any
    /// mismatch aborts the whole batch before any write.
    #[arg(long, value_name = "TIMESTAMP")]
    pub expect_unchanged_since: Option<String>,
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
