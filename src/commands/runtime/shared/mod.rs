//! Shared command runtime helpers grouped by architectural role.

mod body_source;
mod connection;
mod merge;

pub(crate) use body_source::{
    classify_body_source, materialize_body_source, materialize_comment_body,
    materialize_optional_comment_body, read_file_with_context, read_stdin_to_string,
    CommentBodyRequirement,
};
pub(crate) use connection::connect_and_configure;
pub(crate) use merge::{merge_set, merge_vec};

#[cfg(test)]
use body_source::{read_to_string_from, BodySource};
#[cfg(test)]
use connection::{
    classify_and_handle_tls_failure, detect_and_build_client, detect_with_tofu_fallback,
    extract_hostname, handle_pin_rotation, handle_tofu, persist_detected_settings, probe_tls,
    should_offer_tofu, tls_uses_default_trust, ConnectContext,
};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
