//! Output formatting facade — command modules import from `output::*` so they
//! don't couple to which submodule owns each formatter. This keeps internal
//! reorganization (e.g. moving `write_classification` to its own file) invisible
//! to callers.

mod formatting;
mod resources;
mod result_types;
mod writers;

// Re-export shared types and helpers used by commands.
pub use result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, BugViewFailure, ConfigResult,
    DownloadResult, MembershipResult, MultiBugViewResult, ResourceKind, SearchResult, TagResult,
    UploadResult,
};
pub use writers::Writers;

// Re-export all public items from submodules.
pub use resources::{
    write_attachment_batch, write_attachments, write_bug_detail, write_bugs, write_classification,
    write_comments, write_config, write_field_aliases, write_field_values, write_group_info,
    write_history, write_multi_bug_view, write_product_detail, write_products, write_query_detail,
    write_query_list, write_query_saved, write_server_info, write_template_detail,
    write_template_list, write_template_saved, write_users, write_users_detailed, write_whoami,
    AttachmentBatchResult, AttachmentDownloadResult, BatchSummary, BugDownloadResult, ConfigView,
    DownloadedFile, MultiBugRow, ServerDisplayInfo, TargetStatus,
};
