use std::io::IsTerminal;

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::cli::{FieldArgs, ViewArgs};
use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::search::fields::{canonical_field_list, ColumnSpec};
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::bug::MultiBugRow;
use crate::output::resources::bug::{bug_to_json, write_bug_detail, write_multi_bug_view};
use crate::output::result_types::{write_result, BugViewFailure, MultiBugViewResult};
use crate::output::writers::Writers;
use crate::types::bug::Bug;
use crate::types::output::OutputFormat;

#[derive(Clone, Copy)]
enum BugViewMode {
    Strict,
    Permissive,
}

struct BugViewBatch {
    rows: Vec<BugViewRow>,
}

enum BugViewRow {
    Ok(Box<Bug>),
    Failed(BugViewFailure),
}

impl BugViewBatch {
    fn into_json_result(self) -> MultiBugViewResult {
        let mut bugs = Vec::with_capacity(self.rows.len());
        let mut failed = Vec::new();
        for row in self.rows {
            match row {
                BugViewRow::Ok(bug) => bugs.push(*bug),
                BugViewRow::Failed(failure) => failed.push(failure),
            }
        }
        MultiBugViewResult { bugs, failed }
    }

    fn into_table_rows(self) -> Vec<MultiBugRow> {
        self.rows
            .into_iter()
            .map(|row| match row {
                BugViewRow::Ok(bug) => MultiBugRow::Ok(bug),
                BugViewRow::Failed(failure) => MultiBugRow::Failed {
                    id: failure.id,
                    error: failure.error,
                },
            })
            .collect()
    }
}

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &ViewArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let ViewArgs {
        ids,
        permissive,
        web: _,
        field_args: FieldArgs {
            fields,
            exclude_fields,
        },
    } = args;

    if *permissive && ids.len() == 1 {
        return Err(BzrError::input(
            "--permissive only meaningful with multiple IDs".into(),
        ));
    }

    // Raw values drive column / detail-row selection (aliases resolve fine);
    // canonical values go to the server so aliased fields are populated.
    let inc = fields.as_deref();
    let exc = exclude_fields.as_deref();
    let inc_canonical = canonical_field_list(inc);
    let exc_canonical = canonical_field_list(exc);

    if ids.len() == 1 {
        let spec = ColumnSpec::new(inc_canonical.as_deref(), exc_canonical.as_deref());
        return view_single(client, &ids[0], spec, format, w).await;
    }

    let mode = if *permissive {
        BugViewMode::Permissive
    } else {
        BugViewMode::Strict
    };
    let batch = fetch_batch(
        client,
        ids,
        inc_canonical.as_deref(),
        exc_canonical.as_deref(),
        mode,
    )
    .await?;
    let spec = ColumnSpec::new(inc, exc);
    write_batch(batch, spec, format, w);
    Ok(())
}

/// Handle `bug view --web`: resolve each ID's web page URL from local config
/// and open it in the browser, or print it when running headless / non-TTY.
pub(super) fn handle_web(ids: &[String], ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let urls = resolve_bug_urls(ids, ctx)?;
    // Open a browser only with both a terminal stdout and a display; anything
    // else (pipe, redirect, headless) prints the URL instead.
    let interactive = std::io::stdout().is_terminal() && has_display();
    emit_web(&urls, interactive, |url| open::that(url), w);
    Ok(())
}

/// Build the `show_bug.cgi?id=<ID>` URL for each ID against the active server's
/// base URL. Pure local config read — no network, no auth. Uses the
/// unvalidated config so opening a bug page never depends on credentials being
/// set (here or on any other server entry); only the server's URL is needed.
fn resolve_bug_urls(ids: &[String], ctx: &CommandContext) -> Result<Vec<String>> {
    let config = Config::read_unvalidated_at(ctx.config_path_override())?;
    let (_name, srv) = config.resolve_server(ctx.server())?;
    Ok(ids.iter().map(|id| bug_web_url(&srv.url, id)).collect())
}

/// Construct a bug's web page URL. Trailing slashes on `base` are trimmed so
/// the result has exactly one separator; the ID is percent-encoded so an alias
/// with reserved characters stays a valid query value.
fn bug_web_url(base: &str, id: &str) -> String {
    let base = base.trim_end_matches('/');
    let encoded = utf8_percent_encode(id, NON_ALPHANUMERIC);
    format!("{base}/show_bug.cgi?id={encoded}")
}

/// Whether a graphical display is available to open a browser. GUI platforms
/// (macOS, Windows) always qualify.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn has_display() -> bool {
    true
}

/// Whether a graphical display is available to open a browser. On non-GUI
/// platforms this requires `DISPLAY` or `WAYLAND_DISPLAY` to be set.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn has_display() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Open each URL with `open_fn` when interactive, else print it. A failed open
/// falls back to printing (with a stderr warning) so the URL is never lost.
/// `open_fn` is injected so the open path is testable without a real browser.
fn emit_web<F>(urls: &[String], interactive: bool, open_fn: F, w: &mut Writers<'_>)
where
    F: Fn(&str) -> std::io::Result<()>,
{
    for url in urls {
        if interactive {
            if let Err(e) = open_fn(url) {
                let _ = writeln!(w.err, "warning: failed to open browser: {e}");
                let _ = writeln!(w.out, "{url}");
            }
        } else {
            let _ = writeln!(w.out, "{url}");
        }
    }
}

async fn view_single(
    client: &BugzillaClient,
    id: &str,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let bug = client.get_bug(id, spec.include, spec.exclude).await?;
    write_bug_detail(&bug, spec, format, w.out);
    Ok(())
}

async fn fetch_batch(
    client: &BugzillaClient,
    ids: &[String],
    include_fields: Option<&str>,
    exclude_fields: Option<&str>,
    mode: BugViewMode,
) -> Result<BugViewBatch> {
    let mut rows = Vec::with_capacity(ids.len());
    for id in ids {
        match client.get_bug(id, include_fields, exclude_fields).await {
            Ok(bug) => rows.push(BugViewRow::Ok(Box::new(bug))),
            Err(e)
                if matches!(mode, BugViewMode::Permissive) && e.is_permissive_bug_view_error() =>
            {
                rows.push(BugViewRow::Failed(BugViewFailure {
                    id: id.clone(),
                    error: e.to_string(),
                }));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(BugViewBatch { rows })
}

fn write_batch(
    batch: BugViewBatch,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    match format {
        OutputFormat::Table => {
            let rows = batch.into_table_rows();
            write_multi_bug_view(&rows, spec, w.out);
        }
        OutputFormat::Json | OutputFormat::Ndjson => {
            // Project each bug to the selected fields; the wrapper keys and the
            // per-failure metadata (`id`, `error`) stay untrimmed so `jq`
            // consumers can always rely on `.bugs[]` / `.failed[]`.
            let result = batch.into_json_result();
            let bugs: Vec<serde_json::Value> =
                result.bugs.iter().map(|b| bug_to_json(b, spec)).collect();
            let value = serde_json::json!({ "bugs": bugs, "failed": result.failed });
            write_result(&value, "", format, w.out);
        }
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
