use std::path::Path;

use serde::Deserialize;

use crate::cli::CreateArgs;
use crate::commands::bug::compound::CompoundPlan;
use crate::commands::runtime::input::attachment_input::{
    prepare_attachment_params, AttachmentInput,
};
use crate::commands::runtime::input::from_json::JsonOneOrMany;
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::error::Result;
use crate::output::result_types::{
    write_result, BatchCreateResult, CreateFailure, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::bug::CreateBugParams;
use crate::types::comment::AddCommentParams;
use crate::types::output::OutputFormat;

/// One bug's worth of structured input for `bug create --from-json`. Keys match
/// the create flag names; `deny_unknown_fields` rejects typos and keeps this
/// document shape strict. Arbitrary and custom (`cf_*`) fields are set through
/// `--field` / `--field-json` instead, which validates every key against the
/// server's own catalogue (ADR 0053, issues #283 and #671); those flags overlay
/// onto this path through `extra_fields`, which serde skips so the document
/// itself cannot carry the key. All fields are optional here — required-field
/// and date validation happen in [`Self::into_params`] so the error messages
/// can name the offending field.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateBug {
    product: Option<String>,
    component: Option<String>,
    summary: Option<String>,
    version: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    severity: Option<String>,
    assignee: Option<String>,
    op_sys: Option<String>,
    platform: Option<String>,
    alias: Option<String>,
    url: Option<String>,
    whiteboard: Option<String>,
    target_milestone: Option<String>,
    deadline: Option<String>,
    #[serde(default)]
    blocks: Vec<u64>,
    #[serde(default)]
    depends_on: Vec<u64>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    groups: JsonGroups,
    #[serde(default)]
    flags: Vec<String>,
    /// Tags applied to the description comment (comment 0), forwarded as
    /// `comment_tags` on `Bug.create`. Distinct from `comment` below, which
    /// posts a separate compound-create comment after the bug exists.
    #[serde(default)]
    comment_tags: Vec<String>,
    /// First comment to post after the bug is created (compound create).
    #[serde(default)]
    comment: Option<JsonComment>,
    /// Attachments to upload after the bug is created (compound create).
    #[serde(default)]
    attachments: Vec<JsonAttachment>,
    /// Carried in from the CLI `--field` / `--field-json` overlay, never from
    /// the document — `serde(skip)` keeps it out of the deserialized field
    /// list, so `deny_unknown_fields` still rejects an `extra_fields` key.
    #[serde(skip)]
    extra_fields: crate::types::bug::ExtraBugFields,
}

#[derive(Debug, Default)]
struct JsonGroups(Option<Vec<String>>);

impl<'de> Deserialize<'de> for JsonGroups {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer).map(|groups| Self(Some(groups)))
    }
}

impl JsonGroups {
    fn into_option(self) -> Option<Vec<String>> {
        self.0
    }

    fn overlay_cli(&mut self, groups: &[String]) {
        if !groups.is_empty() {
            self.0 = Some(groups.to_vec());
        }
    }
}

/// A first comment in the compound `--from-json` payload.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonComment {
    body: String,
    #[serde(default)]
    is_private: bool,
}

/// One attachment in the compound `--from-json` payload.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonAttachment {
    file: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    is_patch: bool,
    #[serde(default)]
    is_private: bool,
}

impl JsonCreateBug {
    /// Take the `comment`/`attachments` out of the entry and build the compound
    /// [`CompoundPlan`], reading each attachment file and rejecting an empty
    /// comment body — all **before** the bug is created. Leaves the scalar
    /// create fields intact for [`Self::into_params`].
    fn take_plan(&mut self) -> Result<CompoundPlan> {
        let comment = match self.comment.take() {
            Some(c) => {
                if c.body.trim().is_empty() {
                    return Err(crate::error::BzrError::input(
                        "--from-json: 'comment.body' must not be empty".into(),
                    ));
                }
                Some(AddCommentParams {
                    text: c.body,
                    is_private: c.is_private,
                })
            }
            None => None,
        };
        let mut attachments = Vec::with_capacity(self.attachments.len());
        for a in std::mem::take(&mut self.attachments) {
            let (params, _size) = prepare_attachment_params(AttachmentInput {
                file: Path::new(&a.file),
                summary: a.description.as_deref(),
                content_type: a.content_type.as_deref(),
                is_patch: a.is_patch,
                is_private: a.is_private,
                comment: None,
                flags: vec![],
            })?;
            attachments.push(params);
        }
        let comment_tags = super::create::resolve_comment_tags(
            &std::mem::take(&mut self.comment_tags),
            self.description.is_some(),
            "--comment-tag requires a description (set it in the JSON or via --description)",
        )?;
        Ok(CompoundPlan {
            comment,
            attachments,
            comment_tags,
        })
    }

