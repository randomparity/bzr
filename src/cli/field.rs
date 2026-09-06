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
    /// See bzr-field-list(1), which enumerates the field names this
    /// server accepts when given no argument, and one field's legal
    /// values when given a name. Aliases apply to the named form only.
    #[command(verbatim_doc_comment)]
    Aliases,

    /// List the field names this server accepts, or one field's legal values.
    ///
    /// With no argument, prints every bug field name `bzr bug create`
    /// and `bzr bug update` accept for `--field` / `--field-json`, with
    /// a `source` column saying why each is accepted: `server` when the
    /// connected server's field catalogue declares it, `bzr` when bzr
    /// models it as a canonical REST bug field, `both` when both do.
    /// Bugzilla's catalogue reports internal column names for several
    /// built-ins (`status_whiteboard`, `short_desc`, `rep_platform`)
    /// while the write API takes the REST spellings (`whiteboard`,
    /// `summary`, `platform`); both are accepted and both are listed.
    /// A listed name is one bzr will not refuse, which is not a promise
    /// that Bugzilla will honour it.
    ///
    /// With a field name, prints every value the configured server
    /// accepts for that field. Common aliases (`status`, `severity`,
    /// `priority`, `resolution`, ...) are resolved automatically to
    /// their underlying field names; the canonical names also work.
    /// Use this to discover legal values before passing
    /// `--status`, `--priority`, etc. to `bzr bug create` or
    /// `bzr bug update`. Aliases apply to this form only.
    ///
    /// Examples:
    ///
    ///   bzr field list
    ///   bzr field list --json
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
        /// Omit it to list the field names this server accepts instead.
        /// Common aliases are resolved automatically (status -> `bug_status`,
        /// severity -> `bug_severity`, etc.)
        name: Option<String>,
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
