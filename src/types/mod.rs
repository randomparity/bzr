pub(crate) mod attachment;
pub(crate) mod bug;
pub(crate) mod bug_fields;
pub(crate) mod classification;
pub(crate) mod comment;
pub(crate) mod common;
pub(crate) mod component;
pub(crate) mod field;
pub(crate) mod group;
pub(crate) mod product;
pub(crate) mod query;
pub(crate) mod template;
pub(crate) mod user;

pub use attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
pub use bug::{
    partition_filters, Bug, CommentUpdate, CreateBugParams, FieldChange, FieldMapping, FilterField,
    HistoryEntry, IdListUpdate, NegationOp, Overrides, SearchParams, StringListUpdate,
    UpdateBugParams, FIELD_MAPPINGS,
};
pub use bug_fields::ColumnSpec;
pub use classification::{Classification, ClassificationProduct};
pub use comment::{AddCommentParams, Comment, UpdateCommentTagsParams};
pub use common::{
    ApiMode, AuthMethod, ExtensionInfo, Flag, FlagStatus, FlagUpdate, OutputFormat,
    ServerExtensions, ServerInfoResponse, ServerVersion, SortDirection,
};
pub use component::{Component, CreateComponentParams, UpdateComponentParams};
pub use field::{FieldValue, StatusTransition};
pub use group::{CreateGroupParams, GroupInfo, GroupMember, UpdateGroupParams};
pub use product::{
    CreateProductParams, Milestone, Product, ProductListType, UpdateProductParams, Version,
};
pub use query::{QueryKind, SavedQuery};
pub use template::BugTemplate;
pub use user::{BugzillaUser, CreateUserParams, UpdateUserParams, UserGroup, WhoamiResponse};

pub(crate) use field::{resolve_field_alias, FIELD_ALIASES};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
