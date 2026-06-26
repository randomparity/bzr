pub(crate) mod attachment;
pub(crate) mod bug;
pub(crate) mod capabilities;
pub(crate) mod classification;
pub(crate) mod comment;
pub(crate) mod component;
pub(crate) mod field;
pub(crate) mod flag;
pub(crate) mod group;
pub(crate) mod output;
pub(crate) mod product;
pub(crate) mod query;
pub(crate) mod server_info;
pub(crate) mod template;
pub(crate) mod transport;
pub(crate) mod user;

pub use attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
pub use bug::{
    partition_filters, Bug, ColumnSpec, CommentUpdate, CreateBugParams, FieldChange, FieldMapping,
    FilterField, HistoryEntry, IdListUpdate, NegationOp, Overrides, SearchParams, StringListUpdate,
    UpdateBugParams, FIELD_MAPPINGS,
};
pub use capabilities::{
    CustomFieldSummary, FlagTypeSummary, ServerCapabilities, StatusTransitionSummary,
};
pub use classification::{Classification, ClassificationProduct};
pub use comment::{AddCommentParams, Comment, UpdateCommentTagsParams};
pub use component::{Component, CreateComponentParams, UpdateComponentParams};
pub use field::{FieldValue, StatusTransition};
pub use flag::{Flag, FlagStatus, FlagUpdate};
pub use group::{CreateGroupParams, GroupInfo, GroupMember, UpdateGroupParams};
pub use output::{OutputFormat, SortDirection};
pub use product::{
    CreateProductParams, Milestone, Product, ProductListType, UpdateProductParams, Version,
};
pub use query::{QueryKind, SavedQuery};
pub use server_info::{ExtensionInfo, ServerExtensions, ServerInfoResponse, ServerVersion};
pub use template::BugTemplate;
pub use transport::{ApiMode, AuthMethod};
pub use user::{BugzillaUser, CreateUserParams, UpdateUserParams, UserGroup, WhoamiResponse};

pub(crate) use field::{resolve_field_alias, FIELD_ALIASES};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