    /// Validate the merged fields and build the API params. `product`,
    /// `component`, and `summary` are required; `version` defaults to
    /// `"unspecified"`; `flags` and `deadline` are parsed/validated.
    fn into_params(self) -> Result<CreateBugParams> {
        let required = |value: Option<String>, field: &str| {
            value.ok_or_else(|| {
                crate::error::BzrError::input_field(
                    format!(
                        "--from-json: '{field}' is required (set it in the JSON or via --{field})"
                    ),
                    field,
                    None,
                )
            })
        };
        let flags = crate::commands::runtime::input::flags::parse_flags(&self.flags)?;
        let deadline =
            crate::validation::parse_optional_date_only(self.deadline.as_deref(), "deadline")?;
        let groups = self.groups.into_option();
        let mut params = CreateBugParams {
            product: required(self.product, "product")?,
            component: required(self.component, "component")?,
            summary: required(self.summary, "summary")?,
            version: self.version.unwrap_or_else(|| "unspecified".to_string()),
            description: self.description,
            priority: self.priority,
            severity: self.severity,
            assigned_to: self.assignee,
            op_sys: self.op_sys,
            platform: self.platform,
            alias: self.alias,
            url: self.url,
            whiteboard: self.whiteboard,
            target_milestone: self.target_milestone,
            deadline,
            blocks: self.blocks,
            depends_on: self.depends_on,
            cc: self.cc,
            keywords: self.keywords,
            groups: Vec::new(),
            groups_present: false,
            flags,
            extra_fields: crate::types::bug::ExtraBugFields::new(),
        };
        if let Some(groups) = groups {
            params.set_groups_from_structured_input(groups);
        }
        // Assigned after the typed fields are in place so the collision check
        // reads the payload as it will actually be sent.
        params.extra_fields = crate::commands::runtime::input::extra_fields::check_against(
            &params,
            self.extra_fields,
        )?;
        Ok(params)
    }
}

/// Resolve an explicit `--description`/`--description-file` for the JSON path.
/// Unlike the interactive create flow, this does NOT auto-read stdin — only an
/// explicitly-supplied value overrides the JSON `description`.
fn explicit_description(
    description: Option<&str>,
    description_file: Option<&std::path::Path>,
) -> Result<Option<String>> {
    crate::commands::runtime::shared::materialize_body_source(
        crate::commands::runtime::shared::classify_body_source(
            description,
            description_file,
            "--description",
            "--description-file",
        )?,
        "--description-file",
    )
}

/// Overlay explicit CLI flags onto a JSON entry: a CLI value (a `Some` scalar
/// or a non-empty repeatable) wins over the JSON field, applied uniformly to
/// every element of an array. `extra` is parsed once by the caller — its
/// `--field-json -` source may be stdin, which reads only once.
fn overlay_cli(
    mut json: JsonCreateBug,
    args: &CreateArgs,
    extra: &crate::types::bug::ExtraBugFields,
) -> Result<JsonCreateBug> {
    let CreateArgs {
        product,
        component,
        summary,
        version,
        description,
        description_file,
        priority,
        severity,
        assignee,
        op_sys,
        platform,
        blocks,
        depends_on,
        create_fields,
        comment_tag,
        ..
    } = args;
    merge_set(&mut json.product, product.as_deref());
    merge_set(&mut json.component, component.as_deref());
    merge_set(&mut json.summary, summary.as_deref());
    merge_set(&mut json.version, version.as_deref());
    merge_set(&mut json.priority, priority.as_deref());
    merge_set(&mut json.severity, severity.as_deref());
    merge_set(&mut json.assignee, assignee.as_deref());
    merge_set(&mut json.op_sys, op_sys.as_deref());
    merge_set(&mut json.platform, platform.as_deref());
    merge_set(&mut json.alias, create_fields.alias.as_deref());
    merge_set(&mut json.url, create_fields.url.as_deref());
    merge_set(&mut json.whiteboard, create_fields.whiteboard.as_deref());
    merge_set(
        &mut json.target_milestone,
        create_fields.target_milestone.as_deref(),
    );
    merge_set(&mut json.deadline, create_fields.deadline.as_deref());
    if let Some(desc) = explicit_description(description.as_deref(), description_file.as_deref())? {
        json.description = Some(desc);
    }
    merge_vec(&mut json.cc, &create_fields.cc);
    merge_vec(&mut json.keywords, &create_fields.keywords);
    json.groups.overlay_cli(&create_fields.groups);
    merge_vec(&mut json.flags, &create_fields.flag);
    merge_vec(&mut json.comment_tags, comment_tag);
    json.extra_fields.clone_from(extra);
    // `blocks`/`depends_on` are `Vec<u64>`; `merge_vec` is `Vec<String>`-typed,
    // so keep the equivalent guard inline.
    if !blocks.is_empty() {
        json.blocks.clone_from(blocks);
    }
    if !depends_on.is_empty() {
        json.depends_on.clone_from(depends_on);
    }
    Ok(json)
}

