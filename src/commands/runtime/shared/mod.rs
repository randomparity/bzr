//! Shared command runtime helpers grouped by architectural role.

mod body_source;
mod capability;
mod connection;
mod field_catalogue;
mod merge;

pub(crate) use body_source::{
    classify_body_source, materialize_body_source, materialize_comment_body,
    materialize_optional_comment_body, read_file_with_context, read_stdin_to_string,
    CommentBodyRequirement,
};
pub(crate) use capability::{require_server_capability, RED_HAT_EXTENSION};
pub(crate) use connection::connect_and_configure;
pub(crate) use field_catalogue::{
    accepted_bug_fields, connect_and_validate_bug_fields, validate_bug_fields,
};
pub(crate) use merge::{merge_set, merge_vec};

#[cfg(test)]
use body_source::{read_to_string_from, BodySource};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
