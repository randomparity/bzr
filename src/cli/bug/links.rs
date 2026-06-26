use clap::Args;

use crate::types::bug::LinkRelation;

pub const LONG_ABOUT: &str = r"Print a bug's relationship graph.

Emits one record per related bug across all six Bugzilla relationship
types (depends_on, blocks, dupe_of, duplicates, regressed_by,
regressions) as a flat list. With --recursive --depth N it performs a
bounded, cycle-safe breadth-first walk, one record per related bug with
its relation, direction, and hop distance.

Read-only; works against public servers without an API key.

Examples:

  bzr bug links 12345
  bzr bug links 12345 --recursive --depth 2 --output ndjson
  bzr bug links 12345 --relation depends_on
  bzr --json bug links 12345";

/// Arguments for `bug links`.
#[derive(Args, Debug)]
pub(crate) struct LinksArgs {
    /// Bug ID
    pub id: u64,
    /// Walk the relationship graph recursively (breadth-first) instead of one hop.
    #[arg(long)]
    pub recursive: bool,
    /// Maximum hop distance from the root (1..=10); only with --recursive.
    #[arg(
        long,
        value_name = "N",
        default_value_t = 1,
        requires = "recursive",
        value_parser = clap::value_parser!(u32).range(1..=10)
    )]
    pub depth: u32,
    /// Restrict traversal and output to one relationship type.
    #[arg(long, value_name = "TYPE")]
    pub relation: Option<LinkRelation>,
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
