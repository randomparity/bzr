//! Output formatting facade — command modules import from `output::*` so they
//! don't couple to which submodule owns each formatter. This keeps internal
//! reorganization (e.g. moving `write_classification` to its own file) invisible
//! to callers.

mod attachment;
mod bug;
mod classification;
mod comment;
mod config;
mod field;
mod formatting;
mod group;
mod product;
mod query;
mod result_types;
mod server;
mod template;
mod user;
mod writers;

// Re-export shared types and helpers used by commands.
pub(crate) use formatting::write_divider;
pub use result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, BugViewFailure, ConfigResult,
    DownloadResult, MembershipResult, MultiBugViewResult, ResourceKind, SearchResult, TagResult,
    UploadResult,
};
pub use writers::Writers;

// Re-export all public items from submodules.
pub use attachment::{
    write_attachment_batch, write_attachments, AttachmentBatchResult, AttachmentDownloadResult,
    BatchSummary, BugDownloadResult, DownloadedFile, TargetStatus,
};
pub use bug::{write_bug_detail, write_bugs, write_history, write_multi_bug_view, MultiBugRow};
pub use classification::write_classification;
pub use comment::write_comments;
pub use config::ServerDisplayInfo;
pub use config::{write_config, ConfigView};
pub use field::{write_field_aliases, write_field_values};
pub use group::write_group_info;
pub use product::{write_product_detail, write_products};
pub use query::{write_query_detail, write_query_list, write_query_saved};
pub use server::write_server_info;
pub use template::{write_template_detail, write_template_list, write_template_saved};
pub use user::{write_users, write_users_detailed, write_whoami};
