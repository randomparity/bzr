# User/group wire conformance implementation plan

**Goal:** Make Bugzilla user/group requests and responses conform across the bz50, bz52, and bz53
functional matrix without changing the CLI or published JSON contract.

**Architecture:** Keep command-domain types and outputs stable. Correct request construction in the
resource clients, normalize alternate JSON scalar shapes at typed response boundaries through ADR
0033's existing adapters, and prove each non-stock shape with explicit transformations in the
existing production-shape proxy.

**Tech stack:** Rust 1.89.0+, serde/serde_json, reqwest, wiremock, Bash functional phases, Python 3
standard-library proxy and unittest.

Expected implementation size: 280–420 changed lines (M) — derived from nine narrow Rust/type test
edits, one explicit proxy transformer with self-tests, and three functional phase extensions.

## Global constraints

- Rust MSRV is exactly 1.89.0; add no dependency or toolchain floor.
- Host is arm64 macOS; declared targets are `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `powerpc64le-unknown-linux-gnu`,
  `s390x-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, and
  `aarch64-pc-windows-msvc`; the architecture relationship is different.
- Reuse `u64_from_number_or_string` and `option_bool_from_int_or_bool`; do not change ADR 0033's
  accepted domains or add a generalized adapter/proxy registry.
- Preserve disabled-user exclusion; do not send `include_disabled=1`.
- Preserve CLI `--real-name`, JSON input `real_name`, dry-run `changes.real_name`, and response
  `real_name`; only the Bugzilla update wire member is `full_name`.
- `SCHEMA_VERSION` stays `2.0.0`; do not edit `schemas/*.json`.
- Do not edit `docs/adr/README.md`; ADR 0038's index row is campaign-owned and pending.
- User-facing changes extend functional phases 02, 06, and 07. Tests live in sibling
  `*_tests.rs` files.
- User output remains routed through `Writers`; this change adds no output path.
- Branch: `feat/user-group-wire-conformance-625`; base: `main` at
  `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b`.
- Guardrails: focused `make test-one T=<substring>`; `make test-fast`; `make lint`; `make test`;
  `make functional-test-all`.
- Review depth is iterating. Open findings and deferrals: none.

## Task 1: Correct group filtering and user-update request fields

**Files**

- Modify: `src/client/resources/group.rs`
- Modify: `src/client/resources/group_tests.rs`
- Modify: `src/client/resources/user.rs`
- Modify: `src/client/resources/user_tests.rs`
- Modify: `src/commands/user/update_tests.rs`
- Modify: `tests/functional/phases/06-users.sh`
- Modify: `tests/functional/phases/07-groups.sh`

**Interfaces**

- Consume `BugzillaClient::get_json_query`, `BugzillaClient::put_json`, and existing
  `UpdateUserParams` fields.
- Add a private `UpdateUserRequest<'a>` in `client/resources/user.rs` with borrowed
  `names: Option<&'a [String]>`, `full_name: Option<&'a str>`, `email: Option<&'a str>`, and
  `login_denied_text: Option<&'a str>`.
- Keep `BugzillaClient::update_user(&self, user: &str, updates: &UpdateUserParams) -> Result<()>`
  unchanged for callers.

### Step 1.1: Make the group wire tests describe Bugzilla's accepted query

Replace all four `query_param("group", ...)` matchers in `group_tests.rs` with
`query_param("groups", ...)`, remove the fulfilled `TODO(#625)` comments, and keep the `match=*`
matchers. Add `.expect(1)` where the existing tests do not already assert request count.

Run:

```bash
make test-one T=get_group_members
```

Expected pre-fix result: failure because no mock matches the singular `group` request. Record the
command and failing matcher in the controlled-fault notes.

### Step 1.2: Add the functional negative control before the production fix

In phase 07, change `group-list-users` to require both the positive member and the enabled
`$NONMEMBER_EMAIL` negative control:

```bash
if assert_success && assert_stdout_contains "testuser@test.bzr" &&
    assert_stdout_not_contains "$NONMEMBER_EMAIL"; then test_pass; fi
```

After `group-remove-user`, assert absence while `testuser@test.bzr` is still enabled, then disable it
only as cleanup for downstream state. Remove the fulfilled `TODO(#625)` comments.

Run the current binary against bz52 with the corrected test:

```bash
make functional-test-bz52
```

Expected pre-fix result: the group-list test fails because the enabled non-member is present. Keep
the exact test ID and summary for the PR body.

### Step 1.3: Fix the group query and prove it green

In `get_group_members`, replace only:

```rust
("group", group_name),
```

with:

```rust
("groups", group_name),
```

Do not add `include_disabled`. Run:

```bash
make test-one T=get_group_members
make functional-test-bz52
```

Expected: focused tests pass; the phase-07 negative control passes while the positive member remains
visible.

### Step 1.4: Make real-name request tests fail on the old wire key

In `client/resources/user_tests.rs`, add a direct update test whose PUT body is exactly:

```json
{"names":["alice@test.bzr"],"full_name":"Alice Updated"}
```

