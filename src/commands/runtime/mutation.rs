//! Shared driver for admin create/update commands.
//!
//! The `product`, `component`, `group`, and `user` create/update handlers all
//! repeat the same control-flow skeleton: read the output format, branch on
//! dry-run to emit a [`DryRunResult`], otherwise connect, call one resource API
//! method, and write an [`ActionResult`]. [`run`] owns that skeleton so each
//! handler supplies only its params, preview wording, and the single API call.
//!
//! Scope: this seam serves the linear, create-shaped / name-keyed flow only.
//! It hard-codes an empty `ids` slice for the dry-run preview, which is correct
//! for every caller (all are create-shaped or keyed by name, never by numeric
//! id). Id-keyed previews — currently only `component update`, which also needs
//! a pre-call id-resolution round-trip — are deliberately out of scope and stay
//! bespoke (see ADR-0003).

use std::future::Future;

use serde::Serialize;

use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;

/// Dry-run preview for a create/update mutation: the resource kind, the
/// would-be request `params` (serialized into the preview), and the
/// human-readable message.
pub(crate) struct DryRunPreview<P: Serialize> {
    pub(crate) resource: ResourceKind,
    pub(crate) params: P,
    pub(crate) message: String,
}

/// The committed result of a mutation that ran for real: the structured
/// [`ActionResult`] and its human-readable confirmation line.
pub(crate) struct Committed {
    pub(crate) result: ActionResult,
    pub(crate) message: String,
}

/// Drive an admin create/update command.
///
/// On a dry run, write `preview` and return without connecting. Otherwise
/// connect, hand the client and the owned params to `execute`, and write the
/// [`Committed`] result it returns. The two paths are mutually exclusive, so
/// `params` is borrowed for the preview or moved into `execute`, never both.
pub(crate) async fn run<P, F, Fut>(
    ctx: &CommandContext,
    w: &mut Writers<'_>,
    preview: DryRunPreview<P>,
    execute: F,
) -> Result<()>
where
    P: Serialize,
    F: FnOnce(BugzillaClient, P) -> Fut,
    Fut: Future<Output = Result<Committed>>,
{
    let format = ctx.format();
    if ctx.dry_run() {
        write_result(
            &DryRunResult::new(preview.resource, &[], &preview.params),
            &preview.message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let committed = execute(client, preview.params).await?;
    write_result(&committed.result, &committed.message, format, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "mutation_tests.rs"]
mod tests;
