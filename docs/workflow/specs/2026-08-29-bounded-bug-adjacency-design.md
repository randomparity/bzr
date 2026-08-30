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
through a focused strict adjacency variant of the existing search transport, then probes only
omitted numerics and distinct aliases through a strict Bug.get call with the protocol's supported
`permissive` parameter. On Bugzilla 5.0, 5.2, and 5.3, permissive Bug.get returns resource failures
inside a 2xx `faults` array instead of the ordinary code-100/101/102 HTTP status. Both variants
request the fixed projection and reject successful REST or XML-RPC bug rows unless `blocks` and
`depends_on` are present arrays containing only non-negative integer IDs. The shared tolerant `Bug`
mapping remains unchanged for existing commands. This uses one upstream call for the common
all-visible numeric case without pretending the canonical-only batch response can carry a failure
for each request or that a missing adjacency field means an empty adjacency list.

The strict boundary also validates response identity before the command records observations. A
batch may contain at most one row for each requested numeric ID and no unrequested ID. A permissive
single response must contain exactly one outcome: one bug and no faults, or one fault and no bugs.
A numeric bug or fault identity must equal the requested numeric value; an alias may resolve to any
canonical bug ID, while an alias fault must preserve the exact requested string. Empty, multiple,
mixed, extra, duplicate, or mismatched outcomes are command-fatal data-integrity errors and never
become inputs to the first-observation rule.

If a credentialed per-ID probe returns code 102, the handler lazily validates the configured email
and current credential through `rest/valid_login`. For REST it applies the same auth method that
produced Bug.get. XML-RPC always sends the credential in its request body, so `valid_login` applies
the client's configured REST auth method as a conservative independent credential proof; inability
to prove it remains fatal. Only `result: true` or Bugzilla's equivalent integer `1` makes that 102
resource-scoped. The focused proof parser inspects the raw JSON envelope before reading `result`:
top-level `error: true`, a non-boolean `error` marker, rejected credentials, malformed response,
non-success status, redirect, or transport failure leaves the 102 command-fatal. The check does not
use Bugzilla 5.0's `whoami` fallback:
anonymous user lookup can return a user and therefore cannot prove authentication. Successful
credentialed reads without configured email add no eager prerequisite; they become fatal only if a
102 needs classification. Anonymous clients have no credential failure to distinguish. The handler
does not cache a successful proof: every credentialed code 102 gets a contemporaneous
`valid_login` call, including repeated 102s in the same invocation. It commits output only after
every request finishes. The worst case is one batch, 100 sequential omission/alias probes, and 100
credential proofs: 201 physical application requests after shared connection establishment.
Adjacency retrieval and proof calls do not use transient retries or automatic redirects;
connection, version, auth, and TLS probes occur before retrieval and are outside this operation
budget. ADR
[0024](../../adr/0024-bounded-bug-adjacency-contract.md) records why this is a new command instead
of a mode on either existing command.

Strict adjacency REST sends use a focused HTTP client built from the same TLS policy with automatic
redirects disabled. They also disable transient retries and the transport's transparent 401
alternate-auth fallback. The configured client method therefore necessarily produced any returned
code 102, and `valid_login` applies that same method through the same no-redirect client. A redirect
or a 401 from a stale or wrong cached method stays command-fatal instead of changing request count
or authentication provenance.

Strict REST and strict XML-RPC require `status.is_success()` before any error-envelope, JSON,
XML-RPC, fault, or row parsing. Every non-2xx response is command-fatal, including ordinary
Bugzilla 400/401/404 resource errors and a 3xx that carries a `Location` header and an otherwise
schema-valid body. Typed resource results come only from a 2xx permissive single response's strict
`faults` entry. A non-2xx body cannot supply an outcome, and output remains buffered and empty.
Adjacency retrieval also defines stricter API-mode routing: `Rest` and `Hybrid` use only the strict
REST calls, while `XmlRpc` uses only strict XML-RPC. A successful empty REST batch is returned to the
command for per-ID REST probes; it never triggers XML-RPC comparison. No REST error, including 401,
connection/HTTP failure, or API code 100500, falls back to XML-RPC. XML-RPC already sends the
configured credential in its protocol body and has no header/query fallback. Focused tests cover a
configured header returning 401 where alternate query auth would return 102, REST transport and
code-100500 failures, and an empty batch; none may invoke XML-RPC or `valid_login`.

