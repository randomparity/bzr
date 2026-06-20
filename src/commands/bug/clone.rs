use crate::cli::CloneArgs;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::{CreateBugParams, OutputFormat};

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &CloneArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let CloneArgs {
        id,
        summary,
        product,
        component,
        version,
        description,
        priority,
        severity,
        assignee,
        op_sys,
        rep_platform,
        no_comment,
        add_depends_on,
        add_blocks,
        no_cc,
        no_keywords,
    } = args;

    // Fetch source bug with all fields needed for cloning
    let source = client.get_bug(id, None, None).await?;

    // Get description from comment #0
    let clone_description = if description.is_some() {
        description.clone()
    } else {
        let comments = client.get_comments_since(source.id, None).await?;
        comments.into_iter().find(|c| c.count == 0).map(|c| c.text)
    };

    let source_product = source.product.ok_or_else(|| {
        crate::error::BzrError::DataIntegrity("source bug missing product field".into())
    })?;
    let source_component = source.component.ok_or_else(|| {
        crate::error::BzrError::DataIntegrity("source bug missing component field".into())
    })?;

    let mut blocks = Vec::new();
    if *add_blocks {
        blocks.push(source.id);
    }
    let mut depends_on = Vec::new();
    if *add_depends_on {
        depends_on.push(source.id);
    }

    let params = CreateBugParams {
        product: product.clone().unwrap_or(source_product),
        component: component.clone().unwrap_or(source_component),
        summary: summary.clone().unwrap_or(source.summary),
        version: version
            .clone()
            .or(source.version)
            .unwrap_or_else(|| "unspecified".to_string()),
        description: clone_description,
        priority: priority.clone().or(source.priority),
        severity: severity.clone().or(source.severity),
        assigned_to: assignee.clone().or(source.assigned_to),
        op_sys: op_sys.clone().or(source.op_sys),
        rep_platform: rep_platform.clone().or(source.rep_platform),
        blocks,
        depends_on,
        cc: if *no_cc { vec![] } else { source.cc },
        keywords: if *no_keywords {
            vec![]
        } else {
            source.keywords
        },
        ..Default::default()
    };

    if crate::commands::runtime::dry_run::enabled() {
        write_clone_dry_run(source.id, &params, format, w);
        return Ok(());
    }

    let new_id = client.create_bug(&params).await?;

    // The bug was created successfully — that is the clone operation. The
    // "Cloned from" back-reference comment is supplementary, so a failure
    // posting it must not hide the new bug ID (otherwise the user can't tell
    // the clone succeeded and may re-clone, creating a duplicate). Warn and
    // continue rather than propagating.
    if !*no_comment {
        if let Err(e) = client
            .add_comment(new_id, &format!("Cloned from bug #{}", source.id), false)
            .await
        {
            let _ = writeln!(
                w.err,
                "warning: created bug #{new_id} but failed to add the \
                 \"Cloned from bug #{}\" comment: {e}",
                source.id
            );
        }
    }

    write_result(
        &ActionResult::created(new_id, ResourceKind::Bug),
        &format!("Cloned bug #{} → #{new_id}", source.id),
        format,
        w.out,
    );
    Ok(())
}

/// Emit the would-be clone payload without writing, marked `"action":"dry-run"`.
/// The clone creates a new bug, so `ids` is empty; `changes` carries the
/// resolved `CreateBugParams` (built from the fetched source bug).
fn write_clone_dry_run(
    source_id: u64,
    params: &CreateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    write_result(
        &DryRunResult::new(ResourceKind::Bug, &[], params),
        &format!("Dry run: would clone bug #{source_id} (no bug created)"),
        format,
        w.out,
    );
}

#[cfg(test)]
#[path = "clone_tests.rs"]
mod tests;
