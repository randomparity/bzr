#![expect(clippy::unwrap_used)]

use super::{write_skills_install, SkillsDestinationResult, SkillsInstallResult};
use crate::cli::AgentTarget;
use crate::types::OutputFormat;

fn install_result(project: Option<&str>) -> SkillsInstallResult {
    SkillsInstallResult {
        action: "install".into(),
        agent: AgentTarget::All,
        scope: if project.is_some() {
            "project".into()
        } else {
            "global".into()
        },
        project: project.map(str::to_string),
        destinations: vec![
            SkillsDestinationResult {
                layout: "agents".into(),
                path: "/canonical/project/.agents/skills".into(),
                installed: vec!["bzr-bulk-triage".into(), "bzr-file-bug".into()],
            },
            SkillsDestinationResult {
                layout: "claude".into(),
                path: "/canonical/project/.claude/skills".into(),
                installed: vec!["bzr-bulk-triage".into(), "bzr-file-bug".into()],
            },
        ],
    }
}

fn render(result: &SkillsInstallResult, format: OutputFormat) -> String {
    let mut out = Vec::new();
    write_skills_install(result, format, None, &mut out);
    String::from_utf8(out).unwrap()
}

#[test]
fn table_names_each_installed_skill_and_destination() {
    let output = render(
        &install_result(Some("/canonical/project")),
        OutputFormat::Table,
    );

    assert_eq!(
        output,
        concat!(
            "+-----------------+-----------------------------------+\n",
            "| Skill           | Destination                       |\n",
            "+-----------------+-----------------------------------+\n",
            "| bzr-bulk-triage | /canonical/project/.agents/skills |\n",
            "+-----------------+-----------------------------------+\n",
            "| bzr-file-bug    | /canonical/project/.agents/skills |\n",
            "+-----------------+-----------------------------------+\n",
            "| bzr-bulk-triage | /canonical/project/.claude/skills |\n",
            "+-----------------+-----------------------------------+\n",
            "| bzr-file-bug    | /canonical/project/.claude/skills |\n",
            "+-----------------+-----------------------------------+\n",
        )
    );
}

#[test]
fn json_wraps_exact_project_install_data() {
    let output = render(
        &install_result(Some("/canonical/project")),
        OutputFormat::Json,
    );
    let data = crate::test_helpers::json_envelope_data(&output);

    assert_eq!(
        data,
        serde_json::json!({
            "action": "install",
            "agent": "all",
            "scope": "project",
            "project": "/canonical/project",
            "destinations": [
                {
                    "layout": "agents",
                    "path": "/canonical/project/.agents/skills",
                    "installed": ["bzr-bulk-triage", "bzr-file-bug"]
                },
                {
                    "layout": "claude",
                    "path": "/canonical/project/.claude/skills",
                    "installed": ["bzr-bulk-triage", "bzr-file-bug"]
                }
            ]
        })
    );
}

#[test]
fn ndjson_writes_one_bare_compact_result_line() {
    let output = render(
        &install_result(Some("/canonical/project")),
        OutputFormat::Ndjson,
    );

    assert_eq!(output.lines().count(), 1);
    assert!(!output.contains("schema_version"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap()["scope"],
        "project"
    );
}

#[test]
fn global_json_uses_a_null_project() {
    let output = render(&install_result(None), OutputFormat::Json);
    let data = crate::test_helpers::json_envelope_data(&output);

    assert!(data["project"].is_null());
    assert_eq!(data["scope"], "global");
}

#[test]
fn json_preserves_canonical_paths_and_destination_and_skill_order() {
    let output = render(
        &install_result(Some("/canonical/project")),
        OutputFormat::Json,
    );
    let data = crate::test_helpers::json_envelope_data(&output);

    assert_eq!(data["project"], "/canonical/project");
    assert_eq!(data["destinations"][0]["layout"], "agents");
    assert_eq!(data["destinations"][1]["layout"], "claude");
    assert_eq!(
        data["destinations"][0]["installed"],
        serde_json::json!(["bzr-bulk-triage", "bzr-file-bug"])
    );
}
