use clap::Subcommand;

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub(crate) enum FieldAction {
    /// Show the user-friendly aliases bzr accepts for Bugzilla field names.
    ///
    /// Prints the table of short alias to internal field name (e.g.
    /// `status` to `bug_status`, `severity` to `bug_severity`).
    /// These aliases are accepted by `bzr field list <name>` and by
    /// any other command that names a field, so users don't have to
    /// remember the underlying Bugzilla field naming.
    ///
    /// Examples:
    ///
    ///   bzr field aliases
    ///   bzr field aliases --json
    ///
    /// See bzr-field-list(1) to enumerate the legal values of one
    /// field.
    #[command(verbatim_doc_comment)]
    Aliases,

    /// List the legal values for a Bugzilla bug field.
    ///
    /// Prints every value the configured server accepts for the
    /// named field. Common aliases (`status`, `severity`,
    /// `priority`, `resolution`, ...) are resolved automatically to
    /// their underlying field names; the canonical names also work.
    /// Use this to discover legal values before passing
    /// `--status`, `--priority`, etc. to `bzr bug create` or
    /// `bzr bug update`.
    ///
    /// Examples:
    ///
    ///   bzr field list status
    ///   bzr field list priority --json
    ///   bzr field list bug_severity
    ///
    /// See bzr-field-aliases(1) for the alias table and
    /// bzr-bug-create(1) / bzr-bug-update(1) for the commands that
    /// consume these values.
    #[command(verbatim_doc_comment)]
    List {
        /// Field name (e.g. status, priority, severity, resolution).
        /// Common aliases are resolved automatically (status -> `bug_status`,
        /// severity -> `bug_severity`, etc.)
        name: String,
    },
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
