use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::comment::{write_comment_bug_header, write_comments};
use crate::output::writers::Writers;
use crate::types::comment::Comment;
use crate::types::OutputFormat;

/// Arguments for `comment list`, bundled so the handler keeps one parameter per
/// concern -- mirrors `add::AddArgs`.
pub(super) struct ListArgs<'a> {
    pub bug_ids: &'a [u64],
    pub permissive: bool,
    pub since: Option<&'a str>,
    pub projection: &'a crate::cli::ProjectionArgs,
}

pub(super) async fn handle(
    args: &ListArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let ListArgs {
        bug_ids,
        permissive,
        since,
        projection: projection_args,
    } = *args;
    validate(bug_ids, permissive)?;
    let mut projection = crate::validation::fields::projection_for(
        ctx.format(),
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::comment::COMMENT_FIELDS,
        w.err,
    )?;
    // The flat array is attributable only through each record's bug_id, and the
    // projection would otherwise strip it -- `--fields id,creator` is the
    // documented token-saving form. Retain it, and say so once.
    if bug_ids.len() > 1 && projection.would_drop("bug_id") {
        projection.retain_key("bug_id");
        let _ = writeln!(
            w.err,
            "keeping bug_id; it is what attributes each comment to its bug"
        );
    }
    let canonical_since = crate::validation::parse_optional_date(since, "--since")?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;

    // Table output is written per bug so each thread carries a header; JSON and
    // NDJSON accumulate so the payload stays one valid array.
    let table = matches!(ctx.format(), OutputFormat::Table);
    let mut collected: Vec<Comment> = Vec::new();
    let mut wrote_any = false;
    let mut skipped = 0_usize;
    for &bug_id in bug_ids {
        let fetched = match client
            .get_comments_since(bug_id, canonical_since.as_deref())
            .await
        {
            Ok(comments) => comments,
            Err(e) if permissive && e.is_permissive_bug_view_error() => {
                skipped += 1;
                let _ = writeln!(w.err, "bug {bug_id}: {e}");
                continue;
            }
            Err(e) => return Err(e),
        };
        if table {
            if bug_ids.len() > 1 {
                // Keyed on what has been written, not the loop index, so a bug
                // skipped at index 0 leaves no stray leading blank line.
                if wrote_any {
                    let _ = writeln!(w.out);
                }
                write_comment_bug_header(bug_id, w.out);
            }
            write_comments(&fetched, ctx.format(), &projection, w.out);
            wrote_any = true;
        } else {
            collected.extend(fetched);
        }
    }
    if table {
        if !wrote_any {
            // Every bug failed under --permissive. One empty-slice call keeps
            // the "No comments." contract a single ID has always had.
            write_comments(&[], ctx.format(), &projection, w.out);
        }
    } else {
        write_comments(&collected, ctx.format(), &projection, w.out);
    }
    if skipped > 0 {
        // One line a wrapper can grep, so "every bug failed" is distinguishable
        // from "nothing to show" -- the payload cannot carry that, and under
        // ndjson an all-failed run writes nothing to stdout at all.
        let total = bug_ids.len();
        let _ = writeln!(w.err, "{skipped} of {total} bugs could not be read");
    }
    Ok(())
}

// No cap on the ID count: `bug view` loops its list unbounded, and capping here
// would make two sibling read verbs disagree on maximum arity.
fn validate(bug_ids: &[u64], permissive: bool) -> Result<()> {
    if permissive && bug_ids.len() == 1 {
        return Err(BzrError::input_field(
            "--permissive only meaningful with multiple bug ids".into(),
            "permissive",
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
