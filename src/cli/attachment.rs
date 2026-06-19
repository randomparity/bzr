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

    /// View a single attachment's metadata by attachment ID.
    ///
    /// Prints the summary, bug, file name, content type, size, flags
    /// (patch/obsolete/private), creator, and timestamps -- without
    /// downloading the attachment's bytes. On REST the `data` field is
    /// excluded server-side, so large attachments are cheap to inspect.
    ///
    /// Examples:
    ///
    ///   bzr attachment view 9876
    ///   bzr --json attachment view 9876 | jq '.summary, .size'
    ///
    /// See bzr-attachment-list(1) to discover attachment IDs for a bug and
    /// bzr-attachment-download(1) to fetch the bytes.
    #[command(verbatim_doc_comment)]
    View {
        /// Attachment ID
        attachment_id: u64,
    },

    /// Download attachments to disk.
    ///
    /// Three argument shapes are accepted:
    ///
    /// 1. Single attachment, free-form path:
    ///    bzr attachment download 9876 --out patch.diff
    ///
    /// 2. Multiple attachment IDs (positional):
    ///    bzr attachment download 9876 9877 9878 [--out-dir DIR]
    ///
    /// 3. Every attachment of one or more bugs (mixable with #2):
    ///    bzr attachment download --bug 12345 [--bug 67890] \[9876\] [--out-dir DIR]
    ///
    /// In shapes 2 and 3, files are written to
    /// `<out-dir>/<bug-id>/<att-id>.<file_name>`. The attachment-ID
    /// prefix avoids same-name collisions on a single bug. `--out-dir`
    /// defaults to `./attachments` and is created (recursively) on
    /// demand. `--out` is only valid for shape 1.
    ///
    /// Examples:
    ///
    ///   bzr attachment download 9876
    ///   bzr attachment download 9876 --out patch.diff
    ///   bzr attachment download 9876 9877 9878 --out-dir /tmp/patches
    ///   bzr attachment download --bug 12345 --bug 67890 --out-dir /tmp/all
    ///   bzr attachment download --bug 12345 9876 --out-dir /tmp/mixed
    ///
    /// See bzr-attachment-list(1) to discover IDs for a bug.
    #[command(verbatim_doc_comment)]
    Download {
        /// Attachment ID(s) to download. Optional when --bug is set.
        #[arg(value_name = "ID")]
        ids: Vec<u64>,

        /// Download every attachment for the given bug. Repeatable.
        #[arg(long = "bug", value_name = "BUG_ID")]
        bug_ids: Vec<u64>,

        /// Output file path (single-attachment shape only).
        #[arg(
            short = 'o',
            long = "out",
            id = "out_file",
            conflicts_with_all = ["out_dir", "bug_ids"],
        )]
        out: Option<String>,

        /// Output directory for batch downloads.
        #[arg(long = "out-dir", default_value = "./attachments")]
        out_dir: String,
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
    /// `--comment-private` (used together with `--comment`) marks the
    /// new comment private after upload. This requires `editbugs`
    /// permission. Permission errors leave the attachment uploaded and
    /// the comment public; rerun privacy via the web UI.
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
    ///   bzr attachment upload 12345 patch.diff \
    ///     --comment "private context" --comment-private
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
        /// Mark the new attachment as a patch.
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
        /// inherits the bug's default privacy unless `--comment-private`
        /// is also set, in which case a follow-up `Bug.update` call
        /// flips the new comment private (two API round-trips total).
        #[arg(long)]
        comment: Option<String>,
        /// Mark the comment posted via `--comment` private.
        ///
        /// Issues a follow-up `Bug.update` call after the upload to flip
        /// the just-created comment's `is_private` flag. The Bugzilla
        /// `Bug.add_attachment` API does not accept comment privacy in
        /// the upload itself, so this is a two-call workflow internally.
        ///
        /// Requires `--comment <BODY>`; using `--comment-private` alone
        /// is a usage error (exit 7). Marking a comment private requires
        /// `editbugs` permission or `insider` group membership; on 403 the
        /// attachment remains uploaded but the comment stays public, and
        /// the command exits non-zero with a stderr warning.
        #[arg(long)]
        comment_private: bool,
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