Two alternatives were rejected: extending `bug view --permissive` would change its prose-failure
and request-row contract, while extending `bug links` would mix a bounded retrieval primitive with
traversal policy and still inherit its root/revisit omissions. A batch response supplies common-
case successes; omissions still need single-ID probes for typed per-request classification.

## CLI contract

The command accepts 1 through 100 positional strings. Numeric IDs, aliases, leading-zero numeric
spellings, and repeated arguments remain strings at the CLI boundary. An all-decimal value is a
numeric request only within `0..=9223372036854775807`, the signed range supported by every
transport; larger decimal values are an `ids` input-validation error before connection setup. More
than 100 requests is the same pre-connection validation class. Clap rejects zero positional
arguments as usage. An explicitly empty positional string is an `ids` input-validation error before
connection and is never sent as an alias. The strict wire mappings and public schema apply the same
maximum to canonical and adjacency bug IDs returned by the server; every serialized `requested`
string is non-empty.

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

Every canonical bug object contains all eleven keys. `id` is a required integer from `0` through
`9223372036854775807`; `blocks` and `depends_on` are required arrays whose members have the same
range. The eight scalar detail fields are required keys whose values are either a non-empty string
or `null`. REST and XML-RPC both normalize a missing or empty scalar to `null`, so equivalent wire
observations serialize identically.

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
      "resolution": null,
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

Each request entry is an exclusive closed union. A success has exactly `requested` and `bug_id`; a
failure has exactly `requested` and `error`. The closed failure object is itself one of exactly
two correlated variants: `not_found` with required `api_code` 100 or 101, or `inaccessible` with
required `api_code` 102. No entry may contain both `bug_id` and `error` or neither. Every
string-valued scalar is non-empty; empty wire strings have already normalized to JSON `null`.

`bugs` is keyed and sorted by numeric `id`; a canonical bug appears once. The first successful
observation in deterministic fetch order wins: numeric batch rows by canonical ID, then successful
omitted or fallback numeric probes by requested numeric ID, then successful alias probes lexically.
Requests are mapped only after those phases. Later observations that resolve to the same ID map
their request but do not overwrite or union the node. `blocks` and `depends_on` are independently
sorted ascending and deduplicated. These rules make canonical node content and adjacency ordering
independent of argument and Bugzilla response order except for genuine concurrent server mutation
before the deterministic winning observation. Other values are scalars.

Table output prints two deterministic sections: a request mapping in argument order and a
canonical bug table in numeric order. Each bug row renders complete comma-separated `blocks` and
`depends_on` lists. NDJSON follows the existing repository rule for object payloads and emits the
whole result as one compact record without an envelope.

The new payload is an additive public-contract change, so the implementation bumps
`output::SCHEMA_VERSION` from `0.6.1` to `0.6.2` under ADR 0007. All live consumers advance in the
same change: `docs/bzr-cli.md`; `tests/functional/phases/18a-json-envelope.sh`;
`content/skills/bzr-dependency-analysis/scripts/collect.py`; its unit fixtures and recording runner;
the installed-skill replay in `tests/functional/phases/18c-skills-install.sh`; and
`content/skills/bzr-reference/reference/json-recipes.md`. The functional dependency-analysis
pipeline must run the installed collector against the newly built `0.6.2` binary, not only replayed
envelopes. Existing historical ADR and design examples retain the versions they documented.

Publish the closed payload contract as `bzr schema bug-adjacency`. The schema requires the top-level
`requests` and `bugs` arrays, the exclusive request-entry union, all canonical bug keys and their
nullability, and rejects undeclared properties. Register it in the sorted schema registry and pin
representative maximal success, nullable-scalar, and per-request failure values in the schema drift
tests.

## Failure contract

The command maps only these per-request permissive faults. A credentialed code 102 must first pass
lazy identity validation:

