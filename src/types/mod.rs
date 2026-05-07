mod attachment;
mod bug;
mod comment;
mod common;
mod group;
mod product;
mod user;

pub use attachment::{Attachment, UpdateAttachmentParams, UploadAttachmentParams};
pub use bug::{
    partition_filters, Bug, BugTemplate, CreateBugParams, FieldChange, FieldMapping, FieldValue,
    HistoryEntry, IdListUpdate, NegationOp, Overrides, QueryKind, SavedQuery, SearchParams,
    StatusTransition, UpdateBugParams, FIELD_MAPPINGS,
};
pub use comment::{Comment, UpdateCommentTagsParams};
pub use common::{
    ApiMode, AuthMethod, ExtensionInfo, FlagStatus, FlagUpdate, OutputFormat, ServerExtensions,
    ServerInfoResponse, ServerVersion,
};
pub use group::{CreateGroupParams, GroupInfo, GroupMember, UpdateGroupParams};
pub use product::{
    Classification, ClassificationProduct, Component, CreateComponentParams, CreateProductParams,
    Milestone, Product, ProductListType, UpdateComponentParams, UpdateProductParams, Version,
};
pub use user::{BugzillaUser, CreateUserParams, UpdateUserParams, UserGroup, WhoamiResponse};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
