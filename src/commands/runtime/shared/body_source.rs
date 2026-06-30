use std::fmt;

use crate::error::{io_with_context, BzrError, Result};

/// Where a body string comes from. Pure result of classifying the
/// inline + file flag pair; carries no I/O.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodySource {
    /// A literal inline value (`--body "text"`).
    Literal(String),
    /// Read all of stdin from the named flag (`--body -` or `--*-file -`).
    Stdin { flag: String },
    /// Read a UTF-8 file (`--*-file PATH`, PATH != "-").
    File(std::path::PathBuf),
    /// Neither source supplied; the caller applies its own fallback.
    None,
}

#[derive(Clone, Copy)]
pub(crate) enum CommentBodyRequirement {
    Optional,
    RequiredWithFallback(fn() -> Result<String>),
    PrivateRequiresBody,
}

impl CommentBodyRequirement {
    fn optional_or_private_required(comment_private: bool) -> Self {
        if comment_private {
            Self::PrivateRequiresBody
        } else {
            Self::Optional
        }
    }
}

/// Classify an inline flag value and/or a `--*-file` path into a
/// [`BodySource`], honouring the `-` = stdin convention for both. The two
/// sources are mutually exclusive; clap `conflicts_with` makes the
/// both-present case unreachable in normal use, but this guards regardless so
/// correctness does not depend on the CLI layer. `inline_flag` / `file_flag`
/// name the originating options for the mutual-exclusion error message.
pub(crate) fn classify_body_source(
    inline: Option<&str>,
    file: Option<&std::path::Path>,
    inline_flag: &str,
    file_flag: &str,
) -> Result<BodySource> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(BzrError::input(format!(
            "{inline_flag} and {file_flag} are mutually exclusive"
        ))),
        (Some("-"), None) => Ok(BodySource::Stdin {
            flag: inline_flag.to_string(),
        }),
        (Some(s), None) => Ok(BodySource::Literal(s.to_string())),
        (None, Some(path)) => {
            if path == std::path::Path::new("-") {
                Ok(BodySource::Stdin {
                    flag: file_flag.to_string(),
                })
            } else {
                Ok(BodySource::File(path.to_path_buf()))
            }
        }
        (None, None) => Ok(BodySource::None),
    }
}

/// Read everything from `reader` into a `String`, mapping I/O failures
/// (including non-UTF-8 input) to a `BzrError`.
pub(super) fn read_to_string_from(
    reader: &mut impl std::io::Read,
    context: impl fmt::Display,
) -> Result<String> {
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .map_err(|e| io_with_context(context, &e))?;
    Ok(buf)
}

/// Read all of stdin to a `String`. Shared by the `-` (dash) convention and
/// the flag-omitted auto-stdin paths.
pub(crate) fn read_stdin_to_string(context: impl fmt::Display) -> Result<String> {
    read_to_string_from(&mut std::io::stdin().lock(), context)
}

/// Turn a classified [`BodySource`] into the actual body, or `Ok(None)` for
/// [`BodySource::None`] so the caller applies its own fallback. This is the
/// only place that performs stdin/file I/O for the explicit body flags.
pub(crate) fn materialize_body_source(
    source: BodySource,
    file_flag: &str,
) -> Result<Option<String>> {
    match source {
        BodySource::Literal(s) => Ok(Some(s)),
        BodySource::Stdin { flag } => Ok(Some(read_stdin_to_string(format!(
            "read {flag} from stdin"
        ))?)),
        BodySource::File(path) => Ok(Some(read_file_with_context(&path, file_flag)?)),
        BodySource::None => Ok(None),
    }
}

/// Materialize a comment body according to the command's body requirement.
///
/// Explicit sources always reject whitespace-only text. When no explicit source
/// is present, `requirement` decides whether absence is allowed, should fall
/// back to a command reader, or is a `--comment-private` usage error.
pub(crate) fn materialize_comment_body(
    source: BodySource,
    file_flag: &str,
    requirement: CommentBodyRequirement,
) -> Result<Option<String>> {
    let Some(text) = materialize_body_source(source, file_flag)? else {
        return match requirement {
            CommentBodyRequirement::Optional => Ok(None),
            CommentBodyRequirement::RequiredWithFallback(read_fallback) => {
                Ok(Some(require_non_empty_comment_body(read_fallback()?)?))
            }
            CommentBodyRequirement::PrivateRequiresBody => Err(BzrError::input(
                "--comment-private requires --comment or --comment-file".to_string(),
            )),
        };
    };
    Ok(Some(require_non_empty_comment_body(text)?))
}

/// Materialize optional `--comment` / `--comment-file` input.
pub(crate) fn materialize_optional_comment_body(
    comment: Option<&str>,
    comment_file: Option<&std::path::Path>,
    comment_private: bool,
) -> Result<Option<String>> {
    materialize_comment_body(
        classify_body_source(comment, comment_file, "--comment", "--comment-file")?,
        "--comment-file",
        CommentBodyRequirement::optional_or_private_required(comment_private),
    )
}

/// Return comment text when it has non-whitespace content.
fn require_non_empty_comment_body(text: String) -> Result<String> {
    if text.trim().is_empty() {
        return Err(BzrError::input("empty comment, aborting".into()));
    }
    Ok(text)
}

/// Read a UTF-8 file, mapping any I/O error to an `InputValidation` error that
/// names the originating CLI flag and the path. `flag` is the user-facing
/// option name (e.g. `--description-file`) used to prefix the message.
pub(crate) fn read_file_with_context(path: &std::path::Path, flag: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        BzrError::input(format!(
            "{flag} could not be read ({}): {e}",
            path.display()
        ))
    })
}