In `commands/user/update_tests.rs`, strengthen the command-path body matcher to assert `full_name`
and absence of `real_name`; keep the dry-run assertion on `changes.real_name` to pin the public
contract.

In phase 06, change the update test to include `--real-name "Test User Updated"`, then read the user
back and assert `.real_name` equals the new value. Retain login disable/reenable coverage as separate
operations so the real-name assertion identifies the failing field.

Run:

```bash
make test-one T=update_user
make functional-test-bz52
```

Expected pre-fix result: the wiremock request does not match `real_name`, and bz52 reports the
unknown setter for the old field. Record both observations.

### Step 1.5: Add the private wire request and prove both contracts

Define:

```rust
#[derive(Serialize)]
struct UpdateUserRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    names: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    login_denied_text: Option<&'a str>,
}
```

Construct it with `updates.names.as_deref()` and each string's `as_deref()`, then pass it to
`put_json`. Do not change `UpdateUserParams` serde names.

Run:

```bash
make test-one T=update_user
make test-one T=user_update_dry_run
make functional-test-bz52
```

Expected: request and command tests pass, dry-run still emits `real_name`, and bz52 updates/readbacks
the real name.

### Step 1.6: Commit the request corrections

Run `git diff --check`, stage only the seven Task 1 files, and commit:

```text
fix(api): correct user and group request fields
```

Rollback is one commit; no persisted format changes.

## Task 2: Apply shared leniency to every listed response field

**Files**

- Modify: `src/types/user.rs`
- Modify: `src/types/user_tests.rs`
- Modify: `src/types/group.rs`
- Modify: `src/types/group_tests.rs`
- Modify: `src/client/mod.rs`
- Modify: `src/client/mod_tests.rs`
- Modify: `src/client/auth/whoami.rs`
- Modify: `src/client/auth/whoami_tests.rs`
- Modify: `src/client/resources/user_tests.rs`
- Modify: `src/client/resources/group_tests.rs`

**Interfaces**

- Consume `crate::types::deserialization::u64_from_number_or_string` and
  `option_bool_from_int_or_bool` unchanged.
- Add module-local `deserialize_*_id` functions with the serde signature
  `fn<'de, D: Deserializer<'de>>(D) -> Result<u64, D::Error>`.
- For `UserGroup.id: Option<u64>`, add a private newtype implementing `Deserialize` through the
  shared unsigned adapter, then deserialize `Option<Newtype>` and map it back to `Option<u64>`.
- Keep every public field type and serializer unchanged.

### Step 2.1: Add failing type and client tests

Extend sibling tests with alternate shapes:

- `BugzillaUser`: string `id`, integer `can_login`, and nested string group `id`;
- `WhoamiResponse`: string `id`;
- `GroupInfo`: string `id`, integer `is_active`, and string membership `id`;
- `IdResponse`: string `id` through a client request test for both user and group create;
- `WhoamiProbeResponse`: string positive ID authenticates and string zero rejects;
- malformed negative/fractional/out-of-range IDs and boolean `2` still return deserialize errors.

Use response fixtures that exercise production structs, not standalone copies. Run:

```bash
make test-one T=deserializes_production_shapes
make test-one T=create_user_returns_string_id
make test-one T=create_group_returns_string_id
make test-one T=whoami_response_string_id
```

Expected pre-fix result: each valid alternate shape fails deserialization. Malformed-control tests
remain green.

### Step 2.2: Add local wrappers backed by the shared adapters

In each owning module, use the shared function in the local serde callback:

```rust
fn deserialize_user_group_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    u64_from_number_or_string(
        deserializer,
        "an unsigned integer or decimal numeric string user/group ID",
        "expected an unsigned integer user/group ID",
    )
}
```

Apply it to all required non-optional IDs. Apply `option_bool_from_int_or_bool` with both
`default` and `deserialize_with` on `can_login` and `is_active`. Implement the optional nested
group ID with `Option<PrivateId>` so null and absence retain `None`.

Run the four focused commands from Step 2.1 plus:

```bash
make test-fast
```

Expected: all focused cases and the unit suite pass; malformed controls still fail closed.

### Step 2.3: Commit the response normalization

Run `git diff --check`, stage only Task 2 files, and commit:

```text
fix(api): accept Bugzilla user and group scalar variants
```

The commit changes deserialization only. Verify `git diff main...HEAD -- schemas src/output/mod.rs`
shows no schema file or version edit.

## Task 3: Prove production shapes and version-aware `whoami` behavior

**Files**

- Modify: `tests/functional/redhat-shape-proxy.py`
- Modify: `tests/functional/phases/02-server-auth.sh`
- Modify: `tests/functional/phases/06-users.sh`
- Modify: `tests/functional/phases/07-groups.sh`
- Modify: `src/client/resources/user.rs`
- Modify: `src/client/resources/user_tests.rs`
- Modify: `src/client/auth/mod.rs`
- Modify: `src/client/auth/mod_tests.rs`
- Modify: `docs/bzr-cli.md`

**Interfaces**

