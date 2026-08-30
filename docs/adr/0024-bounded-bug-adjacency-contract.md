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
arguments. The command parses and sorts distinct numeric requests, retrieves their successful rows
through one existing `search_bugs` call, and probes only omitted numeric requests through `get_bug`
for typed faults. A batch code 100, 101, or 102 triggers the same per-ID probes because the batch
fault cannot be attributed safely. Distinct aliases are fetched individually in lexical order
because a canonical-only batch response cannot preserve alias identity.

When a credentialed per-ID lookup returns code 102, the command lazily validates the configured
email and current credential through Bugzilla's `valid_login` endpoint using the client's current
auth method. Only `result: true` (or Bugzilla's equivalent integer `1`) proves that the 102 is
resource-scoped. A missing email, rejected credential, malformed response, or unavailable
validation leaves that 102 command-fatal. In particular, the Bugzilla 5.0 `whoami` user-lookup
fallback is not an authentication proof because that lookup can succeed anonymously. An anonymous
invocation has no credential that can fail. The command emits one versioned result containing:

- `requests` in argument order, preserving each original string and either its canonical `bug_id`
  or a typed `error`;
- `bugs` sorted by canonical numeric ID, with one node per successful canonical bug; and
- each node's complete `blocks` and `depends_on` arrays, sorted and deduplicated numerically.

After that validation, codes 100 and 101 serialize as `not_found`; code 102 serializes as
`inaccessible`. A client-side `NotFound` is also `not_found` without an `api_code`. Every other API
error and every auth, TLS, connection, HTTP, deserialization, and transport error aborts the
command without a success body. The command reuses the existing `BugzillaClient::search_bugs` and
`BugzillaClient::get_bug` boundaries and exposes the existing `valid_login` response handling as a
current-client credential check; it adds no graph traversal or analysis policy.

## Consequences

- Consumers replace many process invocations with one bounded invocation while retaining the
  per-ID fault classification available from single-ID Bug.get behavior. The all-visible numeric
  path uses one upstream search. The worst case uses one batch plus 100 sequential alias or omitted
  numeric probes and therefore still carries cumulative latency and retry exposure.
- Alias and numeric requests may both map to one `bugs` entry; the `requests` mapping preserves
  why that node was requested.
- The 100-request limit is a judgmental safety ceiling: it keeps one invocation useful for a
  sizeable frontier while preventing an unbounded number of sequential calls. A server can still
  return a large adjacency array for one bug: completeness and a hard per-array cap are mutually
  exclusive without server support, so the command does not claim a byte or edge bound.
- Completeness is per successful search or Bug.get response at that observation time. The result is
  not an atomic snapshot, and the command neither reconciles nor fabricates reciprocal edges;
  `last_change_time` is observation evidence, not a batch timestamp.
- A canonical node retains its first successful observation in deterministic fetch order: numeric
  batch rows by canonical ID, then successful omitted or fallback numeric probes by requested
  numeric ID, then successful alias probes lexically. Requests are mapped only after those phases.
  Later requests mapping to that ID never overwrite or union fields, so input permutation and
  concurrent changes cannot select an implicit merge rule.
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
- **Perform only individual lookups.** verified: `rg -n "pub async fn search_bugs" \
  src/client/resources/bug.rs` at commit
  `9a7c05735c81107c5f1e74e727eba59a1b293ebb` finds the installed multi-ID numeric search boundary.
  Reusing it for successes and probing only omissions meets the typed-failure contract with fewer
  common-case server round trips than individual-only retrieval.
- **Treat the batch response alone as complete.** verified: issue #573's observed supported-server
  behavior and `src/client/resources/bug.rs`'s `BugListResponse { bugs }` shape provide no
  per-request failure channel, so omitted requests still need single-ID probes for stable codes.
- **Add traversal or relationship caps.** judgment: graph traversal is explicitly excluded, and
  truncating a returned adjacency list would violate the required complete-observation contract.
