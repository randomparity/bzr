# Bounded multi-bug adjacency design

## Scope authority

- Scope identity: <https://github.com/randomparity/bzr/issues/573>, token `q573-04d64ac0`.
- Outcome: add one deterministic, versioned, read-only operation for a bounded set of requested
  bugs and complete dependency adjacency evidence.
- Completion criteria: accept bounded IDs and aliases; return every successful canonical bug;
  preserve alias and numeric request identity without duplicate canonical nodes; return complete
  selected adjacency observations; deterministic ordering; typed per-ID failures for Bugzilla
  codes 100, 101, and 102; every other API and all auth/TLS/connection/transport failures remain
  command-fatal; functional coverage spans every supported Bugzilla version.
- Provenance: issue #573; issue #570's operator-confirmed three-version behavior.
- Exclusions: traversal, graph analysis, scheduling, project-management policy, mutation,
  unbounded request sets, and speculative resource-failure classifications.
- Surface: existing bug CLI, command, client/resource, output/type boundaries, CLI docs, focused
  tests, and functional phase coverage.
- Ambiguities: none.
- Interaction: unattended campaign worker.

## Chosen approach

The smallest contract that satisfies the issue is a new sibling of `bug view` and `bug links`:

```text
bzr --json bug adjacency <ID_OR_ALIAS>...
```

`bug adjacency` is a bounded multi-get, not a graph operation. It batches distinct numeric IDs
through the existing `search_bugs` boundary, then probes only omitted numerics and distinct aliases
through `get_bug`. This uses one upstream call for the common all-visible numeric case without
pretending the canonical-only batch response can carry a failure for each request.

If a credentialed per-ID probe returns code 102, the handler lazily calls the existing `whoami`
boundary, including Bugzilla 5.0's configured-email fallback. Only a successful identity check
makes that 102 resource-scoped; an unavailable check leaves it fatal. Successful Bugzilla 5.0
reads without configured email never call `whoami`, so the command adds no eager prerequisite.
Anonymous clients have no credential failure to distinguish. The handler commits output only after
every request finishes. It removes repeated process startup and connection discovery but retains a
worst case of one batch plus 100 sequential omission/alias probes. ADR
[0024](../../adr/0024-bounded-bug-adjacency-contract.md) records why this is a new command instead
of a mode on either existing command.

Two alternatives were rejected: extending `bug view --permissive` would change its prose-failure
and request-row contract, while extending `bug links` would mix a bounded retrieval primitive with
traversal policy and still inherit its root/revisit omissions. A batch response supplies common-
case successes; omissions still need single-ID probes for typed per-request classification.

## CLI contract

The command accepts 1 through 100 positional strings. Numeric IDs, aliases, leading-zero numeric
spellings, and repeated arguments remain strings at the CLI boundary. More than 100 requests is an
input-validation error attributed to `ids` before connection setup. Clap rejects zero positional
arguments as usage.

The command is anonymous-capable, read-only, and does not support field selection, recursion,
direction selection, or a permissive flag. Both dependency fields are always requested because
the charter requires complete selected adjacency evidence and issue #570 consumes both fields as
reciprocal observations. The fixed canonical bug projection is:

- `id`
- `summary`
- `status`
- `resolution`
- `product`
- `version`
- `assigned_to`
- `last_change_time`
- `target_milestone`
- `blocks`
- `depends_on`

This is the existing dependency collector's detail set, including the three fields needed for its
supported scope restrictions. No arbitrary custom fields, comments, attachments, or history are
included.

## Result schema and ordering

Pretty JSON uses the existing `{schema_version, data}` envelope. `data` has exactly two arrays:

```json
{
  "requests": [
    {"requested": "123", "bug_id": 123},
    {"requested": "release-alias", "bug_id": 123},
    {
      "requested": "999999",
      "error": {"type": "not_found", "api_code": 101}
    }
  ],
  "bugs": [
    {
      "id": 123,
      "summary": "Example",
      "status": "NEW",
      "resolution": "",
      "product": "Example Product",
      "version": "unspecified",
      "assigned_to": "owner@example.invalid",
      "last_change_time": "2026-08-29T00:00:00Z",
      "target_milestone": "---",
      "blocks": [200, 300],
      "depends_on": [10, 20]
    }
  ]
}
```

`requests` stays in original argument order. Every occurrence is retained, including repeated
identical strings, so consumers can correlate outcomes positionally without a separate index.
Numeric spellings are parsed as `u64`, deduplicated by numeric value, sorted, and fetched first;
leading-zero and decimal spellings still retain separate request entries. Aliases are deduplicated
by exact text and fetched lexically after numeric processing. Alias and numeric spellings remain
separate entries even when both resolve to the same numeric bug.

`bugs` is keyed and sorted by numeric `id`; a canonical bug appears once. The first successful
observation in deterministic fetch order wins: numeric batch rows by ID, then aliases lexically.
Later observations that resolve to the same ID map their request but do not overwrite or union the
node. `blocks` and `depends_on` are independently sorted ascending and deduplicated. These rules
make canonical node content and adjacency ordering independent of argument and Bugzilla response
order except for genuine concurrent server mutation before the deterministic winning observation.
Other values are scalars.

Table output prints two deterministic sections: a request mapping in argument order and a
canonical bug table in numeric order. Each bug row renders complete comma-separated `blocks` and
`depends_on` lists. NDJSON follows the existing repository rule for object payloads and emits the
whole result as one compact record without an envelope.

## Failure contract

The command maps only these per-request results. A credentialed code 102 must first pass lazy
identity validation:

