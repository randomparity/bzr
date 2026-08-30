# ADR 0024: Bound multi-bug adjacency at the CLI request boundary

## Status

Accepted

## Context

Dependency consumers need one read-only `bzr` invocation that preserves each requested bug ID or
alias, returns each successful canonical bug once with complete `blocks` and `depends_on` lists,
and turns only Bugzilla's resource-scoped codes 100, 101, and 102 into typed per-request results.
`bug view --permissive` retains prose failures and duplicate canonical bugs, while `bug links`
performs traversal and omits roots and repeated observations. Bugzilla's permissive multi-ID
`Bug.get` response does carry identity-bearing faults, but the shared tolerant `Bug` mapping cannot
distinguish an absent or malformed adjacency field from an empty one. The existing retrieval result
therefore cannot provide the required contract on every supported server.

## Decision

Add `bzr bug adjacency <ID_OR_ALIAS>...` as a non-traversing command with a maximum of 100 request
arguments. The command parses and sorts distinct numeric requests and retrieves their rows and
identity-bearing faults through one adjacency-specific multi-ID `Bug.get` call. Distinct aliases
use individual adjacency-specific `Bug.get` calls because successful alias results contain only
the canonical bug ID. Both forms use Bug.get's supported `permissive` protocol parameter, so a
resource failure is returned inside a 2xx `faults` array on Bugzilla 5.0, 5.2, and 5.3 instead of
as its ordinary 400/401/404 status. Both forms request the fixed projection and reject a
successful bug row unless `blocks` and `depends_on` are present
arrays containing only non-negative integer bug IDs. They use focused strict REST and XML-RPC
response mappings so the shared tolerant `Bug` behavior remains unchanged for existing commands.
Only a 2xx transport status can reach those mappings; every non-2xx, including a redirect carrying
a valid-looking body, is command-fatal before parsing.

A numeric multi-ID response must contain exactly one outcome for each requested numeric identity:
either one identity-valid bug row or one identity-valid fault, with no unrequested, duplicate,
missing, or mixed identity. A permissive alias response must likewise contain exactly one outcome:
one bug and no faults, or one fault and no bugs. A numeric bug row or fault ID must equal the
numeric request; aliases may resolve to any canonical bug ID, while an alias fault must preserve
the exact requested alias. Invalid outcome sets are command-fatal data-integrity errors. Distinct
aliases are fetched individually in lexical order because a successful Bug.get row does not echo
the alias that selected it.

When a credentialed permissive fault returns code 102, the command lazily validates the configured
email and current credential through Bugzilla's `valid_login` endpoint using the client's current
auth method. Only `result: true` (or Bugzilla's equivalent integer `1`) proves that the 102 is
resource-scoped. A missing email, rejected credential, malformed response, or unavailable
validation leaves that 102 command-fatal. In particular, the Bugzilla 5.0 `whoami` user-lookup
fallback is not an authentication proof because that lookup can succeed anonymously. An anonymous
invocation has no credential that can fail. Each credentialed code 102 gets its own contemporaneous
proof; proof is not cached across responses. The command emits one versioned result containing:

- `requests` in argument order, preserving each original string and either its canonical `bug_id`
  or a typed `error`;
- `bugs` sorted by canonical numeric ID, with one node per successful canonical bug; and
- each node's complete `blocks` and `depends_on` arrays, sorted and deduplicated numerically.

After that validation, faults 100 and 101 serialize as `not_found`; fault 102 serializes as
`inaccessible`. Every other fault or API error and every auth, TLS, connection, HTTP,
deserialization, and transport error aborts the command without a success body. The command adds
focused adjacency retrieval methods beside the existing tolerant bug methods and exposes the
existing `valid_login` response handling as a current-client credential check; it adds no graph
traversal or analysis policy.

## Consequences

- Consumers replace many process invocations with one bounded invocation while retaining the
  per-ID fault classification available from permissive Bug.get behavior. The all-numeric path
  uses one upstream multi-get. The worst case uses 100 sequential alias gets and 100 credential
  proofs: 200 upstream calls. It therefore still carries cumulative latency exposure. These
  adjacency retrieval and proof calls do not use transient retries or automatic redirects, so 200
  is a physical application-request ceiling after shared connection establishment;
  connection/version/TLS probes are outside the operation's retrieval
  budget.
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
  multi-get rows by canonical ID, then successful alias gets lexically. Requests are mapped only
  after those phases.
  Later requests mapping to that ID never overwrite or union fields, so input permutation and
  concurrent changes cannot select an implicit merge rule.
- The new public payload is additive, so `SCHEMA_VERSION` advances from `0.6.1` to `0.6.2` under
  ADR 0007. The constant, CLI reference examples, and functional assertions advance together.
  Pretty JSON inherits the envelope; NDJSON remains intentionally unenveloped under the existing
  output contract.

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
- **Perform only individual lookups.** verified: the supported Bugzilla 5.0, 5.2, and 5.3
  `Bug.get` implementations iterate over the supplied `ids` and, with `permissive`, return both
  successful `bugs` and identity-bearing `faults`. One numeric multi-get therefore preserves typed
  outcomes with fewer server round trips than individual-only retrieval.
- **Use `Bug.search` for the numeric batch.** rejected: search returns only successful rows, so
  omissions need additional Bug.get probes. Protocol-native permissive multi-ID Bug.get returns
  the same successes and correlated resource faults in one response.
- **Add traversal or relationship caps.** judgment: graph traversal is explicitly excluded, and
  truncating a returned adjacency list would violate the required complete-observation contract.
