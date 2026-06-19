use clap::Subcommand;

#[derive(Subcommand)]
pub enum CommentAction {
    /// List all comments on a bug.
    ///
    /// Prints each comment's number, author, creation time, tags,
    /// and body in chronological order. Comment 0 is the bug's
    /// initial description. Use `--since` (ISO 8601 date or
    /// datetime) to limit output to comments newer than the given
    /// point.
    ///
    /// Examples:
    ///
    ///   bzr comment list 12345
    ///   bzr comment list 12345 --since 2026-01-01
    ///   bzr comment list 12345 --json | jq '.comments | length'
    ///
    /// See bzr-bug-history(1) for the full bug change log including
    /// non-comment events and bzr-comment-add(1) to post a comment.
    #[command(verbatim_doc_comment)]
    List {
        /// Bug ID
        bug_id: u64,
        /// Only show comments created after this date (ISO 8601)
        #[arg(long)]
        since: Option<String>,
    },

    /// Add a comment to a bug.
    ///
    /// The comment body is resolved in this order of precedence:
    /// `--body "..."`, `--body-file PATH`, piped stdin, then
    /// `$EDITOR` (when stdin is a terminal and no flag is set). A
    /// value of `-` for `--body` or `--body-file` reads the body
    /// from stdin (`echo text | bzr comment add 1 --body -`).
    /// `--body` and `--body-file` are mutually exclusive. Empty
    /// bodies are rejected with exit code 7 (input validation).
    ///
    /// Comments are immutable once posted -- subsequent edits or
    /// retractions are not possible via the REST API. To flag a
    /// comment for review, use `bzr comment tag`.
    ///
    /// Examples:
    ///
    ///   bzr comment add 12345 --body "Reproduced on RHEL 9.4"
    ///   echo "see also #6789" | bzr comment add 12345 --body -
    ///   bzr comment add 12345 --body-file notes.txt
    ///   bzr comment add 12345     # opens $EDITOR
    ///
    /// See bzr-comment-list(1) for the existing thread and
    /// bzr-comment-tag(1) for tagging comments after they're posted.
    #[command(verbatim_doc_comment)]
    Add {
        /// Bug ID
        bug_id: u64,
        /// Comment text.
        ///
        /// A value of `-` reads the body from stdin. Mutually
        /// exclusive with `--body-file`. When both are omitted, bzr
        /// reads from stdin if it's piped, otherwise it opens
        /// `$EDITOR` (or `vi` if `$EDITOR` is unset). Empty bodies
        /// are rejected with exit code 7.
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,
        /// Read the comment body from a UTF-8 file.
        ///
        /// A path of `-` reads from stdin. Mutually exclusive with
        /// `--body`; missing or non-UTF-8 paths exit 7.
        #[arg(long, value_name = "PATH", conflicts_with = "body")]
        body_file: Option<std::path::PathBuf>,
        /// Mark the comment as private (visible only to users with
        /// elevated permissions on the server).
        #[arg(long = "private")]
        private: bool,
    },

    /// Add or remove tags on a comment.
    ///
    /// Tags are server-side labels attached to a specific comment
    /// (not to the bug as a whole). They are most often used to
    /// flag spam, useful explanations, or comments that need
    /// follow-up. `--add` and `--remove` are repeatable; both can
    /// be combined in one invocation.
    ///
    /// Tag visibility and edit permissions depend on server
    /// configuration -- some installations restrict tag editing to
    /// users in a specific group.
    ///
    /// Examples:
    ///
    ///   bzr comment tag 200 --add spam
    ///   bzr comment tag 200 --add useful --remove spam
    ///   bzr comment tag 200 --add tag1 --add tag2
    ///
    /// See bzr-comment-search-tags(1) to find comments by tag.
    #[command(verbatim_doc_comment)]
    Tag {
        /// Comment ID
        comment_id: u64,
        /// Tags to add
        #[arg(long)]
        add: Vec<String>,
        /// Tags to remove
        #[arg(long)]
        remove: Vec<String>,
    },

    /// Search comments by tag.
    ///
    /// Returns a list of comments whose tag set matches the query.
    /// The exact match semantics depend on the Bugzilla server --
    /// most installations do substring matching on the tag name.
    ///
    /// Examples:
    ///
    ///   bzr comment search-tags spam
    ///   bzr comment search-tags useful --json
    ///
    /// See bzr-comment-tag(1) to add or remove tags.
    #[command(verbatim_doc_comment)]
    SearchTags {
        /// Tag query
        query: String,
    },
}
