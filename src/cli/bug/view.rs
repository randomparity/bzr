use clap::Args;

use crate::cli::bug::FieldArgs;

pub const LONG_ABOUT: &str = r#"View one or more bugs by ID or alias.

Single-ID: prints the bug's full record (summary, status,
assignee, priority, CC list, depends-on, blocks, and the most
recent comments) as a formatted detail block or JSON object.

Multi-ID: emits one detail block per bug separated by a
horizontal divider, in argument order. Aliases and numeric IDs
may be mixed. JSON output for multi-ID becomes a wrapped
`{"bugs": [...], "failed": [...]}` object — `failed` is always
present (empty array when no failures) so `jq` consumers can
rely on `.bugs[]`. Single-ID JSON output is unchanged
(a bare `Bug` object).

`--permissive` (multi-ID only) suppresses per-bug access
failures: inaccessible bugs are surfaced as inline
`Bug #N — UNAVAILABLE` placeholder blocks instead of bailing
the whole call. Exit code is 0 even when some bugs fail.
Session-wide failures (transport, auth, security) still bail.

Use `--fields` to fetch only specific fields (faster on large
bugs over REST); `--exclude-fields` is the inverse. Under `--json`
the returned object is trimmed to the selected fields (gh-style) on
every transport, since trimming happens client-side after the fetch.
Built-in fields and Bugzilla custom fields named `cf_*` are valid.
On XML-RPC servers the full bug is fetched regardless of the field
list, so there the selection only controls which detail rows (table)
or object keys (JSON) appear, not what is sent over the wire.

Under `--json`, `bug view` stays lenient when the selection resolves
to nothing known: an unknown or mistyped `--fields`, or an
`--exclude-fields` covering every field, yields an empty `{}` object
and exits 0, with a one-line warning on stderr. `bug list`, `my`,
`search`, and `query run` instead exit 7 in that case. So a `{}`
object plus a zero exit can mean a field name was misspelled — check
stderr.

Examples:

  bzr bug view 12345
  bzr bug view 12345 12346 12347
  bzr bug view 12345 my-alias 12347 --permissive
  bzr bug view 12345 --json | jq .data.summary
  bzr bug view my-alias --fields id,summary,status

See bzr-bug-history(1) for the change log and
bzr-comment-list(1) for the full comment thread."#;

/// Arguments for `bug view`.
#[derive(Args, Debug)]
#[expect(
    clippy::doc_markdown,
    reason = "Bug.get should stay literal in CLI help"
)]
pub(crate) struct ViewArgs {
    /// Bug ID(s) or alias(es). Aliases and numeric IDs may be mixed.
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<String>,
    /// Continue past per-bug failures (multi-ID only).
    ///
    /// When set, inaccessible bugs (NotFound or Bug.get fault
    /// codes 100/101/102) are reported as inline placeholder
    /// rows and the command exits 0. Without `--permissive`,
    /// the first per-bug failure aborts the whole call. Has
    /// no effect on session-wide errors (transport, auth,
    /// security) — those always bail.
    #[arg(long)]
    pub permissive: bool,
    /// Open the bug's web page in the default browser instead of
    /// printing its record.
    ///
    /// Resolves the active server's base URL and opens
    /// `show_bug.cgi?id=<ID>` for each ID given. No network call
    /// or authentication is needed. When stdout is not a terminal,
    /// or there is no display (headless / SSH without X), the URL is
    /// printed to stdout and the command exits 0 instead of opening
    /// a browser — which keeps it safe for scripts and pipes.
    /// `--fields` and `--permissive` are ignored with `--web`.
    #[arg(long)]
    pub web: bool,
    #[command(flatten)]
    pub field_args: FieldArgs,
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