/// Emit a batch-create result: a partial-failure object under `--json`, or a
/// created-IDs line plus per-item failures on stderr in table mode.
fn write_batch_create(result: &BatchCreateResult, format: OutputFormat, w: &mut Writers<'_>) {
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => write_result(result, "", format, w.out),
        OutputFormat::Table => {
            if !result.created.is_empty() {
                let ids: Vec<String> = result.created.iter().map(|id| format!("#{id}")).collect();
                let _ = writeln!(w.out, "Created bugs: {}", ids.join(", "));
            }
            for f in &result.failed {
                let _ = writeln!(
                    w.err,
                    "Failed to create bug (item {}): {}",
                    f.index, f.error
                );
            }
        }
    }
}

/// Create a batch of bugs (top-level JSON array). The array shape always yields
/// the partial-failure result — even for a single element — so an agent's
/// output handling does not depend on the element count. Each element's compound
/// sub-steps (comment/attachments) run after its bug is created; a created bug
/// whose sub-step fails appears in both `created` and `failed`. Exits 11 if any
/// element had any failure (create or sub-step).
async fn create_batch_from_json(
    prepared: Vec<(CreateBugParams, CompoundPlan)>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let total = prepared.len();
    if ctx.dry_run() {
        // One coherent object for the whole batch; `changes` carries the array
        // of per-element compound payloads (bug fields + comment + attachments).
        let changes: Vec<_> = prepared
            .iter()
            .map(|(params, plan)| super::compound::dry_run_changes(params, plan))
            .collect();
        write_result(
            &DryRunResult::new(ResourceKind::Bug, &[], &changes),
            &format!("Dry run: would create {total} bug(s) (no bugs created)"),
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_validate_bug_fields(
        ctx,
        &crate::commands::runtime::input::extra_fields::key_union(
            prepared.iter().map(|(params, _)| &params.extra_fields),
        ),
    )
    .await?;
    let mut created = Vec::new();
    let mut failed = Vec::new();
    for (index, (params, plan)) in prepared.into_iter().enumerate() {
        match client.create_bug(&params).await {
            Ok(id) => {
                created.push(id);
                for sub in super::compound::run_sub_steps(&client, id, plan, w).await {
                    failed.push(CreateFailure::sub_step(
                        index, id, sub.step, sub.file, sub.error,
                    ));
                }
            }
            Err(e) => failed.push(CreateFailure::create(index, e.to_string())),
        }
        crate::output::progress::batch_event(
            ctx.progress(),
            w.err,
            &crate::output::progress::BatchProgress {
                n: index + 1,
                total,
                ok: created.len(),
                failed: failed.len(),
            },
        );
    }
    let succeeded = created.len();
    let failures = failed.len();
    write_batch_create(&BatchCreateResult::new(created, failed), format, w);
    let result = crate::commands::runtime::mutation::ensure_batch_complete(succeeded, failures);
    if result.is_ok() {
        crate::output::progress::done_event(ctx.progress(), w.err, total);
    }
    result
}

/// Build one bug from a structured JSON object or array, the `--from-json`
/// path. All entries are validated before any write (including reading
/// attachment files and rejecting empty comment bodies), so malformed input
/// never half-creates a batch; per-element server failures use the
/// partial-failure model (exit 11).
pub(super) async fn handle(
    args: &CreateArgs,
    arg: &str,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let extra = crate::commands::runtime::input::extra_fields::parse(
        &args.create_fields.field,
        args.create_fields.field_json.as_deref(),
    )?;
    match crate::commands::runtime::input::from_json::read_one_or_many::<JsonCreateBug>(arg)? {
        JsonOneOrMany::One(entry) => {
            let mut merged = overlay_cli(*entry, args, &extra)?;
            let plan = merged.take_plan()?;
            let params = merged.into_params()?;
            if plan.is_empty() {
                super::create::create_and_report(&params, ctx, w).await
            } else {
                super::compound::create_with_sub_steps(&params, plan, ctx, w).await
            }
        }
        JsonOneOrMany::Many(entries) => {
            if entries.is_empty() {
                return Err(crate::error::BzrError::input(
                    "--from-json: empty array, nothing to create".into(),
                ));
            }
            // Phase 1: validate and build every element's params + plan before
            // any write, so a bad element (missing attachment, empty comment)
            // aborts (exit 7) with zero bugs created.
            let mut prepared = Vec::with_capacity(entries.len());
            for entry in entries {
                let mut merged = overlay_cli(entry, args, &extra)?;
                let plan = merged.take_plan()?;
                let params = merged.into_params()?;
                prepared.push((params, plan));
            }
            // Phase 2: create + run sub-steps.
            create_batch_from_json(prepared, ctx, w).await
        }
    }
}

#[cfg(test)]
#[path = "create_json_tests.rs"]
mod tests;
