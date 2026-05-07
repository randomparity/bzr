use clap::Subcommand;

#[derive(Subcommand)]
pub enum AttachmentAction {
    /// List all attachments on a bug.
    ///
    /// Prints attachment ID, summary, file name, MIME type, size,
    /// creator, and creation date for every attachment on the given
    /// bug. Use `--json` for a structured listing suitable for
    /// scripting.
    ///
    /// Examples:
    ///
    ///   bzr attachment list 12345
    ///   bzr attachment list 12345 --json | jq '.attachments[].id'
    ///
    /// See bzr-attachment-download(1) to fetch one and
    /// bzr-attachment-upload(1) to add a new attachment.
    #[command(verbatim_doc_comment)]
    List {
        /// Bug ID
        bug_id: u64,
    },

    /// Download an attachment to a local file.
    ///
    /// Fetches the attachment by ID and writes the decoded bytes to
    /// disk. With `--out`/`-o`, the file is written to the given
    /// path; otherwise the attachment's stored file name is used in
    /// the current directory. Base64 decoding is handled transparently;
    /// the on-disk file is the original binary.
    ///
    /// Examples:
    ///
    ///   bzr attachment download 9876
    ///   bzr attachment download 9876 --out patch.diff
    ///   bzr attachment download 9876 -o /tmp/sample.bin
    ///
    /// See bzr-attachment-list(1) to discover IDs for a bug.
    #[command(verbatim_doc_comment)]
    Download {
        /// Attachment ID
        id: u64,
        /// Output file path (defaults to original filename)
        #[arg(short = 'o', long = "out", id = "out_file")]
        out: Option<String>,
    },

    /// Upload a file as an attachment on a bug.
    ///
    /// Reads the local file at `<file>` and uploads it as an
    /// attachment on `<bug_id>`. MIME type is auto-detected from the
    /// file extension and may be overridden with `--content-type`.
    /// `--summary` sets the attachment's display label;
    /// it defaults to the file name if omitted.
    ///
    /// `--comment <BODY>` posts a comment alongside the attachment
    /// in a single API call. Use this when the attachment needs
    /// explanatory context — typical for patches.
    ///
    /// `--flag` accepts Bugzilla flag syntax (`name?`, `name+`,
    /// `name-`, `name?(user@example.com)`) and is repeatable.
    /// Requires credentials with attach-file permission on the
    /// target product.
    ///
    /// Examples:
    ///
    ///   bzr attachment upload 12345 patch.diff
    ///   bzr attachment upload 12345 trace.log --summary "Stack trace" \
    ///     --content-type text/plain
    ///   bzr attachment upload 12345 fix.patch \
    ///     --flag 'review?(maintainer@example.com)'
    ///   bzr attachment upload 12345 fix.patch --comment "see #4567 for context"
    ///   bzr attachment upload 12345 fix.patch --is-patch
    ///
    /// See bzr-attachment-update(1) to modify metadata after upload.
    #[command(verbatim_doc_comment)]
    Upload {
        /// Bug ID
        bug_id: u64,
        /// File to upload
        file: String,
        /// Attachment summary/description
        #[arg(long)]
        summary: Option<String>,
        /// MIME type (auto-detected if not provided)
        #[arg(long)]
        content_type: Option<String>,
        /// Mark the new attachment as private.
        ///
        /// Private attachments are visible only to users in the bug's
        /// `insider` group (server-configured). Use carefully.
        #[arg(long)]
        private: bool,
        /// Mark the new attachment as a patch (boolean, presence-only).
        ///
        /// Patches render as side-by-side diffs in the Bugzilla web UI
        /// and default `--content-type` to `text/plain` when no explicit
        /// content type is supplied. Use this for `.diff` / `.patch`
        /// files instead of a follow-up `bzr attachment update --is-patch true`.
        #[arg(long)]
        is_patch: bool,
        /// Post a comment alongside the attachment in the same request.
        ///
        /// Folded into the underlying `Bug.add_attachment` API call so
        /// the comment and attachment share a creation timestamp and
        /// are visible to the same audience as the bug. The comment
        /// inherits the bug's default privacy; there is no Bugzilla
        /// API for marking an `add_attachment` comment private — use
        /// `bzr comment add --private` separately if needed.
        #[arg(long)]
        comment: Option<String>,
        /// Set, request, or clear a flag using Bugzilla flag syntax.
        ///
        /// Repeatable. Accepted forms:
        /// `name+` (granted), `name-` (denied), `name?` (request),
        /// `name?(user@example.com)` (request a specific user), or
        /// `name?,!` to clear an existing flag.
        #[arg(long)]
        flag: Vec<String>,
    },

    /// Update an existing attachment's metadata or flags.
    ///
    /// Modifies metadata only -- the attachment's file content
    /// itself cannot be replaced via the REST API. Pass any of the
    /// flags to change that property: `--summary`, `--file-name`,
    /// `--content-type`, `--obsolete`, `--is-patch`, `--is-private`,
    /// or `--flag`. Boolean flags accept `true` or `false` and are
    /// only applied when explicitly supplied.
    ///
    /// `--flag` uses Bugzilla flag syntax (see bzr-attachment-upload(1));
    /// pass `name?,!` to clear an existing flag.
    ///
    /// Examples:
    ///
    ///   bzr attachment update 9876 --obsolete true
    ///   bzr attachment update 9876 --summary "Updated patch v2"
    ///   bzr attachment update 9876 --flag 'review+'
    ///
    /// See bzr-attachment-upload(1) to attach a new file in place
    /// of an obsoleted one.
    #[command(verbatim_doc_comment)]
    Update {
        /// Attachment ID
        id: u64,
        /// New summary
        #[arg(long)]
        summary: Option<String>,
        /// New file name
        #[arg(long)]
        file_name: Option<String>,
        /// New content type
        #[arg(long)]
        content_type: Option<String>,
        /// Mark as obsolete (`true`) or un-obsolete (`false`).
        ///
        /// Only applied when explicitly supplied; an unset value
        /// leaves the flag unchanged. Mark a patch obsolete when
        /// uploading a replacement to keep the bug's attachment
        /// list tidy.
        #[arg(long)]
        obsolete: Option<bool>,
        /// Mark as a patch (`true`) or non-patch (`false`).
        ///
        /// Only applied when explicitly supplied. The patch flag
        /// affects diff rendering in the Bugzilla web UI but is
        /// otherwise informational.
        #[arg(long)]
        is_patch: Option<bool>,
        /// Mark as private (`true`) or public (`false`).
        ///
        /// Only applied when explicitly supplied. Private
        /// attachments are visible only to users in the bug's
        /// `insider` group (server-configured); use carefully.
        #[arg(long)]
        is_private: Option<bool>,
        /// Set, request, or clear a flag using Bugzilla flag syntax.
        ///
        /// Repeatable. Accepted forms:
        /// `name+` (granted), `name-` (denied), `name?` (request),
        /// `name?(user@example.com)` (request a specific user), or
        /// `name?,!` to clear an existing flag.
        #[arg(long)]
        flag: Vec<String>,
    },
}
