use serde::Deserialize;

use crate::cli::CreateArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::from_json::JsonOneOrMany;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::error::Result;
use crate::output::result_types::{
    write_result, BatchCreateResult, CreateFailure, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::{CreateBugParams, OutputFormat};

/// One bug's worth of structured input for `bug create --from-json`. Keys match
/// the create flag names; `deny_unknown_fields` rejects typos and keeps
/// undesigned `cf_*` custom-field writes (issue #283) out of this path. All
/// fields are optional here — required-field and date validation happen in
/// [`Self::into_params`] so the error messages can name the offending field.
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
    rep_platform: Option<String>,
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
    groups: Vec<String>,
    #[serde(default)]
    flags: Vec<String>,
}

impl JsonCreateBug {
    /// Validate the merged fields and build the API params. `product`,
    /// `component`, and `summary` are required; `version` defaults to
    /// `"unspecified"`; `flags` and `deadline` are parsed/validated.
    fn into_params(self) -> Result<CreateBugParams> {
        let required = |value: Option<String>, field: &str| {
            value.ok_or_else(|| {
                crate::error::BzrError::InputValidation(format!(
                    "--from-json: '{field}' is required (set it in the JSON or via --{field})"
                ))
            })
        };
        let flags = crate::commands::runtime::flags::parse_flags(&self.flags)?;
        let deadline =
            crate::validation::parse_optional_date_only(self.deadline.as_deref(), "deadline")?;
        Ok(CreateBugParams {
            product: required(self.product, "product")?,
            component: required(self.component, "component")?,
            summary: required(self.summary, "summary")?,
            version: self.version.unwrap_or_else(|| "unspecified".to_string()),
            description: self.description,
            priority: self.priority,
            severity: self.severity,
            assigned_to: self.assignee,
            op_sys: self.op_sys,
            rep_platform: self.rep_platform,
            alias: self.alias,
            url: self.url,
            whiteboard: self.whiteboard,
            target_milestone: self.target_milestone,
            deadline,
            blocks: self.blocks,
            depends_on: self.depends_on,
            cc: self.cc,
            keywords: self.keywords,
            groups: self.groups,
            flags,
        })
    }
}

/// Structured `--from-json` input, preserving the top-level shape so the
/// output shape follows the *input* (a 1-element array is still a batch), not
/// the element count.
#[derive(Debug)]
enum JsonInput {
    /// A top-level object: one bug, single-result output. Boxed because a
    /// `JsonCreateBug` is far larger than the `Many` vec handle.
    One(Box<JsonCreateBug>),
    /// A top-level array: one bug per element, partial-failure output.
    Many(Vec<JsonCreateBug>),
}

fn read_json_bugs(arg: &str) -> Result<JsonInput> {
    match crate::commands::runtime::from_json::read_one_or_many(arg)? {
        JsonOneOrMany::One(entry) => Ok(JsonInput::One(entry)),
        JsonOneOrMany::Many(entries) => Ok(JsonInput::Many(entries)),
    }
}

#[cfg(test)]
fn parse_json_bugs(raw: &str) -> Result<JsonInput> {
    match crate::commands::runtime::from_json::parse_one_or_many(raw)? {
        JsonOneOrMany::One(entry) => Ok(JsonInput::One(entry)),
        JsonOneOrMany::Many(entries) => Ok(JsonInput::Many(entries)),
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
/// every element of an array.
fn overlay_cli(mut json: JsonCreateBug, args: &CreateArgs) -> Result<JsonCreateBug> {
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
        rep_platform,
        blocks,
        depends_on,
        create_fields,
        ..
    } = args;
    // `merge_set`/`merge_vec` overwrite the target when the CLI flag was
    // supplied (a `Some` scalar / non-empty repeatable), else leave the JSON
    // value — exactly the "CLI wins" precedence.
    merge_set(&mut json.product, product.as_deref());
    merge_set(&mut json.component, component.as_deref());
    merge_set(&mut json.summary, summary.as_deref());
    merge_set(&mut json.version, version.as_deref());
    merge_set(&mut json.priority, priority.as_deref());
    merge_set(&mut json.severity, severity.as_deref());
    merge_set(&mut json.assignee, assignee.as_deref());
    merge_set(&mut json.op_sys, op_sys.as_deref());
    merge_set(&mut json.rep_platform, rep_platform.as_deref());
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
    merge_vec(&mut json.groups, &create_fields.groups);
    merge_vec(&mut json.flags, &create_fields.flag);
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
/// output handling does not depend on the element count. Exits 11 if any
/// element fails.
async fn create_batch_from_json(
    client: &BugzillaClient,
    params_list: &[CreateBugParams],
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if crate::commands::runtime::dry_run::enabled() {
        // One coherent object for the whole batch (N pretty-printed objects
        // would not be valid JSON); `changes` carries the array of params.
        write_result(
            &DryRunResult::new(ResourceKind::Bug, &[], &params_list),
            &format!(
                "Dry run: would create {} bug(s) (no bugs created)",
                params_list.len()
            ),
            format,
            w.out,
        );
        return Ok(());
    }
    let mut created = Vec::new();
    let mut failed = Vec::new();
    for (index, params) in params_list.iter().enumerate() {
        match client.create_bug(params).await {
            Ok(id) => created.push(id),
            Err(e) => failed.push(CreateFailure {
                index,
                error: e.to_string(),
            }),
        }
    }
    let succeeded = created.len();
    let failures = failed.len();
    write_batch_create(&BatchCreateResult::new(created, failed), format, w);
    super::update::ensure_batch_complete(succeeded, failures)
}

/// Build one bug from a structured JSON object or array, the `--from-json`
/// path. All entries are validated before any write, so malformed input never
/// half-creates a batch; per-element server failures use the partial-failure
/// model (exit 11).
pub(super) async fn handle(
    client: &BugzillaClient,
    args: &CreateArgs,
    arg: &str,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    match read_json_bugs(arg)? {
        JsonInput::One(entry) => {
            let params = overlay_cli(*entry, args)?.into_params()?;
            super::create::create_and_report(client, &params, format, w).await
        }
        JsonInput::Many(entries) => {
            if entries.is_empty() {
                return Err(crate::error::BzrError::InputValidation(
                    "--from-json: empty array, nothing to create".into(),
                ));
            }
            let mut params_list = Vec::with_capacity(entries.len());
            for entry in entries {
                params_list.push(overlay_cli(entry, args)?.into_params()?);
            }
            create_batch_from_json(client, &params_list, format, w).await
        }
    }
}

#[cfg(test)]
#[path = "create_json_tests.rs"]
mod tests;
