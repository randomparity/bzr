use clap::Args;

use crate::cli::bug::CreateFieldArgs;

pub const LONG_ABOUT: &str = r#"Create a new bug under a product and component.

`--product` and `--component` are required unless a saved
template (`--template`) supplies them; CLI flags override
template values. Some Bugzilla installations also require
`--op-sys` and `--rep-platform` -- the API call fails with
exit code 4 (Api) when the server demands a field that
wasn't provided.

Description sources, highest priority first:

  1. `--description "text"` (literal)
  2. `--description-file PATH` (UTF-8 file contents)
  3. piped stdin (when stdin is not a TTY)
  4. `$EDITOR` (when stdin is a TTY and none of the above)

A value of `-` for `--description` or `--description-file`
reads the description from stdin. `--description` and
`--description-file` are mutually
exclusive. When the editor flow is active, `--summary` is
optional: the first non-empty line of the buffer becomes
the summary and the rest becomes the description. A
`git commit -v`-style sentinel divider separates editable
content from informational field reminders.

On success, prints the new bug ID, alias (if assigned), and
URL to stdout. With `--json`, the same fields are emitted as
a JSON object suitable for piping into scripts.

Examples:

  bzr bug create --product Fedora --component kernel \
    --summary "Boot failure on 6.x" \
    --description "System hangs at initramfs"
  bzr bug create --product Fedora --component kernel \
    --description-file /tmp/desc.txt --summary "Boot failure"
  bzr bug create --product Fedora --component kernel
    # opens $EDITOR; first non-empty line of the buffer
    # becomes the summary
  bzr bug create --template security-bug --summary "XSS in login"

Field flags shared with `bug update` set the new bug's metadata
in the same `Bug.create` call (no follow-up update): `--alias`,
`--url`, `--whiteboard`, `--target-milestone`, `--deadline`,
`--cc`, `--keywords`, `--groups`, and `--flag`. The list flags
(`--cc`, `--keywords`, `--groups`) accept comma-separated values
and repeat; `--flag` uses Bugzilla flag syntax (`name+`, `name-`,
`name?`, `name?(user@example.com)`) and repeats. `--deadline`
takes a `YYYY-MM-DD` date.

  bzr bug create --product P --component C --summary S \
    --description D --keywords regression,crash \
    --cc qa@example.com --flag review? --target-milestone 9.0

Exit codes: 0 on success, 4 on Bugzilla API error, 7 on
input validation (missing --summary outside the editor flow,
empty editor buffer, missing or non-UTF-8 --description-file,
malformed --deadline, $EDITOR exited non-zero), 9 on auth failure.

See bzr-bug-clone(1) for cloning an existing bug,
bzr-template(1) for managing templates, and bzr-field(1) for
discovering valid `--priority`, `--severity`, and `--status`
values."#;

/// Arguments for `bug create`.
#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Create one or more bugs from a JSON object or array.
    ///
    /// A value of `-` reads the JSON from stdin; otherwise it is a
    /// file path. A single object files one bug; an array files one
    /// bug per element and returns a partial-failure result (exit 11
    /// if any element fails). Keys match the create flag names
    /// (`product`, `component`, `summary`, `version`, `description`,
    /// `priority`, `severity`, `assignee`, `op_sys`, `rep_platform`,
    /// `alias`, `url`, `whiteboard`, `target_milestone`, `deadline`,
    /// `blocks`, `depends_on`, `cc`, `keywords`, `groups`, `flags`);
    /// unknown keys are rejected. Explicit CLI flags override the
    /// corresponding JSON field (applied to every element of an
    /// array). Mutually exclusive with `--template`; bypasses the
    /// `$EDITOR` flow.
    #[arg(long, value_name = "PATH", conflicts_with = "template")]
    pub from_json: Option<String>,
    /// Use a saved template for default field values.
    ///
    /// References a named template from `bzr template list`.
    /// When set, fields stored in the template (product,
    /// component, version, priority, severity, assignee,
    /// op-sys, rep-platform, description) are used as defaults
    /// for this `create` invocation; CLI flags override
    /// template values.
    #[arg(long)]
    pub template: Option<String>,
    /// Product name (required unless supplied by `--template`).
    ///
    /// Required unless the chosen template provides a product.
    /// When both are set, this CLI value wins.
    #[arg(long)]
    pub product: Option<String>,
    /// Component name (required unless supplied by `--template`).
    ///
    /// Required unless the chosen template provides a
    /// component. When both are set, this CLI value wins. The
    /// component must exist on the chosen product -- discover
    /// valid names via `bzr product view <product>`.
    #[arg(long)]
    pub component: Option<String>,
    /// Bug summary (required unless the editor flow is active)
    #[arg(long)]
    pub summary: Option<String>,
    /// Version
    #[arg(long)]
    pub version: Option<String>,
    /// Bug description (a value of `-` reads from stdin)
    #[arg(long, conflicts_with = "description_file")]
    pub description: Option<String>,
    /// Read the bug description from a UTF-8 file.
    ///
    /// A path of `-` reads from stdin. Mutually exclusive with
    /// `--description`. The file path must exist and be readable;
    /// non-existent paths or non-UTF-8 contents fail with exit
    /// code 7.
    #[arg(long, value_name = "PATH", conflicts_with = "description")]
    pub description_file: Option<std::path::PathBuf>,
    /// Priority
    #[arg(long)]
    pub priority: Option<String>,
    /// Severity
    #[arg(long)]
    pub severity: Option<String>,
    /// Assignee
    #[arg(long)]
    pub assignee: Option<String>,
    /// Operating system (required by some Bugzilla installations)
    #[arg(long)]
    pub op_sys: Option<String>,
    /// Hardware platform (required by some Bugzilla installations)
    #[arg(long)]
    pub rep_platform: Option<String>,
    /// Bug IDs that this bug blocks (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub blocks: Vec<u64>,
    /// Bug IDs that this bug depends on (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub depends_on: Vec<u64>,
    #[command(flatten)]
    pub create_fields: CreateFieldArgs,
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
