//! Output formatting facade — command modules import from `output::*` so they
//! don't couple to which submodule owns each formatter. This keeps internal
//! reorganization (e.g. moving `print_classification` to its own file) invisible
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

// Re-export shared types and helpers used by commands.
pub(crate) use formatting::write_divider;
pub use result_types::{
    print_result, ActionResult, BatchFailure, BatchResult, BugViewFailure, ConfigResult,
    DownloadResult, MembershipResult, MultiBugViewResult, ResourceKind, SearchResult, TagResult,
    UploadResult,
};

// Re-export all public items from submodules.
pub use attachment::print_attachments;
#[expect(
    unused_imports,
    reason = "consumed by print_attachment_batch renderer (Task 4) and dispatch (Task 6)"
)]
pub use attachment::{
    AttachmentBatchResult, AttachmentDownloadResult, BatchSummary, BugDownloadResult,
    DownloadedFile, TargetStatus,
};
pub use bug::{print_bug_detail, print_bugs, print_history, print_multi_bug_view, MultiBugRow};
pub use classification::print_classification;
pub use comment::print_comments;
#[expect(
    unused_imports,
    reason = "re-exported for API completeness — used as ConfigView field type"
)]
pub use config::ServerDisplayInfo;
pub use config::{print_config, ConfigView};
pub use field::{print_field_aliases, print_field_values};
pub use group::print_group_info;
pub use product::{print_product_detail, print_products};
pub use query::{print_query_detail, print_query_list, print_query_saved};
pub use server::print_server_info;
pub use template::{print_template_detail, print_template_list, print_template_saved};
pub use user::{print_users, print_users_detailed, print_whoami};
