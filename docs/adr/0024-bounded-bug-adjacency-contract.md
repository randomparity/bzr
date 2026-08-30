# ADR 0024: Bound multi-bug adjacency at the CLI request boundary

## Status

Accepted

## Context

Dependency consumers need one read-only `bzr` invocation that preserves each requested bug ID or
alias, returns each successful canonical bug once with complete `blocks` and `depends_on` lists,
and turns only Bugzilla's resource-scoped codes 100, 101, and 102 into typed per-request results.
`bug view --permissive` retains prose failures and duplicate canonical bugs, while `bug links`
performs traversal and omits roots and repeated observations. Bugzilla does not return typed
per-request failures from its multi-ID REST response, so one upstream request cannot provide the
required contract on every supported server.

## Decision

Add `bzr bug adjacency <ID_OR_ALIAS>...` as a non-traversing command with a maximum of 100 request
arguments. Before classifying code 102, a credentialed invocation validates its current identity
through the existing `whoami` boundary; a failed or unavailable identity check is command-fatal.
An anonymous invocation has no credential that can fail. The command then performs the existing
single-bug lookup once per distinct textual request and emits one versioned result containing:

- `requests` in argument order, preserving each original string and either its canonical `bug_id`
  or a typed `error`;
- `bugs` sorted by canonical numeric ID, with one node per successful canonical bug; and
- each node's complete `blocks` and `depends_on` arrays, sorted and deduplicated numerically.

After that preflight, codes 100 and 101 serialize as `not_found`; code 102 serializes as
`inaccessible`. A client-side `NotFound` is also `not_found` without an `api_code`. Every other API
error and every auth, TLS, connection, HTTP, deserialization, and transport error aborts the
command without a success body. The command reuses the existing `BugzillaClient::get_bug` and
`BugzillaClient::whoami` protocol/fallback boundaries and assembles the batch in the command
layer; it adds no graph traversal or analysis policy.

## Consequences

- Consumers replace many process invocations with one bounded invocation while retaining the
  per-ID fault classification available from single-ID Bug.get behavior. It removes repeated
  process, configuration, and connection setup, but still performs up to 100 sequential upstream
  single-bug calls and therefore retains their cumulative latency and retry exposure.
- Alias and numeric requests may both map to one `bugs` entry; the `requests` mapping preserves
  why that node was requested.
- The 100-request limit is a judgmental safety ceiling: it keeps one invocation useful for a
  sizeable frontier while preventing an unbounded number of sequential calls. A server can still
  return a large adjacency array for one bug: completeness and a hard per-array cap are mutually
  exclusive without server support, so the command does not claim a byte or edge bound.
- Completeness is per successful Bug.get response at that request's observation time. The result
  is not an atomic snapshot, and the command neither reconciles nor fabricates reciprocal edges;
  `last_change_time` is observation evidence, not a batch timestamp.
- Pretty JSON inherits the repository's `schema_version` envelope; NDJSON remains intentionally
  unenveloped under the existing output contract.

## Considered & rejected

- **Extend `bug view --permissive`.** verified: `rg -n "BugViewFailure|permissive"
  src/commands/bug/view.rs src/output/result_types.rs` at commit
  `9a7c05735c81107c5f1e74e727eba59a1b293ebb` shows its public failure shape is prose and its
  successful rows are request-oriented, so changing it would restructure an existing contract.
- **Extend `bug links`.** verified: `sed -n '1,220p' src/commands/bug/links.rs` at commit
  `9a7c05735c81107c5f1e74e727eba59a1b293ebb` shows traversal owns a visited set and emits only
  discovered related bugs, not requested roots or complete revisited observations.
- **Keep the status quo of one `bzr bug view` process per bug.** judgment: it preserves typed
  single-ID behavior but fails the chartered one-operation aggregate and repeats process,
  configuration, connection, and output-envelope work for every bug.
- **Use one multi-ID Bugzilla REST request.** verified: `make functional-test-all` runs
  `tests/functional/phases/18d-dependency-analysis.sh` on `bz50`, `bz52`, and `bz53`; cases 123n,
  123o, 123o2, and 123o3 at merge commit
  `2021055150121e8913a86b3368973c0b6f49bef8` establish the successful-preflight boundary for typed
  single-ID missing/inaccessible results and fatal rejected credentials. Issue #573 records the
  complementary observed multi-ID omission/prose behavior that motivated this primitive.
- **Add traversal or relationship caps.** judgment: graph traversal is explicitly excluded, and
  truncating a returned adjacency list would violate the required complete-observation contract.
