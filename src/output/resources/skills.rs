//! Presentation for bundled skill installation results.

use std::io::Write;

use serde::Serialize;

use crate::cli::AgentTarget;
use crate::output::formatting::{write_formatted, write_table_records};
use crate::types::output::OutputFormat;

/// One agent-layout destination populated by an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SkillsDestinationResult {
    pub layout: String,
    pub path: String,
    pub installed: Vec<String>,
}

/// The complete successful result of a bundled skill installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SkillsInstallResult {
    pub action: String,
    pub agent: AgentTarget,
    pub scope: String,
    pub project: Option<String>,
    pub destinations: Vec<SkillsDestinationResult>,
}

/// Write a successful skill-install result in the selected output format.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 3 writes the success result after filesystem installation"
    )
)]
pub(crate) fn write_skills_install<W: Write + ?Sized>(
    result: &SkillsInstallResult,
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(result, format, out, |result, out| {
        let mut rows = Vec::new();
        for destination in &result.destinations {
            for skill in &destination.installed {
                rows.push(vec![skill.clone(), destination.path.clone()]);
            }
        }
        write_table_records(&["Skill", "Destination"], rows, out);
    });
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
