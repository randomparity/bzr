# User/group wire conformance design

Decision: [ADR 0038](../../adr/0038-normalize-user-group-wire-contracts.md)

## Scope and goal

Issue #625 corrects four related Bugzilla user/group boundary defects in one resource entry:

1. `group list-users` must send the recognized `groups=<name>` query parameter and exclude the
   enabled non-member fixture.
2. `user update --real-name` must send `full_name` and succeed on Bugzilla 5.2.
3. Every issue-listed user/group/create/auth ID and boolean must accept its documented production
   alternate shape through ADR 0033's shared adapters.
4. `whoami` guidance and functional behavior must identify Bugzilla 5.3+/BMO-derived servers as
   the native endpoint arm and 5.0/5.2 as the email-backed fallback arm.

The source charter excludes disabled-user visibility changes, #634's generalized proxy registry,
schema/version changes, unrelated resources, merging, and the campaign-owned ADR index.

## Architecture

Normalization happens where untrusted JSON enters a typed response. The existing public Rust
types stay `u64`, `Option<u64>`, and `Option<bool>`, and their serialized JSON stays numeric and
boolean. Module-local serde functions provide contextual diagnostics while calling
`types::deserialization::{u64_from_number_or_string, option_bool_from_int_or_bool}` for the
accepted domains.

Request fixes stay at their current client owners: `client/resources/group.rs` chooses the group
query key, and `client/resources/user.rs` maps the command-domain update object into a private wire
request. No command-layer compatibility alias is needed.

The existing Python production-shape proxy transforms only successful responses on the named
user/group routes. Its handler passes the HTTP method and path to one transformation function so
create responses can be distinguished from reads without a registry abstraction. Each non-empty
transformation emits a route/count record that functional assertions use as proof that a green
command actually consumed the alternate shape.

## Wire mappings

| Rust field | Accepted input | Serialized output |
|---|---|---|
| `BugzillaUser.id` | unsigned JSON number or decimal string | number |
| `UserGroup.id` | absent/null, unsigned number, or decimal string | number/null |
| `WhoamiResponse.id` | unsigned number or decimal string | number |
| `GroupInfo.id` | unsigned number or decimal string | number |
| `GroupMember.id` | unsigned number or decimal string | number |
| `IdResponse.id` | unsigned number or decimal string | private response only |
| `WhoamiProbeResponse.id` | unsigned number or decimal string | private response only |
| `BugzillaUser.can_login` | absent/null, bool, or integer `0`/`1` | bool/null |
| `GroupInfo.is_active` | absent/null, bool, or integer `0`/`1` | bool/null |

Malformed values remain errors. Zero remains accepted because the existing typed fields accept
zero; the auth probe continues to interpret zero as unauthenticated after deserialization.

## Request behavior

`get_group_members` retains `match=*` for Bugzilla 5.0 compatibility and replaces only
`group=<name>` with `groups=<name>`. It does not send `include_disabled`, so disabled users retain
the command's existing exclusion behavior.

`BugzillaClient::update_user` converts `UpdateUserParams` into a private borrowed request whose
wire member is named `full_name`. `UpdateUserParams` itself is unchanged, so command JSON input,
dry-run output, and public response output continue to say `real_name`; only the Bugzilla request
body changes.

## `whoami` behavior and guidance

Native `/rest/whoami` exists on the repository's Bugzilla 5.3/master container and BMO-derived
servers, but not on the 5.0 or 5.2 containers. Code comments, the missing-email error, and the CLI
reference must say 5.3+/BMO versus 5.0/5.2 rather than 5.1+ versus 5.0.

The missing-email diagnostic is valid for both connection forms: configure `--email` through
`bzr config set-server` for a named server, or add `--server-email` to an inline `--server-url`
invocation. The README and compiled `content/skills/bzr-setup/SKILL.md` carry the same version split
and recovery flags; they are direct guidance dependencies, not new behavior.

Functional proof runs credentialed inline `whoami` through the shape proxy. The proxy log must
show a transformed native `whoami` response on bz53 and a transformed `/rest/user` response on
bz50/bz52. The result must expose a numeric JSON ID in both arms. Existing credentialless
`whoami` rejection remains unchanged.

## Production-shape proof

The proxy transformation covers:

