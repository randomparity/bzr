use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum GroupAction {
    /// Add a user to a group.
    ///
    /// Requires Bugzilla admin permissions. The `--user` value can
    /// be a login name, an email address, or a numeric user ID.
    /// On Bugzilla 5.3+, this operation uses POST internally
    /// (handled automatically by bzr).
    ///
    /// Examples:
    ///
    ///   bzr group add-user --group editbugs --user alice@example.com
    ///   bzr group add-user --group admins --user bob
    ///
    /// See bzr-group-remove-user(1) for the inverse and
    /// bzr-group-list-users(1) to confirm membership.
    #[command(verbatim_doc_comment)]
    AddUser {
        /// Group name
        #[arg(long)]
        group: String,
        /// User email/login
        #[arg(long)]
        user: String,
    },

    /// Remove a user from a group.
    ///
    /// Requires Bugzilla admin permissions. The `--user` value can
    /// be a login name, an email address, or a numeric user ID.
    /// Removing a user from a group does not delete or disable the
    /// account -- only their group membership is revoked.
    ///
    /// Examples:
    ///
    ///   bzr group remove-user --group editbugs --user alice@example.com
    ///   bzr group remove-user --group admins --user bob
    ///
    /// See bzr-group-add-user(1) for the inverse and
    /// bzr-user-update(1) to disable a user account entirely.
    #[command(verbatim_doc_comment)]
    RemoveUser {
        /// Group name
        #[arg(long)]
        group: String,
        /// User email/login
        #[arg(long)]
        user: String,
    },

    /// List the users in a group.
    ///
    /// Prints the login name and real name of every member of the
    /// group. `--details` adds login status (enabled, disabled,
    /// last-login) and other group memberships. Visibility depends
    /// on server configuration -- some installations require admin
    /// privileges to enumerate group members.
    ///
    /// Examples:
    ///
    ///   bzr group list-users --group editbugs
    ///   bzr group list-users --group editbugs --details --json
    ///
    /// See bzr-group-view(1) for group metadata and
    /// bzr-group-add-user(1) / bzr-group-remove-user(1) to modify
    /// membership.
    #[command(verbatim_doc_comment)]
    ListUsers {
        /// Group name
        #[arg(long)]
        group: String,
        /// Show extended details (groups, login status)
        #[arg(long)]
        details: bool,
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },

    /// View group metadata (description, active flag, ID).
    ///
    /// Prints the group's name, description, active status, and
    /// numeric ID. The positional argument can be either the group
    /// name or its numeric ID.
    ///
    /// Examples:
    ///
    ///   bzr group view editbugs
    ///   bzr group view 42 --json
    ///
    /// See bzr-group-list-users(1) to enumerate members.
    #[command(verbatim_doc_comment)]
    View {
        /// Group name or ID
        group: String,
        #[command(flatten)]
        projection: crate::cli::ProjectionArgs,
    },

    /// Create a new Bugzilla group (admin only).
    ///
    /// Requires Bugzilla admin permissions. Both `--name` and
    /// `--description` are required. New groups are active by
    /// default; pass `--is-active false` to create a disabled
    /// group.
    ///
    /// Examples:
    ///
    ///   bzr group create --name release-eng --description "Release Engineering"
    ///   bzr group create --name temp --description "Temporary" \
    ///     --is-active false
    ///
    /// See bzr-group-update(1) to modify a group and
    /// bzr-group-add-user(1) to populate it.
    #[command(verbatim_doc_comment)]
    Create {
        /// Read group fields from a JSON object (`-` reads stdin)
        #[arg(long, value_name = "PATH")]
        from_json: Option<String>,
        /// Group name
        #[arg(long, required_unless_present = "from_json")]
        name: Option<String>,
        /// Group description
        #[arg(long, required_unless_present = "from_json")]
        description: Option<String>,
        /// Whether the group is active
        #[arg(long)]
        is_active: Option<bool>,
    },

    /// Update an existing group's description or active state.
    ///
    /// Requires Bugzilla admin permissions. Pass `--description`
    /// to change the description and/or `--is-active` to enable or
    /// disable the group. Renaming a group is not supported by the
    /// Bugzilla REST API.
    ///
    /// Examples:
    ///
    ///   bzr group update editbugs --description "Bug-edit privileges"
    ///   bzr group update temp --is-active false
    ///
    /// See bzr-group-create(1) for new groups and
    /// bzr-group-view(1) to inspect current state.
    #[command(verbatim_doc_comment)]
    Update {
        /// Read group update fields from a JSON object (`-` reads stdin)
        #[arg(long, value_name = "PATH")]
        from_json: Option<String>,
        /// Group name or ID
        #[arg(required_unless_present = "from_json")]
        group: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// Whether the group is active
        #[arg(long)]
        is_active: Option<bool>,
    },
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
