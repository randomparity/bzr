mod attachment;
mod bug;
mod classification;
mod comment;
mod common;
mod component;
mod group;
mod product;
mod user;

pub use attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
pub use bug::{
    partition_filters, Bug, BugTemplate, CommentUpdate, CreateBugParams, FieldChange, FieldMapping,
    FieldValue, HistoryEntry, IdListUpdate, NegationOp, Overrides, QueryKind, SavedQuery,
    SearchParams, StatusTransition, StringListUpdate, UpdateBugParams, FIELD_MAPPINGS,
};
pub use classification::{Classification, ClassificationProduct};
pub use comment::{Comment, UpdateCommentTagsParams};
pub use common::{
    ApiMode, AuthMethod, ExtensionInfo, FlagStatus, FlagUpdate, OutputFormat, ServerExtensions,
    ServerInfoResponse, ServerVersion, SortDirection,
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
