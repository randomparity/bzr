mod attachment;
mod bug;
mod classification;
mod comment;
mod formatting;
mod config;
mod field;
mod group;
mod product;
mod result_types;
mod server;
mod user;

// Re-export shared types and helpers used by commands.
pub use result_types::{
    print_result, ActionResult, ConfigResult, DownloadResult, MembershipResult, ResourceKind,
    SearchResult, TagResult, UploadResult,
};

// Re-export all public items from submodules.
pub use attachment::print_attachments;
pub use bug::{print_bug_detail, print_bugs, print_history};
pub use comment::print_comments;
pub use config::{print_config, ConfigView};
pub use field::print_field_values;
pub use group::print_group_info;
pub use classification::print_classification;
pub use product::{print_product_detail, print_products};
pub use server::print_server_info;
pub use user::{print_users, print_users_detailed, print_whoami};
