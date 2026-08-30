use clap::Args;

pub const LONG_ABOUT: &str = r"Retrieve bounded dependency adjacency for bug IDs and aliases.

Fetches each distinct numeric ID and exact alias once, preserving every
positional request in the result. Successful canonical bugs include complete
sorted blocks and depends_on arrays. Bugzilla resource failures 100, 101, and
102 are returned as typed per-request outcomes; other failures abort without a
partial result.

Read-only and limited to 100 positional requests. Public bugs can be read
without an API key.";

#[derive(Args, Debug, Clone)]
pub(crate) struct AdjacencyArgs {
    /// Bug IDs or aliases to retrieve.
    #[arg(required = true, num_args = 1.., value_name = "ID_OR_ALIAS")]
    pub ids: Vec<String>,
}

#[cfg(test)]
#[path = "adjacency_tests.rs"]
mod tests;