| Source | `error.type` | `api_code` |
|---|---|---|
| Bugzilla code 100 (invalid alias) | `not_found` | `100` |
| Bugzilla code 101 (invalid bug ID) | `not_found` | `101` |
| Bugzilla code 102 (access denied) | `inaccessible` | `102` |
| Existing client `NotFound { resource: "bug" }` | `not_found` | absent |

Per-request failures contain no server message. This avoids turning attacker- or administrator-
controlled prose into a stable machine contract and avoids duplicating credential-redaction logic.
A batch containing only classified failures is still a successful report and exits zero.

Every other `BzrError` aborts the command. A batch-level 100, 101, or 102 has no per-ID identity, so
it triggers individual probes rather than becoming a request result. A `whoami` failure after a
credentialed per-ID 102 is always fatal and never converted into a request result. The handler
buffers requests and bugs and writes nothing until collection succeeds, so a fatal response never
leaves a partial success document on stdout. The ordinary structured command error remains on
stderr with its normal exit code. Connection, authentication negotiation, API-mode selection, and
TLS trust happen once before the handler.

Each successful bug record is complete for the search or Bug.get response whose observation won
the canonical-node rule. Sequential responses are not an atomic Bugzilla snapshot: another actor
may change dependencies between reads, the command does not reconcile reciprocal arrays, and
`last_change_time` is the consumer's per-node observation evidence rather than a batch timestamp.

## Components

- `src/cli/bug/adjacency.rs` defines the positional arguments and help text; `bug/mod.rs` adds the
  action.
- `src/commands/bug/adjacency.rs` validates the cap, batches numeric requests through the existing
  `SearchParams`/`search_bugs` path, probes omissions and aliases, maps typed failures, deduplicates
  canonical bugs, and delegates output.
- `src/types/bug/adjacency.rs` owns the exact request, failure, canonical-bug, and result types plus
  canonicalization from the existing `Bug` type.
- `src/output/resources/bug.rs` renders the two table sections and uses the shared JSON-family
  formatter for structured output.
- Existing `BugzillaClient::search_bugs`, `get_bug`, and `whoami` remain the protocol boundaries.
  No client abstraction is added.

## Trust boundaries and controls

### Boundary inventory

- Added: up to 100 local-operator-controlled identifiers enter the new CLI action.
- Added: Bugzilla-controlled bug objects and fault codes are aggregated into one public result.
- Existing, reused: identifiers enter the existing REST/XML-RPC search and Bug.get paths;
  connection and auth state cross the existing configuration, credential, TLS, and client
  boundaries.

### Actors and trust

The local operator controls arguments and output destination. The configured Bugzilla server and
any authenticated Bugzilla user can influence response values; neither is trusted to order arrays
or provide safe failure prose. The existing connection layer is trusted to enforce credentials,
TLS policy, timeouts, and API-mode selection.

### Controls

- Count validation runs before connection and bounds work at 100 distinct argument positions.
- Numeric IDs are sorted and deduplicated before one bounded search; exact aliases are sorted and
  deduplicated before individual fetches.
- Credentialed `whoami` validation runs lazily only after a per-ID 102; a failure is command-fatal.
  Anonymous mode carries no credential whose rejection could be confused with resource denial.
- `Bug` deserialization remains at the existing client boundary; the new code accepts no alternate
  wire schema.
- Only codes 100, 101, and 102 are downgraded, and the mapping is exhaustive and test-pinned.
- Failure records copy only the stable type and numeric code, never server prose.
- Canonical `BTreeMap` storage and explicit adjacency sorting/deduplication remove server-order
  influence.
- Output is buffered until success, so fatal errors cannot create a plausible partial document.

### Explicitly out of scope

The command does not defend against a configured server returning an extremely large single-bug
response. The existing client reads complete response bodies, and truncating adjacency would break
the charter's completeness requirement. General response-byte bounding requires a separate client
policy and server/protocol analysis. The design also does not change alias URL encoding, auth,
TLS, retries, or XML-RPC fallback behavior; it reuses those existing boundaries unchanged.

## Verification

### Focused tests

- CLI parsing accepts mixed numeric/alias inputs and rejects missing inputs.
- Command tests prove one-batch visible numeric retrieval; probes only for omitted numerics;
  resource-code batch fallback; the 101/102 mixed-success schema; code 100 alias failure;
  all-failure success; lazy credentialed 102 validation; successful credentialed Bugzilla 5.0 reads
  without email and without `whoami`; code 410 fatal behavior; transport fatal behavior with empty
  stdout; and exact-input caching.
- Command/output tests prove alias-plus-numeric convergence, one canonical bug, positional request
  identity, numeric node order, and sorted/deduplicated adjacency arrays.
- A controlled-fault test changes one accepted resource code to fatal and must make the focused
  mixed-result test fail before the fault is reverted.

### Functional matrix

Extend a functional phase used by every supported container version. Create two related public
bugs and one restricted bug, then assert:

1. numeric IDs and an alias resolve in one invocation;
2. alias plus its numeric ID produce two request entries and one canonical bug;
3. both adjacency arrays contain the complete expected IDs in numeric order;
4. a missing numeric ID is a typed `not_found` request with code 101;
5. the restricted bug is a typed `inaccessible` request with code 102 under the credentialless
   server path;
6. the mixed operation exits zero; and
7. an unsupported/auth-flavored API error remains covered as command-fatal in the focused command
   suite because the stock functional servers do not provide a safe way to synthesize it.

Run `make lint`, `make test`, and `make functional-test-all` before delivery. The host is arm64;
the declared project targets are x86_64/aarch64 Linux, powerpc64le Linux, s390x Linux,
aarch64 macOS, and x86_64/aarch64 Windows. The host differs from part of the target matrix; CI and
the existing cross-build configuration remain responsible for non-host compilation.