| Source | `error.type` | `api_code` |
|---|---|---|
| Bugzilla code 100 (invalid alias) | `not_found` | `100` |
| Bugzilla code 101 (invalid bug ID) | `not_found` | `101` |
| Bugzilla code 102 (access denied) | `inaccessible` | `102` |
Per-request failures contain no server message. This avoids turning attacker- or administrator-
controlled prose into a stable machine contract and avoids duplicating credential-redaction logic.
A batch containing only classified failures is still a successful report and exits zero.

Every other `BzrError` or permissive fault aborts the command. A batch error has no per-ID identity
and is always fatal, including code 100, 101, or 102. A credentialed per-ID permissive fault 102 is
also fatal unless `valid_login` conclusively accepts the configured email and current credential
under the same auth method; missing email, `false`, malformed output, API failure, and transport
failure are never converted into request results. The handler buffers requests and bugs and writes
nothing until collection succeeds, so a fatal response never leaves a partial success document on
stdout. The ordinary structured command error remains on stderr with its normal exit code.
Connection, authentication negotiation, API-mode selection, and TLS trust happen once before the
handler.

Strict adjacency REST parsing does not use the shared ADR-0015 populated-data warning downgrade.
It inspects the raw JSON envelope before typed row deserialization: any top-level `error: true`
becomes its `BzrError::Api` even when `bugs` is populated. Every batch error and every single error
outside a well-formed permissive `faults` outcome is command-fatal. Existing REST callers retain the
shared warning-with-data behavior.

Each successful bug record is complete for the search or Bug.get response whose observation won
the canonical-node rule. Sequential responses are not an atomic Bugzilla snapshot: another actor
may change dependencies between reads, the command does not reconcile reciprocal arrays, and
`last_change_time` is the consumer's per-node observation evidence rather than a batch timestamp.

## Components

- `src/cli/bug/adjacency.rs` defines the positional arguments and help text; `bug/mod.rs` adds the
  action.
- `src/commands/bug/adjacency.rs` validates the cap, batches numeric requests through the strict
  adjacency path, probes omissions and aliases, maps typed failures, deduplicates canonical bugs,
  and delegates output.
- `src/types/bug/adjacency.rs` owns the exact request, failure, canonical-bug, and result types. Its
  REST wire form requires both adjacency arrays rather than converting from the tolerant `Bug`.
- `src/client/resources/bug.rs` adds strict batch search and permissive single-Bug.get adjacency
  methods with the strict routing rule: `Rest` and `Hybrid` use REST only, while `XmlRpc` uses
  XML-RPC only. Strict REST sends disable transient retries and transparent alternate-auth fallback.
- `src/xmlrpc/resources/bug.rs` adds a strict adjacency mapper that requires both arrays and every
  member to be a non-negative integer. Its strict calls use an XML-RPC client backed by the focused
  no-redirect HTTP client; existing tolerant bug reads keep their current behavior.
- `src/client/transport.rs` exposes the focused no-auth-fallback send path used only by strict
  adjacency REST retrieval. It sends once without redirect, transient, or alternate-auth retries,
  requires a 2xx response before returning it for parsing, and leaves ordinary command behavior
  unchanged.
- `src/client/response.rs` exposes a focused strict-error parser that turns every top-level
  `error: true` envelope into `BzrError::Api` and rejects a non-boolean `error` marker before
  deserializing adjacency rows or credential proof; the shared ADR-0015 warning-with-data parser
  remains unchanged.
- `src/tls/mod.rs` builds the focused no-redirect client from the same certificate, CA, issuer, pin,
  timeout, and insecurity policy as the ordinary client. `BugzillaClient` and its strict XML-RPC
  adapter retain that client only for bounded adjacency retrieval and proof.
- `src/output/resources/bug.rs` renders the two table sections and uses the shared JSON-family
  formatter for structured output.
- A focused `BugzillaClient` credential-validation method reuses the installed `valid_login`
  response rules while applying the client's current auth method; it neither re-detects nor
  changes that method.
- `schemas/bug-adjacency.json` and the sorted registry/drift samples in
  `src/commands/schema.rs` and `src/commands/schema_tests.rs` publish and pin the exact payload.
