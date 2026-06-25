use crate::cli::CloneArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::flags::parse_flags;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::bug::CreateBugParams;
use crate::types::comment::AddCommentParams;
use crate::types::output::OutputFormat;
use crate::validation::parse_optional_date_only;

pub(super) async fn handle(
    args: &CloneArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
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
        create_fields,
        no_comment,
        add_depends_on,
        add_blocks,
        no_cc,
        no_keywords,
    } = args;

    let flags = parse_flags(&create_fields.flag)?;
    let deadline = parse_optional_date_only(create_fields.deadline.as_deref(), "--deadline")?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;

    // Fetch source bug with all fields needed for cloning
    let source = client.get_bug(id, None, None).await?;

    let clone_description =
        resolve_source_description(&client, source.id, description.as_deref()).await?;
    let source_product = required_source_field(source.product, "product")?;
    let source_component = required_source_field(source.component, "component")?;

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
        url: create_fields.url.clone().or(source.url),
        whiteboard: create_fields.whiteboard.clone().or(source.whiteboard),
        target_milestone: create_fields
            .target_milestone
            .clone()
            .or(source.target_milestone),
        deadline: deadline.or(source.deadline),
        blocks,
        depends_on,
        cc: clone_list(source.cc, &create_fields.cc, *no_cc),
        keywords: clone_list(source.keywords, &create_fields.keywords, *no_keywords),
        groups: create_fields.groups.clone(),
        flags,
        ..Default::default()
    };

    if ctx.dry_run() {
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
        let params = AddCommentParams {
            text: format!("Cloned from bug #{}", source.id),
            is_private: false,
        };
        if let Err(e) = client.add_comment(new_id, &params).await {
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

async fn resolve_source_description(
    client: &BugzillaClient,
    source_id: u64,
    explicit: Option<&str>,
) -> Result<Option<String>> {
    if let Some(description) = explicit {
        return Ok(Some(description.to_string()));
    }
    let comments = client.get_comments_since(source_id, None).await?;
    Ok(comments
        .into_iter()
        .find(|comment| comment.count == Some(0))
        .and_then(|comment| comment.text))
}

fn required_source_field(value: Option<String>, field: &str) -> Result<String> {
    value.ok_or_else(|| {
        crate::error::BzrError::DataIntegrity(format!("source bug missing {field} field"))
    })
}

fn clone_list(
    source_values: Vec<String>,
    override_values: &[String],
    omit_source: bool,
) -> Vec<String> {
    if omit_source {
        Vec::new()
    } else if override_values.is_empty() {
        source_values
    } else {
        override_values.to_vec()
    }
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
