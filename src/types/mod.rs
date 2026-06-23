pub(crate) mod attachment;
pub(crate) mod bug;
pub(crate) mod bug_fields;
pub(crate) mod classification;
pub(crate) mod comment;
pub(crate) mod common;
pub(crate) mod component;
pub(crate) mod group;
pub(crate) mod product;
pub(crate) mod user;

pub use attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
pub use bug::{
    partition_filters, Bug, BugTemplate, CommentUpdate, CreateBugParams, FieldChange, FieldMapping,
    FieldValue, FilterField, HistoryEntry, IdListUpdate, NegationOp, Overrides, QueryKind,
    SavedQuery, SearchParams, StatusTransition, StringListUpdate, UpdateBugParams, FIELD_MAPPINGS,
};
pub use bug_fields::ColumnSpec;
pub use classification::{Classification, ClassificationProduct};
pub use comment::{AddCommentParams, Comment, UpdateCommentTagsParams};
pub use common::{
    ApiMode, AuthMethod, ExtensionInfo, Flag, FlagStatus, FlagUpdate, OutputFormat,
    ServerExtensions, ServerInfoResponse, ServerVersion, SortDirection,
};
pub use component::{Component, CreateComponentParams, UpdateComponentParams};
pub use group::{CreateGroupParams, GroupInfo, GroupMember, UpdateGroupParams};
pub use product::{
    CreateProductParams, Milestone, Product, ProductListType, UpdateProductParams, Version,
};
pub use user::{BugzillaUser, CreateUserParams, UpdateUserParams, UserGroup, WhoamiResponse};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
