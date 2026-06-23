use crate::error::{BzrError, Result};

/// Where a body string comes from. Pure result of classifying the
/// inline + file flag pair; carries no I/O.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodySource {
    /// A literal inline value (`--body "text"`).
    Literal(String),
    /// Read all of stdin (`--body -` or `--*-file -`).
    Stdin,
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
    pub(crate) fn optional_or_private_required(comment_private: bool) -> Self {
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
        (Some(_), Some(_)) => Err(BzrError::InputValidation(format!(
            "{inline_flag} and {file_flag} are mutually exclusive"
        ))),
        (Some("-"), None) => Ok(BodySource::Stdin),
        (Some(s), None) => Ok(BodySource::Literal(s.to_string())),
        (None, Some(path)) => {
            if path == std::path::Path::new("-") {
                Ok(BodySource::Stdin)
            } else {
                Ok(BodySource::File(path.to_path_buf()))
            }
        }
        (None, None) => Ok(BodySource::None),
    }
}

/// Read everything from `reader` into a `String`, mapping I/O failures
/// (including non-UTF-8 input) to a `BzrError`.
pub(super) fn read_to_string_from(reader: &mut impl std::io::Read) -> Result<String> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Read all of stdin to a `String`. Shared by the `-` (dash) convention and
/// the flag-omitted auto-stdin paths.
pub(crate) fn read_stdin_to_string() -> Result<String> {
    read_to_string_from(&mut std::io::stdin().lock())
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
        BodySource::Stdin => Ok(Some(read_stdin_to_string()?)),
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
            CommentBodyRequirement::PrivateRequiresBody => Err(BzrError::InputValidation(
                "--comment-private requires --comment or --comment-file".to_string(),
            )),
        };
    };
    Ok(Some(require_non_empty_comment_body(text)?))
}

/// Return comment text when it has non-whitespace content.
fn require_non_empty_comment_body(text: String) -> Result<String> {
    if text.trim().is_empty() {
        return Err(BzrError::InputValidation("empty comment, aborting".into()));
    }
    Ok(text)
}

/// Read a UTF-8 file, mapping any I/O error to an `InputValidation` error that
/// names the originating CLI flag and the path. `flag` is the user-facing
/// option name (e.g. `--description-file`) used to prefix the message.
pub(crate) fn read_file_with_context(path: &std::path::Path, flag: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        BzrError::InputValidation(format!(
            "{flag} could not be read ({}): {e}",
            path.display()
        ))
    })
}
