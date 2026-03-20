mod common;
mod attachment;
mod bug;
mod comment;
mod field;
mod group;
mod product;
mod server;
mod user;

// Re-export shared types and helpers used by commands.
pub use common::{print_result, ActionResult, ResourceKind};

// Re-export all public items from submodules.
pub use attachment::print_attachments;
pub use bug::{print_bug_detail, print_bugs, print_history};
pub use comment::print_comments;
pub use field::print_field_values;
pub use group::print_group_info;
pub use product::{print_classification, print_product_detail, print_products};
pub use server::{print_config, print_server_info, ServerInfo};
pub use user::{print_users, print_whoami};
