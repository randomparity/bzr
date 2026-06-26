use std::collections::{BTreeMap, BTreeSet};

use crate::cli::LinksArgs;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::output::resources::bug::write_bug_links;
use crate::output::writers::Writers;
use crate::types::bug::{BugLink, BugLinksNode, LinkRelation, LINKS_MAX_NODES};
use crate::types::output::OutputFormat;

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &LinksArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let LinksArgs {
        id,
        recursive,
        depth,
        relation,
    } = args;
    let max_depth = if *recursive { *depth } else { 1 };

    let root_nodes = client.get_bug_links_nodes(&[*id]).await?;
    let root = root_nodes
        .into_iter()
        .find(|n| n.id == *id)
        .ok_or_else(|| BzrError::NotFound {
            resource: "bug",
            id: id.to_string(),
        })?;

    let mut visited: BTreeSet<u64> = BTreeSet::new();
    visited.insert(*id);
    let mut current_nodes: Vec<BugLinksNode> = vec![root];
    let mut results: Vec<BugLink> = Vec::new();
    let mut current_depth: u32 = 0;
    let mut truncated = false;

    while current_depth < max_depth {
        current_nodes.sort_by_key(|n| n.id);
        let mut frontier: Vec<(u64, LinkRelation)> = Vec::new();
        'discover: for node in &current_nodes {
            for (rel, neighbor) in node.edges(*relation) {
                if visited.contains(&neighbor) {
                    continue;
                }
                // `visited` includes the root, so the count of distinct related
                // bugs is `len() - 1`; stop once that reaches the cap.
                if visited.len() > LINKS_MAX_NODES {
                    truncated = true;
                    break 'discover;
                }
                visited.insert(neighbor);
                frontier.push((neighbor, rel));
            }
        }
        if frontier.is_empty() {
            break;
        }
        current_depth += 1;

        let ids: Vec<u64> = frontier.iter().map(|(n, _)| *n).collect();
        let fetched = client.get_bug_links_nodes(&ids).await?;
        let mut by_id: BTreeMap<u64, BugLinksNode> =
            fetched.into_iter().map(|n| (n.id, n)).collect();

        let mut next_nodes = Vec::new();
        for (neighbor, rel) in frontier {
            if let Some(node) = by_id.remove(&neighbor) {
                results.push(BugLink {
                    id: neighbor,
                    relation: rel,
                    direction: rel.direction(),
                    depth: current_depth,
                    summary: node.summary.clone(),
                    status: node.status.clone(),
                });
                next_nodes.push(node);
            }
        }
        current_nodes = next_nodes;
        if truncated {
            break;
        }
    }

    if truncated {
        let _ = writeln!(
            w.err,
            "stopped at LINKS_MAX_NODES ({LINKS_MAX_NODES}) related bugs; results may be incomplete"
        );
    }
    write_bug_links(&results, format, w.out);
    if results.is_empty() && matches!(format, OutputFormat::Table) {
        let _ = writeln!(w.out, "No related bugs for #{id}.");
    }
    Ok(())
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