- `src/output/mod.rs`, current CLI documentation, the embedded dependency-analysis collector and
  its fixtures, the embedded JSON reference, and functional schema fixtures advance together to
  `SCHEMA_VERSION` `0.6.2`.
- `tests/functional/versions/bz50/entrypoint.sh`, `bz52/entrypoint.sh`, and `bz53/entrypoint.sh`
  enable `usebugaliases` in each disposable server's generated parameter file before Apache starts.

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
- Empty positional strings are rejected as `ids` input errors before connection and cannot reach an
  alias URL or the published result.
- Numeric validation caps every transport at `i64::MAX` before connection; strict response parsing
  and the schema apply the same maximum to canonical and adjacency IDs.
- Numeric IDs are sorted and deduplicated before one bounded search; exact aliases are sorted and
  deduplicated before individual fetches.
- Credentialed `valid_login` validation runs lazily only after a per-ID 102, uses the configured
  email plus current credential and auth method, and treats missing or inconclusive proof as fatal.
  Every credentialed 102 gets a fresh proof; no invocation-level proof cache can mask revocation or
  a later validation failure. Anonymous mode carries no credential whose rejection could be
  confused with resource denial.
- Credential proof parses `error` before `result`; `error: true` plus `result: true`, a malformed
  error marker, and redirects are fatal rather than authentication proof.
- Strict REST retrieval cannot switch auth methods after a 401, preserving the provenance needed
  by the lazy validation rule. It also cannot retry a transient failure, keeping the physical
  application-request budget at 201; ordinary client calls retain their retry and alternate-auth
  policies.
- The strict adjacency wire type rejects absent, non-array, negative, or non-integer adjacency
  members. Its retrieval boundary also rejects extra or duplicate batch identities and every
  permissive single response without exactly one identity-matched bug-or-fault outcome. Existing
  `Bug` deserialization remains tolerant for its current callers.
- Only codes 100, 101, and 102 are downgraded, and the mapping is exhaustive and test-pinned.
- Failure records copy only the stable type and numeric code, never server prose.
- Canonical `BTreeMap` storage and explicit adjacency sorting/deduplication remove server-order
  influence.
- Output is buffered until success, so fatal errors cannot create a plausible partial document.

### Explicitly out of scope

The command does not defend against a configured server returning an extremely large single-bug
response. The existing client reads complete response bodies, and truncating adjacency would break
the charter's completeness requirement. General response-byte bounding requires a separate client
policy and server/protocol analysis. The design also does not change alias URL encoding, TLS, or
retry behavior. It deliberately gives adjacency stricter auth and Hybrid routing so required fatal
errors cannot be hidden; ordinary client methods retain their existing alternate-auth and XML-RPC
fallback behavior.

## Verification

### Focused tests

- CLI parsing accepts mixed numeric/alias inputs and rejects missing inputs; a command-level test
  passes an explicit empty argv element and proves an `ids` error before connection.
- Command tests prove one-batch visible numeric retrieval; probes only for omitted numerics;
  resource-code batch fallback; the 101/102 mixed-success schema; code 100 alias failure;
  all-failure success; lazy credentialed 102 validation; successful credentialed Bugzilla 5.0 reads
  without email and without `valid_login`; stale or wrong cached auth cannot turn a credentialed 102
  into `inaccessible`; missing email and inconclusive `valid_login` are fatal; code 410 fatal
  behavior; transport fatal behavior with empty stdout; and exact-input caching.
- A two-restricted-ID command test proves two credentialed code 102 responses make two
  `valid_login` calls; the second proof's failure aborts without stdout even after the first proof
  succeeded.
- Command/output tests prove alias-plus-numeric convergence, one canonical bug, positional request
  identity, numeric node order, and sorted/deduplicated adjacency arrays.
- REST and XML-RPC client tests prove missing adjacency fields and malformed or negative edge
  members are fatal rather than silently shortened or converted to empty arrays.
- REST and XML-RPC tests prove extra and duplicate batch rows plus empty, multiple, mixed, and
  mismatched permissive single outcomes are command-fatal. Numeric bug/fault identities must match
  numerically; aliases may map to a canonical bug ID but fault identities preserve exact alias text.
