//! Bug subcommand handlers, split per-action.

use crate::cli::BugAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::bug::{
    validate_json_field_selection, validate_table_columns, warn_unknown_fields, ColumnSpec,
};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

mod clone;
mod create;
mod create_json;
mod history;
mod list;
mod my;
mod search;
mod update;
mod update_json;
mod verbs;
mod view;

/// The `--fields` / `--exclude-fields` column spec for the four field-bearing
/// bug actions, or `None` for actions that take no field selection.
fn bug_column_spec(action: &BugAction) -> Option<ColumnSpec<'_>> {
    let field_args = match action {
        BugAction::List(a) => &a.field_args,
        BugAction::My(a) => &a.field_args,
        BugAction::Search(a) => &a.field_args,
        BugAction::View(a) => &a.field_args,
        _ => return None,
    };
    Some(ColumnSpec::new(
        field_args.fields.as_deref(),
        field_args.exclude_fields.as_deref(),
    ))
}

/// Rewrite search params for `--count`: fetch only IDs (smallest payload) and
/// lift the row limit (`0` = all matches, bounded by the server's max-results)
/// so the count reflects the full match set, not a page of it. Clear offsets
/// that may have come from saved Bugzilla URLs for the same reason.
pub(super) fn count_search_params(
    mut params: crate::types::SearchParams,
) -> crate::types::SearchParams {
    params.include_fields = Some("id".to_string());
    params.exclude_fields = None;
    params.limit = Some(0);
    params.offset = None;
    params.raw_params.retain(|(key, _)| key != "offset");
    params
}

/// Reject `--offset`/`--paginate` combined with `--count`. `--count` reports
/// the full match total, so windowing or page-looping it is contradictory;
/// fail fast (exit 7) rather than silently ignore the paging flags.
pub(super) fn ensure_no_paging_with_count(
    count: bool,
    offset: Option<u32>,
    paginate: bool,
) -> Result<()> {
    if count && (offset.is_some() || paginate) {
        return Err(crate::error::BzrError::InputValidation(
            "--count cannot be combined with --offset or --paginate".into(),
        ));
    }
    Ok(())
}

/// Whether a bug action is a mutation that supports `--dry-run` (the create-,
/// update-, and clone-shaped writes). Read actions and `--web` are excluded.
#[must_use]
pub fn is_dry_runnable(action: &BugAction) -> bool {
    matches!(
        action,
        BugAction::Create(_)
            | BugAction::Update(_)
            | BugAction::Clone(_)
            | BugAction::Resolve(_)
            | BugAction::Close(_)
            | BugAction::Reopen(_)
            | BugAction::Dup(_)
    )
}

pub(crate) fn requires_credentials(action: &BugAction) -> Option<&'static str> {
    match action {
        BugAction::List(_) | BugAction::View(_) | BugAction::Search(_) | BugAction::History(_) => {
            None
        }
        BugAction::Create(_) => Some("bug create"),
        BugAction::My(_) => Some("bug my"),
        BugAction::Clone(_) => Some("bug clone"),
        BugAction::Update(_) => Some("bug update"),
        BugAction::Resolve(_) => Some("bug resolve"),
        BugAction::Close(_) => Some("bug close"),
        BugAction::Reopen(_) => Some("bug reopen"),
        BugAction::Dup(_) => Some("bug dup"),
    }
}

/// Dispatch bug actions to their respective handlers.
pub async fn execute(action: &BugAction, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let format = ctx.format();
    update::validate_action(action)?;

    // `--web` resolves the bug's URL from local config and opens (or prints)
    // it — no auth, no network, no field selection. Short-circuit before any
    // of the connect/validate machinery below.
    if let BugAction::View(args) = action {
        if args.web {
            return view::handle_web(&args.ids, ctx.server(), w);
        }
    }

    // Resolve the field selection before any network I/O. A selection that
    // leaves nothing to show exits 7 deterministically — measured against the
    // five-column default in table mode, against the full field universe in
    // JSON mode (where output is trimmed gh-style to the selected keys). Either
    // way the exit code is independent of subcommand or server reachability.
    // `bug view` is exempt from this zero-field error in both modes: a sparse
    // (or empty `{}`) single-bug result is coherent, not an error. Custom
    // `cf_*` fields are recognized dynamic fields. Other unknown `--fields`
    // tokens warn once on stderr for JSON output and for `bug view` table
    // output, since those lenient paths would otherwise hide a typo.
    if let Some(spec) = bug_column_spec(action) {
        let is_view = matches!(action, BugAction::View(_));
        match format {
            OutputFormat::Table => {
                if is_view {
                    warn_unknown_fields(spec, w.err);
                } else {
                    validate_table_columns(spec)?;
                }
            }
            OutputFormat::Json | OutputFormat::Ndjson => {
                if !is_view {
                    validate_json_field_selection(spec)?;
                }
                warn_unknown_fields(spec, w.err);
            }
        }
    }

    // Search builds its own client because --from-url may resolve a different
    // server from the URL hostname. Skip the shared connect to avoid double
    // auth/version detection on every `bug search` invocation.
    if let BugAction::Search(args) = action {
        return search::handle(args, ctx, w).await;
    }

    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;

    match action {
        BugAction::List(args) => list::handle(&client, args, format, w).await,
        BugAction::View(args) => view::handle(&client, args, format, w).await,
        BugAction::History(args) => history::handle(&client, args, format, w).await,
        BugAction::My(args) => my::handle(&client, args, format, w).await,
        BugAction::Create(args) => create::handle(&client, args, ctx, w).await,
        BugAction::Clone(args) => clone::handle(&client, args, ctx, w).await,
        BugAction::Update(args) => update::handle(&client, args, ctx, w).await,
        BugAction::Resolve(a) => verbs::resolve(&client, a, ctx, w).await,
        BugAction::Close(a) => verbs::close(&client, a, ctx, w).await,
        BugAction::Reopen(a) => verbs::reopen(&client, a, ctx, w).await,
        BugAction::Dup(a) => verbs::dup(&client, a, ctx, w).await,
        BugAction::Search(_) => unreachable!("handled above"),
    }
}
