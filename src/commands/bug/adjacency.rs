use std::collections::BTreeMap;

use crate::cli::AdjacencyArgs;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::output::resources::bug::write_bug_adjacency;
use crate::output::writers::Writers;
use crate::types::bug::{
    BugAdjacencyBug, BugAdjacencyError, BugAdjacencyRequest, BugAdjacencyResult,
};
use crate::types::{AuthMode, OutputFormat};

const MAX_REQUESTS: usize = 100;

#[derive(Clone, Copy)]
enum CachedOutcome {
    Success(u64),
    Failure(BugAdjacencyError),
}

pub(super) fn validate(args: &AdjacencyArgs) -> Result<()> {
    if args.ids.is_empty() {
        return Err(BzrError::input_field(
            "bug adjacency requires at least one id".into(),
            "ids",
            None,
        ));
    }
    if args.ids.len() > MAX_REQUESTS {
        return Err(BzrError::input_field(
            format!("bug adjacency accepts at most {MAX_REQUESTS} ids"),
            "ids",
            Some(args.ids.len().to_string()),
        ));
    }
    for requested in &args.ids {
        if requested.is_empty() {
            return Err(BzrError::input_field(
                "bug adjacency ids cannot contain an empty value".into(),
                "ids",
                Some(String::new()),
            ));
        }
        if requested.bytes().all(|byte| byte.is_ascii_digit()) && requested.parse::<i64>().is_err()
        {
            return Err(BzrError::input_field(
                format!("bug adjacency numeric id exceeds {}", i64::MAX),
                "ids",
                Some(requested.clone()),
            ));
        }
    }
    Ok(())
}

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &AdjacencyArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let mut numeric_requests = BTreeMap::new();
    let mut alias_requests = BTreeMap::new();
    for requested in &args.ids {
        if let Some(numeric) = crate::client::parse_adjacency_numeric(requested) {
            numeric_requests
                .entry(numeric)
                .or_insert_with(|| numeric.to_string());
        } else {
            alias_requests
                .entry(requested.clone())
                .or_insert_with(|| requested.clone());
        }
    }

    let mut numeric_outcomes = BTreeMap::new();
    let mut alias_outcomes = BTreeMap::new();
    let mut bugs = BTreeMap::new();

    for (numeric, requested) in &numeric_requests {
        let outcome = collect_one(client, requested, &mut bugs).await?;
        numeric_outcomes.insert(*numeric, outcome);
    }
    for requested in alias_requests.values() {
        let outcome = collect_one(client, requested, &mut bugs).await?;
        alias_outcomes.insert(requested.clone(), outcome);
    }

    let requests = args
        .ids
        .iter()
        .map(|requested| -> Result<BugAdjacencyRequest> {
            let outcome =
                if let Some(numeric) = crate::client::parse_adjacency_numeric(requested) {
                    numeric_outcomes.get(&numeric)
                } else {
                    alias_outcomes.get(requested)
                }
                .copied()
                .ok_or_else(|| {
                    BzrError::DataIntegrity(format!(
                        "adjacency request '{requested}' was not collected"
                    ))
                })?;
            Ok(request_result(requested.clone(), outcome))
        })
        .collect::<Result<Vec<_>>>()?;
    let result = BugAdjacencyResult {
        requests,
        bugs: bugs.into_values().collect(),
    };
    write_bug_adjacency(&result, format, w.out);
    Ok(())
}

async fn collect_one(
    client: &BugzillaClient,
    requested: &str,
    bugs: &mut BTreeMap<u64, BugAdjacencyBug>,
) -> Result<CachedOutcome> {
    match client.get_bug_adjacency(requested).await? {
        Ok(mut bug) => {
            bug.blocks.sort_unstable();
            bug.blocks.dedup();
            bug.depends_on.sort_unstable();
            bug.depends_on.dedup();
            let id = bug.id;
            bugs.entry(id).or_insert(bug);
            Ok(CachedOutcome::Success(id))
        }
        Err(error) => {
            if error == BugAdjacencyError::Inaccessible && client.auth_mode() == AuthMode::ApiKey {
                client.prove_current_credentials().await?;
            }
            Ok(CachedOutcome::Failure(error))
        }
    }
}

fn request_result(requested: String, outcome: CachedOutcome) -> BugAdjacencyRequest {
    match outcome {
        CachedOutcome::Success(bug_id) => BugAdjacencyRequest::Success { requested, bug_id },
        CachedOutcome::Failure(error) => BugAdjacencyRequest::Failure { requested, error },
    }
}

#[cfg(test)]
#[path = "adjacency_tests.rs"]
mod tests;