- Equivalent REST and XML-RPC fixtures serialize byte-equivalent canonical scalar values, including
  empty or missing wire strings normalized to JSON `null`.
- Schema drift tests cover a maximal success result, all nullable scalar keys, and both
  correlated failure variants; invalid type/code combinations and empty strings fail; the registry
  lists `bug-adjacency` in lexical order.
- Transport tests prove strict adjacency does not follow 401 alternate-auth fallback and that the
  ordinary send path still does. With `--retry 10`, 100 classified inputs still produce at most 201
  adjacency/proof attempts; a transient strict response is fatal after its first attempt.
- Same-host redirect tests prove strict REST retrieval, strict XML-RPC retrieval, and credential
  proof each stop at the first redirect response while ordinary callers retain their existing
  bounded same-host redirect behavior. REST and XML-RPC redirect fixtures attach valid-looking
  payload bodies and still produce fatal errors with empty stdout.
- REST, XML-RPC, and Hybrid boundary tests accept `i64::MAX` and reject `i64::MAX + 1` before
  connection with the same `ids` input-validation error.
- REST strict-wire tests reject canonical and adjacency response IDs above `i64::MAX`, matching the
  XML-RPC signed-integer domain and the public schema.
- Hybrid tests prove REST 401, transport/HTTP failure, code 100500, and an empty REST batch never
  invoke XML-RPC; the empty batch proceeds to strict per-ID REST probes.
- Strict response tests prove populated-data batch error envelopes, including code 100/101/102,
  410, and 100500, are fatal before typed row deserialization. Existing callers retain their
  ADR-0015 warning behavior.
- Proof-response tests reject `error: true` with `result: true` and non-boolean `error` values before
  accepting a valid boolean or integer-one result.
- Output and functional assertions pin the additive `SCHEMA_VERSION` bump to `0.6.2` everywhere
  ADR 0007 requires synchronized current-contract documentation or fixtures.
- A controlled-fault test changes one accepted resource code to fatal and must make the focused
  mixed-result test fail before the fault is reverted.

### Functional matrix

Extend functional phases used by every supported container version. Create two related public bugs
and one restricted bug, then assert:

1. numeric IDs and an alias resolve in one invocation;
2. alias plus its numeric ID produce two request entries and one canonical bug;
3. both adjacency arrays contain the complete expected IDs in numeric order;
4. a missing numeric ID is a typed `not_found` request with code 101;
5. the restricted bug is a typed `inaccessible` request with code 102 under the credentialless
   server path;
6. the mixed operation exits zero; and
7. an unsupported/auth-flavored API error remains covered as command-fatal in the focused command
   suite because the stock functional servers do not provide a safe way to synthesize it.

Before those cases run, each version entrypoint enables Bugzilla's `usebugaliases` parameter in its
generated `data/params.json` or `data/params` file before Apache starts. The phase creates a bug with
an alias and first uses existing `bug view <alias>` behavior to prove the server persisted and
resolves it; a skipped, silently ignored, or unresolved alias fails before adjacency assertions.

Before phase 08e grants membership, add `restricted-rest` and `restricted-xmlrpc` server aliases
using the non-member's configured email and API key with explicit API modes. Invoke
`bug adjacency` on the restricted bug through both and assert typed code 102; that result is possible
only after each path's live `valid_login` proof. The focused two-102 request-count test pins one proof
per response. Reuse phase 18d's anonymous explicit REST and XML-RPC aliases to run public-success,
missing-ID, and credentialless-inaccessible cases through both transports on each supported
container version. Run the installed dependency-analysis collector against the newly built binary
after the `0.6.2` bump so its accepted version is exercised live rather than only through replay
fixtures.

Run `make lint`, `make test`, and `make functional-test-all` before delivery. The host is arm64;
the declared project targets are x86_64/aarch64 Linux, powerpc64le Linux, s390x Linux,
aarch64 macOS, and x86_64/aarch64 Windows. The host differs from part of the target matrix; CI and
the existing cross-build configuration remain responsible for non-host compilation.