- Add `shape_user_group_response(method, path, data) -> tuple[bytes, str | None, int]` in the
  Python proxy. The route value is `whoami`, `user-read`, `group-read`, `user-create`, or
  `group-create`; unrelated responses return the original bytes, `None`, and zero.
- Reuse `redhat_shape_start`/`redhat_shape_stop` and their log path; add no new process manager.
- Keep `BugzillaClient::whoami` and auth-detection signatures unchanged.

### Step 3.1: Implement and self-test explicit proxy transforms

Transform only successful JSON responses:

- GET `/rest/whoami`: stringify `id`;
- GET `/rest/user` and `/rest/user/...`: stringify user and nested group IDs and map boolean
  `can_login` to `0`/`1`;
- GET `/rest/group` and `/rest/group/...`: stringify group and membership IDs and map boolean
  `is_active` to `0`/`1`;
- POST `/rest/user` and `/rest/group`: stringify the top-level create `id`.

Call the transformer after existing shape functions and emit:

```text
user-group-shaped route=<route> count=<positive integer>
```

Add self-tests for every route, null/absent optional fields, unrelated paths/methods, and malformed
JSON. Run:

```bash
python3 tests/functional/redhat-shape-proxy.py --self-test
```

Expected: all proxy self-tests pass independently of Rust production code.

### Step 3.2: Add functional production-shape arms

In phase 02, run inline credentialed `whoami` through the proxy with the API-key env and email. Pin
numeric output IDs. Inspect `REDHAT_SHAPE_LOG`: bz53 must contain a positive `route=whoami` count;
bz50/bz52 must contain a positive `route=user-read` count and no successful native route. Preserve
the existing anonymous `whoami` rejection.

In phase 06, create per-run-unique user and group resources through the credentialed proxy and
require numeric `.id` output, with positive `user-create` and `group-create` log counts. The user
real-name update stays on stock bz52 as Task 1's server-conformance proof.

In phase 07, run credentialless `group list-users --details` through the proxy and assert:

- the expected member is present;
- `$NONMEMBER_EMAIL` is absent while enabled;
- each returned `id` is numeric and each populated `can_login` is boolean;
- the log contains a positive `user-read` transform.

On bz50/bz52, run credentialless `group view` through the proxy and assert numeric group/member IDs
and boolean `is_active`, plus a positive `group-read` log. On bz53, record a semantic skip because
stock REST Group.get returns 32610 and the client uses XML-RPC, outside this JSON proxy.

Before relying on Task 2's implementation, run the phase additions against the parent commit or a
controlled fault that removes one relevant serde annotation:

```bash
make functional-test-bz52
make functional-test-bz53
```

Expected red proof: shaped IDs/booleans return exit 8 (or auth detection fails) after positive
proxy log records. Restore the production implementation and rerun both commands green.

### Step 3.3: Correct `whoami` guidance and pin it in tests

Change code comments and public guidance from `5.1+`/`5.0` to `5.3+ or a BMO-derived server` versus
`5.0/5.2`. The missing-email runtime error must say:

```text
whoami requires Bugzilla 5.3+ or a BMO-derived server; add --email for Bugzilla 5.0/5.2
```

Update auth-module documentation and the two matching CLI-reference passages. Add wiremock tests
for a missing native endpoint without an email hint and for the auth-detection fallback hint.

Run:

```bash
make test-one T=whoami
```

Expected: guidance tests pass and existing native/fallback behavior remains green.

### Step 3.4: Commit functional proof and guidance

Run `git diff --check`, stage only Task 3 files, and commit:

```text
test(functional): prove user and group production shapes
```

If guidance changes remain a separable diff after tests, commit them first as:

```text
fix(auth): correct whoami version guidance
```

## Task 4: Verify the whole branch and preserve evidence

**Files**

- Verify only; modify a task-owned file only if a current failure identifies its cause.
- Do not edit `docs/adr/README.md`, `schemas/*.json`, or `src/output/mod.rs`.

**Interfaces**

- Consume repository guardrails exactly as recorded; produce no new interface.

### Step 4.1: Verify schema/version non-change

Run:

```bash
git diff --exit-code main...HEAD -- schemas src/output/mod.rs
```

Expected: exit 0 and no output.

### Step 4.2: Run required local guardrails

Run bare commands, preserving exit codes:

```bash
make test-fast
make lint
make test
```

Expected: each exits 0 with zero warnings and zero failed tests. Record observed durations and test
counts.

### Step 4.3: Run the full real-server matrix

Confirm Docker or podman is available, then run:

```bash
make functional-test-all
```

Expected: bz50, bz52, and bz53 all report zero failures. Record pass/fail/skip counts per version,
including the intentional bz53 Group.get production-shape skip.

### Step 4.4: Audit the final diff and commit any evidence-backed correction

Run:

```bash
git status --short --untracked-files=all
git diff --check main...HEAD
git diff --stat main...HEAD
```

Confirm every changed file is in the frozen surface, all `TODO(#625)` markers in touched fixtures
are resolved, controlled-fault observations are retained for the PR body, and no schema/version or
ADR-index edit exists. Any correction gets its own conventional commit after its focused guardrail.