- GET `/rest/whoami`: string ID;
- GET `/rest/user`: string user IDs, integer `can_login`, and string nested group IDs;
- GET `/rest/group`: string group and membership IDs plus integer `is_active`;
- POST `/rest/user` and POST `/rest/group`: string create-result IDs.

Proxy self-tests pin matching routes, transformed fields, unchanged unrelated payloads, and the
count returned for logging. Functional phase 02 proves native/fallback `whoami`; phase 06 proves
`full_name` on bz52 and both create-result paths through the proxy; phase 07 uses credentialed
inline calls to prove recognized group filtering, user/group response normalization, and the
enabled non-member exclusion. Those calls run after the positive member is added and before it is
removed. The detailed user assertion requires the expected member's `can_login` and at least one
nested group ID to be present before checking their normalized types; the group-detail assertion
likewise requires at least one membership row before checking every member ID. These non-empty
checks prevent optional or collection-valued fields from making the adapter proof vacuous. A
separate credentialless phase-07 call asserts the stock server's access-denied response, covering
the anonymous command path without claiming stock Bugzilla returns user data.
A bz53 group-detail arm may skip because stock 5.3 rejects REST Group.get and the client correctly
falls back to XML-RPC, which the JSON response-shape proxy does not transform.

The controlled-fault record is produced before the Rust/request fixes: the corrected wiremock
query/body expectations and functional assertions must fail with the old implementation, while
the proxy-shaped commands must fail deserialization after their server requests succeed.

## Error handling

The shared adapters retain their existing error domains and messages supplied by local wrappers.
Malformed response values return the existing deserialize error (exit 8). HTTP, API, and mutation
errors follow the existing client pipeline. The proxy returns 502 on malformed JSON or a failed
backend, and applies no transformation to unsuccessful responses.

## Threat model

### Boundary inventory

- Existing widened boundary: a Bugzilla server or reverse proxy controls JSON response values for
  user, group, create, and auth endpoints.
- Existing unchanged boundary: CLI users control group names, user update values, server URLs, and
  credentials through the current validated command/config paths.
- No new entry point, credential source, network destination, or permission is added.

### Actors and trust

The remote Bugzilla deployment is trusted to authorize operations and identify resources, but its
serializer is not trusted to choose one JSON scalar representation. A local operator is trusted
to choose the server and supply credentials. Anonymous callers remain limited to commands already
classified as credentialless, and stock Bugzilla may still deny a particular anonymous read.

### Controls

- ID normalization accepts only unsigned integers and decimal strings that fit `u64`; all other
  shapes fail closed.
- Boolean normalization accepts only booleans and integer `0`/`1`; all other values fail closed.
- Existing request authentication, URL construction, TLS policy, timeouts, redaction, and API
  error parsing remain the controls for the network boundary.
- Route-specific proxy matching prevents a test transformation from silently changing unrelated
  responses; route/count logs prove the desired transform ran.
- The group filter remains server-side and uses Bugzilla's recognized parameter; tests include a
  positive member and enabled negative control. Credentialed calls prove successful filtering;
  a separate credentialless call proves the stock-server denial path.

### Explicitly out of scope

This design does not make a malicious server trustworthy, expand authorization checks, validate
that a returned ID names the newly created object, or expose disabled users. Those threats are
unchanged and are not required to correct representation and parameter conformance.

## Compatibility and documentation

No CLI flag, response key, schema file, or schema version changes. `SCHEMA_VERSION` remains
`2.0.1`. The only visible behavior changes are correct group filtering, successful real-name
updates, tolerance for valid alternate response shapes, and corrected version guidance.

## Verification

- Focused Rust tests for group query parameters, update serialization, every annotated ID/boolean,
  create IDs, auth-probe IDs, and all version-guidance surfaces.
- `python3 tests/functional/redhat-shape-proxy.py --self-test`.
- Controlled red runs before implementation, recorded in the PR body.
- `make test-fast`, `make lint`, `make test`, and `make functional-test-all`.

## Durable workflow state

- Branch: `feat/user-group-wire-conformance-625`
- Base: `main` at `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b`
- Review depth: iterating; cumulative review begins at `0/0`.
- Guardrails: focused `make test-one T=<substring>`; `make test-fast`; `make lint`; `make test`;
  `make functional-test-all`.
- Host/targets: arm64 macOS; seven declared release targets; relationship different.
- ADR index coupling: not coupled; ADR 0038 index row pending for the campaign orchestrator.
- Open findings and deferrals: none.
