mod attachment;
mod bug;
mod bug_fields;
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
    FieldValue, FilterField, HistoryEntry, IdListUpdate, NegationOp, Overrides, QueryKind,
    SavedQuery, SearchParams, StatusTransition, StringListUpdate, UpdateBugParams, FIELD_MAPPINGS,
};
pub use bug_fields::ColumnSpec;
pub(crate) use bug_fields::{
    apply_exclude, canonical_excludes, canonical_field_list, default_selected_fields,
    field_selected, partition_include, selected_custom_detail_fields, selected_keys, BugField,
    SelectedBugField, BUG_FIELDS,
};
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
