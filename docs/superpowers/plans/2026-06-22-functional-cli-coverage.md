# Functional CLI Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand the functional suite so every new post-0.5.0 command and option is exercised against the real Bugzilla Docker containers with real persisted server data.

**Architecture:** Keep the existing shell-based phase runner. Add focused fixtures and assertions to the existing phase files, plus one new bug-update-from-json phase, so coverage follows the CLI's domain areas without a second test harness. TLS functional coverage is explicitly out of scope and deferred to GitHub issue #406.

**Tech Stack:** Bash functional tests, Docker/Podman Bugzilla 5.0/5.2/5.3 containers, `bzr` release/debug binary, `jq`, `curl`, `shellcheck`, `shfmt`, Rust `cargo test`.

---

## Scope

In scope:

- Credentialless named and inline HTTP server reads against the running Bugzilla container.
- Write and identity-command credential checks against real configured server entries.
- Product, component, user, and group create/update `--from-json` and `--dry-run`.
- `bug update --from-json`, `--url`, `--target-milestone`, richer `bug my` filters, richer templates, richer clones, and convenience verb `--expect-unchanged-since`.
- `query update --from-url` and `query run --count`.
- Attachment/comment body-source and no-op paths: `attachment upload --comment-file`, stdin comment forms, `attachment download --out -`, no-op `attachment update`, and no-op `comment tag`.
- Schema list coverage for every published input schema.
- Functional README refresh.

Out of scope:

- Functional tests for `--server-tls-insecure`, `--server-tls-ca-cert`,
  `--server-tls-pin-sha256`, and `--server-tls-pin-now`.
- HTTPS proxy/certificate fixture work. That follow-up is tracked in
  https://github.com/randomparity/bzr/issues/406.

## Files

- Modify `tests/functional/lib.sh`: shared helpers for unique fixture names,
  exact stdout/file comparison, Docker/Podman SQL execution, JSON fixture writes,
  and schema-list assertions.
- Modify `tests/functional/run-tests.sh`: source the new
  `08d-bug-update-from-json` phase.
- Create `tests/functional/phases/08d-bug-update-from-json.sh`: structured bug
  update functional tests.
- Modify `tests/functional/phases/01-config.sh`: credentialless named server
  config.
- Modify `tests/functional/phases/02-server-auth.sh`: named and inline
  credentialless HTTP read/write-boundary tests.
- Modify `tests/functional/phases/03-products.sh`: product `--from-json` and
  `--dry-run` tests.
- Modify `tests/functional/phases/04-components.sh`: component `--from-json`,
  named-target update, and `--dry-run` tests.
- Modify `tests/functional/phases/06-users.sh`: user `--from-json` and
  `--dry-run` tests.
- Modify `tests/functional/phases/07-groups.sh`: group `--from-json`,
  `--dry-run`, and product group-control fixture setup.
- Modify `tests/functional/phases/08-bugs.sh`: `bug update --url` and
  `--target-milestone` readback.
- Modify `tests/functional/phases/08c-bugs-create-fields.sh`: remaining create
  metadata, flag, and group coverage.
- Modify `tests/functional/phases/10-bug-clone.sh`: clone metadata override
  and inherited metadata readback.
- Modify `tests/functional/phases/11b-bug-verbs.sh`: convenience verb
  `--expect-unchanged-since`.
- Modify `tests/functional/phases/12-my-bugs.sh`: richer `bug my` filters.
- Modify `tests/functional/phases/13-templates.sh`: template metadata storage,
  clear, and create-from-template readback.
- Modify `tests/functional/phases/14-queries.sh`: `query update --from-url` and
  `query run --count`.
- Modify `tests/functional/phases/16-attachments.sh`: attachment comment-file,
  stdin comment, stdout download, content-type update, flag update, and no-op
  rejection.
- Modify `tests/functional/phases/17-global-options.sh`: dry-run coverage for
  supported non-bug mutation families.
- Modify `tests/functional/phases/17b-arg-validation.sh`: command conflict and
  no-op rejection coverage.
- Modify `tests/functional/phases/18-completion-schema.sh`: all input schema
  names.
- Modify `tests/functional/README.md`: current phase list, real-server fixture
  data, cross-version behavior, and explicit TLS deferral.

### Task 1: Branch Baseline and Coverage Ledger

**Files:**

- Modify: `docs/superpowers/plans/2026-06-22-functional-cli-coverage.md`

- [ ] **Step 1: Confirm branch and clean tree**

Run:

```bash
git status --short --branch
```

Expected: branch is not `main` or `master`; no uncommitted changes except this
plan if the plan is still being edited.

- [ ] **Step 2: Run the current single-version baseline**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh start
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
```

Expected: existing tests pass or any failure is unrelated and recorded before
adding coverage. Do not start writing new assertions on top of an unexplained
baseline failure.

- [ ] **Step 3: Record the coverage target in the implementation PR**

Use this checklist in the PR body and update it as tasks land:

```markdown
Functional coverage added:

- [ ] Credentialless named and inline HTTP server reads
- [ ] Credential failure boundaries for writes and identity commands
- [ ] Admin `--from-json`
- [ ] Admin `--dry-run`
- [ ] `bug update --from-json`
- [ ] Bug URL and target-milestone update/readback
- [ ] `bug my` shared filters
- [ ] Template metadata
- [ ] Clone metadata
- [ ] Query `--from-url` update and run `--count`
- [ ] Attachment/comment source and stdout download paths
- [ ] No-op rejection paths
- [ ] Input schema list coverage
- [ ] Functional README refresh

Deferred:

- TLS functional coverage: #406
```

- [ ] **Step 4: Commit the planning artifact if desired**

Run:

```bash
git add -f docs/superpowers/plans/2026-06-22-functional-cli-coverage.md
git commit -m "docs: plan functional CLI coverage"
```

Expected: one docs-only commit. If the branch owner does not want a plan-only
commit, skip this step and keep the plan staged for the first implementation
commit.

### Task 2: Shared Functional Helpers and Real-Server Fixtures

**Files:**

- Modify: `tests/functional/lib.sh`

- [ ] **Step 1: Add helpers to `tests/functional/lib.sh`**

Append these helpers after `wait_for_changed`:

```bash
# unique_name <prefix> — per-run fixture id safe for Bugzilla names.
unique_name() {
    local prefix="$1"
    printf '%s-%s-%s' "$prefix" "$$" "$RANDOM"
    return 0
}

# write_json_fixture <path> <json> — writes compact JSON without a trailing
# shell-expanded newline surprise.
write_json_fixture() {
    local path="$1"
    local json="$2"
    printf '%s' "$json" >"$path"
    return 0
}

# assert_stdout_equals_file <path> — raw stdout exactly matches file bytes.
assert_stdout_equals_file() {
    local path="$1"
    if ! cmp -s "$BZR_STDOUT" "$path"; then
        test_fail "stdout does not exactly match '$path'"
        return 1
    fi
}

# assert_schema_list_contains <name> — schema list stdout contains a schema name.
assert_schema_list_contains() {
    local name="$1"
    assert_json_exists "index(\"$name\")"
}

container_runtime() {
    if command -v podman >/dev/null 2>&1; then
        printf '%s' podman
        return 0
    fi
    if command -v docker >/dev/null 2>&1; then
        printf '%s' docker
        return 0
    fi
    return 1
}

bugzilla_container_name() {
    printf '%s' "${BZR_FUNC_CONTAINER:-bzr-func-test-${BZ_VERSION}}"
    return 0
}

# run_bugzilla_sql_file <path> — execute SQL inside the running Bugzilla
# container. Use this only for fixture capabilities that Bugzilla's public API
# cannot create, such as flag types and product group controls.
run_bugzilla_sql_file() {
    local sql_file="$1"
    local runtime
    local container
    runtime=$(container_runtime)
    container=$(bugzilla_container_name)
    "$runtime" exec -i "$container" mysql -u root bugs <"$sql_file"
}
```

- [ ] **Step 2: Run shell lint for the helper edit**

Run:

```bash
shellcheck tests/functional/lib.sh
shfmt -d tests/functional/lib.sh
```

Expected: both commands exit 0. If `shfmt -d` prints a diff, apply that format
with `shfmt -w tests/functional/lib.sh`.

- [ ] **Step 3: Commit**

Run:

```bash
git add tests/functional/lib.sh
git commit -m "test: add functional fixture helpers"
```

Expected: one commit containing only helper changes.

### Task 3: Seed Flag and Group Capabilities in the Real Containers

**Files:**

- Modify: `tests/functional/phases/02-server-auth.sh`
- Modify: `tests/functional/phases/07-groups.sh`

- [ ] **Step 1: Add global flag types after server/auth checks**

In `tests/functional/phases/02-server-auth.sh`, before the final `echo ""`,
insert:

```bash
test_begin "8b. fixture flag types exist"
_FLAG_SQL=$(mktemp /tmp/bzr-func-flags.XXXXXX.sql)
cat >"$_FLAG_SQL" <<'SQL'
INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'review', 'Functional test review flag for bugs', 'b', 1, 1, 1, 1, 10
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'review' AND target_type = 'b'
);

INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'review', 'Functional test review flag for attachments', 'a', 1, 1, 1, 1, 10
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'review' AND target_type = 'a'
);

INSERT INTO flaginclusions (type_id, product_id, component_id)
SELECT id, NULL, NULL
FROM flagtypes
WHERE name = 'review'
  AND target_type IN ('b', 'a')
  AND NOT EXISTS (
      SELECT 1 FROM flaginclusions WHERE flaginclusions.type_id = flagtypes.id
  );
SQL
if run_bugzilla_sql_file "$_FLAG_SQL"; then
    test_pass
else
    test_fail "could not seed functional flag types"
fi
rm -f "$_FLAG_SQL"
unset _FLAG_SQL
```

- [ ] **Step 2: Add product group-control setup after group creation**

In `tests/functional/phases/07-groups.sh`, after test `27. group update
functest-grp`, insert:

```bash
test_begin "27a. fixture group enabled for FuncTestProd bugs"
_GROUP_SQL=$(mktemp /tmp/bzr-func-group-control.XXXXXX.sql)
cat >"$_GROUP_SQL" <<'SQL'
INSERT INTO group_control_map
    (group_id, product_id, entry, membercontrol, othercontrol, canedit,
     editcomponents, editbugs, canconfirm)
SELECT g.id, p.id, 0, 1, 1, 1, 0, 1, 1
FROM groups AS g
JOIN products AS p ON p.name = 'FuncTestProd'
WHERE g.name = 'functest-grp'
ON DUPLICATE KEY UPDATE
    membercontrol = 1,
    othercontrol = 1,
    canedit = 1,
    editbugs = 1,
    canconfirm = 1;
SQL
if run_bugzilla_sql_file "$_GROUP_SQL"; then
    test_pass
else
    test_fail "could not enable functest-grp for FuncTestProd"
fi
rm -f "$_GROUP_SQL"
unset _GROUP_SQL
```

- [ ] **Step 3: Verify fixture capability on bz50**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
```

Expected: the new fixture tests pass and existing phases still pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add tests/functional/phases/02-server-auth.sh tests/functional/phases/07-groups.sh
git commit -m "test: seed functional Bugzilla flag fixtures"
```

Expected: one commit containing only fixture-capability setup.

### Task 4: Credentialless HTTP Server Coverage

**Files:**

- Modify: `tests/functional/phases/01-config.sh`
- Modify: `tests/functional/phases/02-server-auth.sh`
- Modify: `tests/functional/phases/08-bugs.sh`
- Modify: `tests/functional/phases/17b-arg-validation.sh`

- [ ] **Step 1: Add a credentialless named server**

In `tests/functional/phases/01-config.sh`, after test `3a. config set-server
auto-detect`, insert:

```bash
test_begin "3b. config set-server public without credentials"
run_bzr config set-server public --url "$BZ_URL"
if assert_success; then
    run_bzr config show
    if assert_json '.servers.public.url' "$BZ_URL" &&
        assert_json '.servers.public.api_key_source' "none"; then test_pass; fi
fi
```

- [ ] **Step 2: Add named credentialless read and write-boundary tests**

In `tests/functional/phases/02-server-auth.sh`, after test `8a. --server auto
whoami`, insert:

```bash
test_begin "8c. credentialless named server info"
run_bzr_raw --json --server public server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "8d. credentialless named whoami fails before network auth"
run_bzr_raw --json --server public whoami
if assert_exit_code 3 && assert_stderr_contains "requires credentials"; then test_pass; fi

test_begin "8e. credentialless named write fails before mutation"
run_bzr_raw --json --server public bug create \
    --product FuncTestProd --component Backend --summary "public write" \
    --description "should not write" --op-sys Linux --rep-platform PC
if assert_exit_code 3 && assert_stderr_contains "requires credentials"; then test_pass; fi

test_begin "8f. inline credentialless server info"
run_bzr_raw --json --server-url "$BZ_URL" server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "8g. inline credentialed whoami"
export BZR_FUNC_INLINE_KEY="$API_KEY"
run_bzr_raw --json --server-url "$BZ_URL" \
    --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi
unset BZR_FUNC_INLINE_KEY
```

- [ ] **Step 3: Add credentialless reads of real bug data after fixtures exist**

In `tests/functional/phases/08-bugs.sh`, after test `35. bug view`, insert:

```bash
test_begin "35a. credentialless named bug view"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --json --server public bug view "$BUG1"
    if assert_success && assert_json '.summary' "Bug one"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "35b. inline credentialless bug list"
run_bzr_raw --json --server-url "$BZ_URL" bug list --product FuncTestProd --limit 1
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
```

These checks intentionally live in phase 8, after `FuncTestProd` and `BUG1`
exist, so anonymous reads prove real persisted Bugzilla data is visible without
credentials.

- [ ] **Step 4: Add parse-level credential/TLS boundary checks without TLS I/O**

In `tests/functional/phases/17b-arg-validation.sh`, after test `125.
--server-url + --server conflict`, insert:

```bash
test_begin "125a. --server-api-key-env requires --server-url"
run_bzr --server-api-key-env BZR_FUNC_INLINE_KEY server info
if assert_exit_code 2 && assert_stderr_contains "required"; then test_pass; fi

test_begin "125b. --server-tls-insecure requires --server-url"
run_bzr --server-tls-insecure server info
if assert_exit_code 2 && assert_stderr_contains "required"; then test_pass; fi

test_begin "125c. ad-hoc TLS flags are mutually exclusive"
run_bzr --server-url "$BZ_URL" --server-tls-insecure \
    --server-tls-pin-now server info
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi
```

This covers the CLI boundary for TLS flags while keeping HTTPS behavior out of
scope for this plan.

- [ ] **Step 5: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/phases/01-config.sh tests/functional/phases/02-server-auth.sh tests/functional/phases/08-bugs.sh tests/functional/phases/17b-arg-validation.sh
shfmt -d tests/functional/phases/01-config.sh tests/functional/phases/02-server-auth.sh tests/functional/phases/08-bugs.sh tests/functional/phases/17b-arg-validation.sh
```

Expected: functional run passes, shellcheck exits 0, and shfmt prints no diff.

Commit:

```bash
git add tests/functional/phases/01-config.sh tests/functional/phases/02-server-auth.sh tests/functional/phases/08-bugs.sh tests/functional/phases/17b-arg-validation.sh
git commit -m "test: cover credentialless server functional paths"
```

### Task 5: Admin `--from-json` and `--dry-run`

**Files:**

- Modify: `tests/functional/phases/03-products.sh`
- Modify: `tests/functional/phases/04-components.sh`
- Modify: `tests/functional/phases/06-users.sh`
- Modify: `tests/functional/phases/07-groups.sh`
- Modify: `tests/functional/phases/17-global-options.sh`

- [ ] **Step 1: Product structured input**

Append to `tests/functional/phases/03-products.sh` before `unset _PV`:

```bash
_PJSON_DIR=$(mktemp -d /tmp/bzr-func-product-json.XXXXXX)
_PJ_NAME=$(unique_name prodjson)
write_json_fixture "$_PJSON_DIR/create.json" \
    "{\"name\":\"$_PJ_NAME\",\"description\":\"product json\",\"version\":\"8.8\",\"is_open\":true}"
write_json_fixture "$_PJSON_DIR/update.json" \
    "{\"name\":\"$_PJ_NAME\",\"description\":\"product json updated\",\"is_open\":false}"
write_json_fixture "$_PJSON_DIR/bad.json" \
    "{\"name\":\"bad\",\"description\":\"bad\",\"unknown\":true}"

test_begin "13c. product create --from-json"
run_bzr product create --from-json "$_PJSON_DIR/create.json"
if assert_success; then
    run_bzr product view "$_PJ_NAME"
    if assert_json '.name' "$_PJ_NAME" &&
        assert_json_contains '[.versions[].name] | join(",")' "8.8"; then test_pass; fi
fi

test_begin "13d. product update --from-json"
run_bzr product update --from-json "$_PJSON_DIR/update.json"
if assert_success; then
    run_bzr product view "$_PJ_NAME"
    if assert_json '.is_active' "false"; then test_pass; fi
fi

test_begin "13e. product create --from-json unknown key"
run_bzr product create --from-json "$_PJSON_DIR/bad.json"
if assert_exit_code 7 && assert_stderr_contains "unknown field"; then test_pass; fi

test_begin "13f. product create --from-json CLI override"
_PJ_OVERRIDE=$(unique_name prodjson-override)
run_bzr product create --from-json "$_PJSON_DIR/create.json" --name "$_PJ_OVERRIDE"
if assert_success; then
    run_bzr product view "$_PJ_OVERRIDE"
    if assert_json '.name' "$_PJ_OVERRIDE"; then test_pass; fi
fi

rm -r "$_PJSON_DIR"
unset _PJSON_DIR _PJ_NAME _PJ_OVERRIDE
```

- [ ] **Step 2: Component structured input and named-target update**

Append to `tests/functional/phases/04-components.sh` before `echo ""`:

```bash
_CJSON_DIR=$(mktemp -d /tmp/bzr-func-component-json.XXXXXX)
_CJ_NAME=$(unique_name compjson)
write_json_fixture "$_CJSON_DIR/create.json" \
    "{\"product\":\"FuncTestProd\",\"name\":\"$_CJ_NAME\",\"description\":\"component json\",\"default_assignee\":\"$ADMIN_EMAIL\"}"
write_json_fixture "$_CJSON_DIR/update-by-name.json" \
    "{\"product\":\"FuncTestProd\",\"component\":\"$_CJ_NAME\",\"description\":\"component json updated\"}"

test_begin "15c. component create --from-json"
run_bzr component create --from-json "$_CJSON_DIR/create.json"
if assert_success; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.name' "$_CJ_NAME"; then test_pass; fi
fi

test_begin "15d. component update --product --component target"
run_bzr component update --product FuncTestProd --component "$_CJ_NAME" \
    --description "component named target updated"
if [[ $BZR_EXIT -eq 0 ]]; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.description' "component named target updated"; then test_pass; fi
elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
    test_skip "component update REST endpoint not available"
else
    assert_success
fi

test_begin "15e. component update --from-json named target"
run_bzr component update --from-json "$_CJSON_DIR/update-by-name.json"
if [[ $BZR_EXIT -eq 0 ]]; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.description' "component json updated"; then test_pass; fi
elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
    test_skip "component update REST endpoint not available"
else
    assert_success
fi

rm -r "$_CJSON_DIR"
unset _CJSON_DIR _CJ_NAME
```

- [ ] **Step 3: User structured input**

Append to `tests/functional/phases/06-users.sh` before `echo ""`:

```bash
_UJSON_DIR=$(mktemp -d /tmp/bzr-func-user-json.XXXXXX)
_UJ_LOGIN="$(unique_name userjson)@test.bzr"
write_json_fixture "$_UJSON_DIR/create.json" \
    "{\"email\":\"$_UJ_LOGIN\",\"full_name\":\"User Json\",\"password\":\"TestPass1!\"}"
write_json_fixture "$_UJSON_DIR/update.json" \
    "{\"user\":\"$_UJ_LOGIN\",\"disable_login\":true,\"login_denied_text\":\"json disabled\"}"

test_begin "24a. user create --from-json"
run_bzr user create --from-json "$_UJSON_DIR/create.json"
if assert_success; then
    run_bzr user search "$_UJ_LOGIN" --details
    if assert_stdout_contains "$_UJ_LOGIN"; then test_pass; fi
fi

test_begin "24b. user update --from-json"
run_bzr user update --from-json "$_UJSON_DIR/update.json"
if assert_success; then
    run_bzr user search "$_UJ_LOGIN" --details
    if assert_success &&
        assert_json "[.[] | select(.name == \"$_UJ_LOGIN\")][0].can_login" \
            "false"; then test_pass; fi
fi

rm -r "$_UJSON_DIR"
unset _UJSON_DIR _UJ_LOGIN
```

- [ ] **Step 4: Group structured input**

Append to `tests/functional/phases/07-groups.sh` before `echo ""`:

```bash
_GJSON_DIR=$(mktemp -d /tmp/bzr-func-group-json.XXXXXX)
_GJ_NAME=$(unique_name groupjson)
write_json_fixture "$_GJSON_DIR/create.json" \
    "{\"name\":\"$_GJ_NAME\",\"description\":\"group json\",\"is_active\":true}"
write_json_fixture "$_GJSON_DIR/update.json" \
    "{\"group\":\"$_GJ_NAME\",\"description\":\"group json updated\",\"is_active\":false}"

test_begin "32a. group create --from-json"
run_bzr group create --from-json "$_GJSON_DIR/create.json"
if assert_success; then
    run_bzr group view "$_GJ_NAME"
    if assert_json '.name' "$_GJ_NAME"; then test_pass; fi
fi

test_begin "32b. group update --from-json"
run_bzr group update --from-json "$_GJSON_DIR/update.json"
if assert_success; then
    run_bzr group view "$_GJ_NAME"
    if assert_json '.description' "group json updated"; then test_pass; fi
fi

rm -r "$_GJSON_DIR"
unset _GJSON_DIR _GJ_NAME
```

- [ ] **Step 5: Admin dry-run no-write checks**

Append to `tests/functional/phases/17-global-options.sh` after the existing
`--dry-run bug create` test:

```bash
test_begin "103d. --dry-run product create previews without writing"
_DP=$(unique_name dryprod)
run_bzr --dry-run product create --name "$_DP" --description "dry product"
if assert_success && assert_json '.resource' "product" && assert_json '.action' "dry-run"; then
    run_bzr product view "$_DP"
    if assert_failure; then test_pass; fi
fi
unset _DP

test_begin "103e. --dry-run component update by name resolves but does not write"
run_bzr --dry-run component update --product FuncTestProd --component Backend \
    --description "dry component update"
if [[ $BZR_EXIT -eq 0 ]]; then
    run_bzr component view FuncTestProd Backend
    if assert_stdout_not_contains "dry component update"; then test_pass; fi
elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
    test_skip "component update REST endpoint not available"
else
    assert_success
fi

test_begin "103f. --dry-run user update previews without writing"
run_bzr --dry-run user update testuser@test.bzr --disable-login false
if assert_success && assert_json '.resource' "user" && assert_json '.action' "dry-run"; then
    test_pass
fi

test_begin "103g. --dry-run group update previews without writing"
run_bzr --dry-run group update functest-grp --description "dry group update"
if assert_success && assert_json '.resource' "group" && assert_json '.action' "dry-run"; then
    run_bzr group view functest-grp
    if assert_stdout_not_contains "dry group update"; then test_pass; fi
fi
```

- [ ] **Step 6: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/phases/03-products.sh tests/functional/phases/04-components.sh tests/functional/phases/06-users.sh tests/functional/phases/07-groups.sh tests/functional/phases/17-global-options.sh
shfmt -d tests/functional/phases/03-products.sh tests/functional/phases/04-components.sh tests/functional/phases/06-users.sh tests/functional/phases/07-groups.sh tests/functional/phases/17-global-options.sh
```

Commit:

```bash
git add tests/functional/phases/03-products.sh tests/functional/phases/04-components.sh tests/functional/phases/06-users.sh tests/functional/phases/07-groups.sh tests/functional/phases/17-global-options.sh
git commit -m "test: cover admin structured input and dry runs"
```

### Task 6: Bug Update Fields and Structured Input

**Files:**

- Modify: `tests/functional/run-tests.sh`
- Modify: `tests/functional/phases/08-bugs.sh`
- Create: `tests/functional/phases/08d-bug-update-from-json.sh`

- [ ] **Step 1: Add URL and target-milestone update readback**

In `tests/functional/phases/08-bugs.sh`, after test `42a. bug update
--deadline`, insert:

```bash
test_begin "42c. bug update --url and --target-milestone"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --url "http://example.com/updated-$BUG1" \
        --target-milestone "---"
    if assert_success; then
        run_bzr bug view "$BUG1"
        if assert_json '.url' "http://example.com/updated-$BUG1" &&
            assert_json '.target_milestone' "---"; then test_pass; fi
    fi
else test_skip "no BUG1"; fi
```

- [ ] **Step 2: Add the new phase to `run-tests.sh`**

In `tests/functional/run-tests.sh`, change the phase list segment:

```bash
08-bugs 08b-bugs-paging 08c-bugs-create-fields \
09-bug-relationships
```

to:

```bash
08-bugs 08b-bugs-paging 08c-bugs-create-fields 08d-bug-update-from-json \
09-bug-relationships
```

- [ ] **Step 3: Create `08d-bug-update-from-json.sh`**

Create `tests/functional/phases/08d-bug-update-from-json.sh` with:

```bash
# 08d-bug-update-from-json
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

echo "── Phase 8d: Bug update --from-json ────────────────────────"

_UJ_DIR=$(mktemp -d /tmp/bzr-func-bug-update-json.XXXXXX)
_UJ_CREATE=(--product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d)

test_begin "152. bug update --from-json object with positional ID"
_UJ_ONE=$(make_bug "${_UJ_CREATE[@]}" --summary "update json object")
write_json_fixture "$_UJ_DIR/object.json" \
    '{"priority":"High","whiteboard":"json-object","url":"http://example.com/json-object"}'
run_bzr bug update "$_UJ_ONE" --from-json "$_UJ_DIR/object.json"
if assert_success; then
    run_bzr bug view "$_UJ_ONE"
    if assert_json '.priority' "High" &&
        assert_json '.whiteboard' "json-object" &&
        assert_json '.url' "http://example.com/json-object"; then test_pass; fi
fi

test_begin "153. bug update --from-json object target from JSON"
_UJ_TWO=$(make_bug "${_UJ_CREATE[@]}" --summary "update json target")
write_json_fixture "$_UJ_DIR/target.json" \
    "{\"id\":$_UJ_TWO,\"severity\":\"major\",\"whiteboard\":\"json-target\"}"
run_bzr bug update --from-json "$_UJ_DIR/target.json"
if assert_success; then
    run_bzr bug view "$_UJ_TWO"
    if assert_json '.severity' "major" &&
        assert_json '.whiteboard' "json-target"; then test_pass; fi
fi

test_begin "154. bug update --from-json array partial failure"
_UJ_THREE=$(make_bug "${_UJ_CREATE[@]}" --summary "update json array valid")
write_json_fixture "$_UJ_DIR/array.json" \
    "[{\"id\":$_UJ_THREE,\"priority\":\"Low\"},{\"id\":999999,\"priority\":\"High\"}]"
run_bzr bug update --from-json "$_UJ_DIR/array.json"
if assert_exit_code 11 &&
    assert_json '.succeeded[0]' "$_UJ_THREE" &&
    assert_json '.failed[0].id' "999999"; then
    run_bzr bug view "$_UJ_THREE"
    if assert_json '.priority' "Low"; then test_pass; fi
fi

test_begin "155. bug update --from-json stdin with CLI override"
_UJ_FOUR=$(make_bug "${_UJ_CREATE[@]}" --summary "update json override")
write_json_fixture "$_UJ_DIR/stdin.json" \
    "{\"id\":$_UJ_FOUR,\"priority\":\"Low\",\"whiteboard\":\"json-loses\"}"
run_bzr bug update --from-json - --whiteboard "cli-wins" <"$_UJ_DIR/stdin.json"
if assert_success; then
    run_bzr bug view "$_UJ_FOUR"
    if assert_json '.priority' "Low" &&
        assert_json '.whiteboard' "cli-wins"; then test_pass; fi
fi

test_begin "156. bug update --from-json unknown key"
_UJ_FIVE=$(make_bug "${_UJ_CREATE[@]}" --summary "update json bad")
write_json_fixture "$_UJ_DIR/bad.json" "{\"id\":$_UJ_FIVE,\"bogus\":true}"
run_bzr bug update --from-json "$_UJ_DIR/bad.json"
if assert_exit_code 7 && assert_stderr_contains "unknown field"; then test_pass; fi

test_begin "157. bug update --from-json no-op rejected"
_UJ_SIX=$(make_bug "${_UJ_CREATE[@]}" --summary "update json noop")
write_json_fixture "$_UJ_DIR/noop.json" "{\"id\":$_UJ_SIX}"
run_bzr bug update --from-json "$_UJ_DIR/noop.json"
if assert_exit_code 7 && assert_stderr_contains "no fields to update"; then test_pass; fi

rm -r "$_UJ_DIR"
unset _UJ_DIR _UJ_CREATE _UJ_ONE _UJ_TWO _UJ_THREE _UJ_FOUR _UJ_FIVE _UJ_SIX
echo ""
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/run-tests.sh tests/functional/phases/08-bugs.sh tests/functional/phases/08d-bug-update-from-json.sh
shfmt -d tests/functional/run-tests.sh tests/functional/phases/08-bugs.sh tests/functional/phases/08d-bug-update-from-json.sh
```

Commit:

```bash
git add tests/functional/run-tests.sh tests/functional/phases/08-bugs.sh tests/functional/phases/08d-bug-update-from-json.sh
git commit -m "test: cover structured bug updates functionally"
```

### Task 7: Bug Create, Clone, Template, and My-Filter Coverage

**Files:**

- Modify: `tests/functional/phases/08c-bugs-create-fields.sh`
- Modify: `tests/functional/phases/10-bug-clone.sh`
- Modify: `tests/functional/phases/11b-bug-verbs.sh`
- Modify: `tests/functional/phases/12-my-bugs.sh`
- Modify: `tests/functional/phases/13-templates.sh`

- [ ] **Step 1: Expand bug create metadata coverage**

In `tests/functional/phases/08c-bugs-create-fields.sh`, after test `146a`,
insert:

```bash
test_begin "146b. bug create --target-milestone and --deadline round-trip"
MID=$(make_bug "${_CF[@]}" --summary "milestone create" \
    --target-milestone "---" --deadline 2026-12-30)
run_bzr bug view "$MID"
if assert_success &&
    assert_json '.target_milestone' "---" &&
    assert_json '.deadline' "2026-12-30"; then test_pass; fi

test_begin "146c. bug create --groups succeeds with fixture group"
GID=$(make_bug "${_CF[@]}" --summary "group create" --groups functest-grp)
if [[ -n "$GID" ]]; then
    run_bzr bug view "$GID"
    if assert_success && assert_json '.id' "$GID"; then test_pass; fi
fi

test_begin "146d. bug create --flag round-trips"
FID=$(make_bug "${_CF[@]}" --summary "flag create" --flag 'review?')
run_bzr bug view "$FID"
if assert_success &&
    assert_json_contains '[.flags[].name] | join(",")' "review"; then test_pass; fi
```

- [ ] **Step 2: Expand clone metadata coverage**

Append to `tests/functional/phases/10-bug-clone.sh` before `echo ""`:

```bash
test_begin "57a. bug clone copies source metadata"
if [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG3" --url "http://example.com/source-$BUG3" \
        --whiteboard "clone-source-$BUG3" --target-milestone "---" \
        --deadline 2026-12-29
    if assert_success; then
        run_bzr bug clone "$BUG3" --op-sys Linux --rep-platform PC --no-comment
        if assert_success && assert_json_exists '.id'; then
            _CL_META=$(jq -r '.id' "$BZR_STDOUT")
            run_bzr bug view "$_CL_META"
            if assert_json '.url' "http://example.com/source-$BUG3" &&
                assert_json '.whiteboard' "clone-source-$BUG3" &&
                assert_json '.deadline' "2026-12-29"; then test_pass; fi
        fi
    fi
else test_skip "no BUG3"; fi

test_begin "57b. bug clone metadata overrides"
if [[ -n "$BUG3" ]]; then
    _CL_WB="clone-override-$$"
    run_bzr bug clone "$BUG3" --op-sys Linux --rep-platform PC --no-comment \
        --url "http://example.com/clone-override" --whiteboard "$_CL_WB" \
        --target-milestone "---" --deadline 2026-12-28 \
        --cc "$ADMIN_EMAIL" --flag 'review?'
    if assert_success && assert_json_exists '.id'; then
        _CL_OVERRIDE=$(jq -r '.id' "$BZR_STDOUT")
        run_bzr bug view "$_CL_OVERRIDE"
        if assert_json '.url' "http://example.com/clone-override" &&
            assert_json '.whiteboard' "$_CL_WB" &&
            assert_json_contains '[.flags[].name] | join(",")' "review"; then test_pass; fi
    fi
else test_skip "no BUG3"; fi
unset _CL_META _CL_WB _CL_OVERRIDE
```

- [ ] **Step 3: Add convenience verb concurrency guard coverage**

Append to `tests/functional/phases/11b-bug-verbs.sh` before `unset _VERB_CREATE`:

```bash
test_begin "134a. bug resolve --expect-unchanged-since happy path"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve guarded")
run_bzr bug view "$VID"
if assert_success; then
    LCT=$(jq -r '.last_change_time' "$BZR_STDOUT" 2>/dev/null || true)
    run_bzr bug resolve "$VID" --expect-unchanged-since "$LCT"
    if assert_success; then
        run_bzr bug view "$VID"
        if assert_json '.status' "RESOLVED"; then test_pass; fi
    fi
fi

test_begin "134b. bug dup --expect-unchanged-since detects collision"
SRC=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup guarded source")
TGT=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup guarded target")
run_bzr bug view "$SRC"
if assert_success; then
    LCT=$(jq -r '.last_change_time' "$BZR_STDOUT" 2>/dev/null || true)
    run_bzr bug update "$SRC" --whiteboard "verb-guard-touch"
    if wait_for_changed "$SRC" "$LCT"; then
        run_bzr bug dup "$SRC" "$TGT" --expect-unchanged-since "$LCT"
        if assert_exit_code 14; then test_pass; fi
    else
        test_skip "last_change_time did not advance within retry budget"
    fi
fi
```

- [ ] **Step 4: Expand `bug my` filters**

Append to `tests/functional/phases/12-my-bugs.sh` before `echo ""`:

```bash
test_begin "64d. bug my --product --component"
run_bzr bug my --all --product FuncTestProd --component Backend
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64e. bug my --priority --severity"
_MY_PS_MARK=$(unique_name my-priority)
_MY_PS_ID=$(make_bug --marker "$_MY_PS_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --rep-platform PC --description d --summary "my priority filter" \
    --priority High --severity major)
run_bzr bug my --all --priority High --severity major --whiteboard "$_MY_PS_MARK"
if assert_success && assert_stdout_contains "$_MY_PS_ID"; then test_pass; fi

test_begin "64f. bug my --whiteboard --url"
_MY_MARK="my-filter-$$"
_MY_ID=$(make_bug --marker "$_MY_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --rep-platform PC --description d --summary "my filter" \
    --url "http://example.com/$_MY_MARK")
run_bzr bug my --all --whiteboard "$_MY_MARK" --url "$_MY_MARK"
if assert_success && assert_stdout_contains "$_MY_ID"; then test_pass; fi

test_begin "64g. bug my --changed-since malformed"
run_bzr bug my --changed-since "not-a-date"
if assert_exit_code 7; then test_pass; fi
unset _MY_PS_MARK _MY_PS_ID _MY_MARK _MY_ID
```

- [ ] **Step 5: Expand template metadata coverage**

Append to `tests/functional/phases/13-templates.sh` before deleting `func-tmpl`:

```bash
test_begin "69a. template save metadata fields"
run_bzr template save meta-tmpl --product FuncTestProd --component Backend \
    --priority Normal --severity normal --url "http://example.com/template" \
    --whiteboard "template-wb" --target-milestone "---" --deadline 2026-12-27 \
    --cc "$ADMIN_EMAIL" --flag 'review?'
if assert_success; then
    run_bzr template show meta-tmpl
    if assert_json '.url' "http://example.com/template" &&
        assert_json '.whiteboard' "template-wb" &&
        assert_json '.deadline' "2026-12-27"; then test_pass; fi
fi

test_begin "69b. bug create --template metadata applies"
run_bzr bug create --template meta-tmpl --summary "Bug from meta template" \
    --description "Description from meta template" --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    _TMETA_BUG=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$_TMETA_BUG"
    if assert_json '.url' "http://example.com/template" &&
        assert_json '.whiteboard' "template-wb" &&
        assert_json_contains '[.flags[].name] | join(",")' "review"; then test_pass; fi
fi

test_begin "69c. template update --clear metadata"
run_bzr template update meta-tmpl --clear url --clear whiteboard --cc "$ADMIN_EMAIL"
if assert_success; then
    run_bzr template show meta-tmpl
    if assert_json '.url' "null" &&
        assert_json '.whiteboard' "null" &&
        assert_json_contains '.cc | join(",")' "$ADMIN_EMAIL"; then test_pass; fi
fi

test_begin "69d. template delete metadata template"
run_bzr template delete meta-tmpl
if assert_success; then
    run_bzr template show meta-tmpl
    if assert_failure; then test_pass; fi
fi
unset _TMETA_BUG
```

- [ ] **Step 6: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/phases/08c-bugs-create-fields.sh tests/functional/phases/10-bug-clone.sh tests/functional/phases/11b-bug-verbs.sh tests/functional/phases/12-my-bugs.sh tests/functional/phases/13-templates.sh
shfmt -d tests/functional/phases/08c-bugs-create-fields.sh tests/functional/phases/10-bug-clone.sh tests/functional/phases/11b-bug-verbs.sh tests/functional/phases/12-my-bugs.sh tests/functional/phases/13-templates.sh
```

Commit:

```bash
git add tests/functional/phases/08c-bugs-create-fields.sh tests/functional/phases/10-bug-clone.sh tests/functional/phases/11b-bug-verbs.sh tests/functional/phases/12-my-bugs.sh tests/functional/phases/13-templates.sh
git commit -m "test: expand bug workflow functional coverage"
```

### Task 8: Query Functional Coverage

**Files:**

- Modify: `tests/functional/phases/14-queries.sh`
- Modify: `tests/functional/phases/17b-arg-validation.sh`

- [ ] **Step 1: Add `query run --count`**

In `tests/functional/phases/14-queries.sh`, after test `82. query run with
fields override`, insert:

```bash
test_begin "82a. query run --count"
_QCOUNT_MARK=$(unique_name query-count)
make_bug --marker "$_QCOUNT_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --rep-platform PC --description d --summary "query count 1" >/dev/null
make_bug --marker "$_QCOUNT_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --rep-platform PC --description d --summary "query count 2" >/dev/null
run_bzr query save count-bugs --product FuncTestProd --whiteboard "$_QCOUNT_MARK" --limit 1
if assert_success && assert_json '.action' "saved"; then
    run_bzr query run count-bugs --count
    if assert_success && assert_count 2; then test_pass; fi
fi
unset _QCOUNT_MARK
```

Also update test `87. query delete remaining` so it deletes `count-bugs`
between `prod-bugs` and `complex`:

```bash
run_bzr query delete prod-bugs
if assert_success; then
    run_bzr query delete count-bugs
    if assert_success; then
        run_bzr query delete complex
        if assert_success; then test_pass; fi
    fi
fi
```

- [ ] **Step 2: Add `query update --from-url` using the real server URL**

In `tests/functional/phases/14-queries.sh`, after test `82a`, insert:

```bash
test_begin "82b. query update --from-url"
_Q_URL="${BZ_URL}/buglist.cgi?product=FuncTestProd&component=Backend&bug_status=NEW&query_format=advanced"
run_bzr query update complex --from-url "$_Q_URL" --limit 2
if assert_success; then
    run_bzr query show complex
    if assert_json '.product[0]' "FuncTestProd" &&
        assert_json '.component[0]' "Backend" &&
        assert_json '.limit' "2"; then test_pass; fi
fi
unset _Q_URL
```

- [ ] **Step 3: Add count conflict checks**

In `tests/functional/phases/17b-arg-validation.sh`, after test `122. bug list
--offset + --paginate conflict`, insert:

```bash
test_begin "122a. query run --count + --offset conflict"
run_bzr query run prod-bugs --count --offset 1
if assert_exit_code 7 && assert_stderr_contains "cannot be combined with --offset"; then test_pass; fi

test_begin "122b. query run --count + --paginate conflict"
run_bzr query run prod-bugs --count --paginate
if assert_exit_code 7 && assert_stderr_contains "cannot be combined with --offset"; then test_pass; fi
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/phases/14-queries.sh tests/functional/phases/17b-arg-validation.sh
shfmt -d tests/functional/phases/14-queries.sh tests/functional/phases/17b-arg-validation.sh
```

Commit:

```bash
git add tests/functional/phases/14-queries.sh tests/functional/phases/17b-arg-validation.sh
git commit -m "test: cover query update and count options"
```

### Task 9: Attachment and Comment Functional Coverage

**Files:**

- Modify: `tests/functional/phases/16-attachments.sh`
- Modify: `tests/functional/phases/17b-arg-validation.sh`

- [ ] **Step 1: Add attachment comment-file and stdin tests**

In `tests/functional/phases/16-attachments.sh`, after test `100h`, insert:

```bash
test_begin "100k. attachment upload --comment-file"
if [[ -n "$BUG1" ]]; then
    _ACF=$(mktemp /tmp/bzr-func-attachment-comment.XXXXXX)
    printf 'attachment comment from file' >"$_ACF"
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
        --summary "comment file upload" --comment-file "$_ACF"
    if assert_success; then
        run_bzr comment list "$BUG1"
        if assert_stdout_contains "attachment comment from file"; then test_pass; fi
    fi
    rm -f "$_ACF"
else test_skip "no BUG1"; fi

test_begin "100l. attachment upload --comment-file -"
if [[ -n "$BUG1" ]]; then
    _ACF=$(mktemp /tmp/bzr-func-attachment-comment.XXXXXX)
    printf 'attachment comment from stdin' >"$_ACF"
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
        --summary "comment stdin upload" --comment-file - <"$_ACF"
    if assert_success; then
        run_bzr comment list "$BUG1"
        if assert_stdout_contains "attachment comment from stdin"; then test_pass; fi
    fi
    rm -f "$_ACF"
else test_skip "no BUG1"; fi

test_begin "100m. attachment upload empty --comment-file rejected"
if [[ -n "$BUG1" ]]; then
    _ACF=$(mktemp /tmp/bzr-func-attachment-comment.XXXXXX)
    printf '   ' >"$_ACF"
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
        --summary "empty comment upload" --comment-file "$_ACF"
    if assert_exit_code 7 && assert_stderr_contains "empty attachment comment"; then test_pass; fi
    rm -f "$_ACF"
else test_skip "no BUG1"; fi
unset _ACF
```

- [ ] **Step 2: Add raw stdout download**

In `tests/functional/phases/16-attachments.sh`, after test `98. attachment
download`, insert:

```bash
test_begin "98a. attachment download --out - streams raw bytes"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    run_bzr_raw attachment download "$ATTACH_ID" --out -
    if assert_success && assert_stdout_equals_file /tmp/bzr-func-test.txt; then test_pass; fi
else
    test_skip "no attachment ID"
fi
```

- [ ] **Step 3: Add attachment update content-type and flag**

In `tests/functional/phases/16-attachments.sh`, after test `151. attachment
update --file-name round-trips`, insert:

```bash
test_begin "152. attachment update --content-type and --flag"
if [[ -n "${_AID:-}" ]]; then
    run_bzr attachment update "$_AID" --content-type text/plain --flag 'review?'
    if assert_success; then
        run_bzr attachment view "$_AID"
        if assert_json '.content_type' "text/plain" &&
            assert_json_contains '[.flags[].name] | join(",")' "review"; then test_pass; fi
    fi
else test_skip "no attachment id"; fi
```

- [ ] **Step 4: Add no-op rejection tests**

In `tests/functional/phases/17b-arg-validation.sh`, before the final `echo ""`,
insert:

```bash
test_begin "128. attachment update no fields rejected"
run_bzr attachment update 1
if assert_exit_code 7 && assert_stderr_contains "no attachment fields to update"; then test_pass; fi

test_begin "129. comment tag no changes rejected"
run_bzr comment tag 1
if assert_exit_code 7 && assert_stderr_contains "no comment tag changes"; then test_pass; fi
```

- [ ] **Step 5: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/phases/16-attachments.sh tests/functional/phases/17b-arg-validation.sh
shfmt -d tests/functional/phases/16-attachments.sh tests/functional/phases/17b-arg-validation.sh
```

Commit:

```bash
git add tests/functional/phases/16-attachments.sh tests/functional/phases/17b-arg-validation.sh
git commit -m "test: expand attachment and comment functional coverage"
```

### Task 10: Schema and README Refresh

**Files:**

- Modify: `tests/functional/phases/18-completion-schema.sh`
- Modify: `tests/functional/README.md`

- [ ] **Step 1: Expand schema list assertions**

In `tests/functional/phases/18-completion-schema.sh`, replace the schema list
assertion with:

```bash
if assert_success && assert_json_valid &&
    assert_schema_list_contains "bug" &&
    assert_schema_list_contains "bug-create-input" &&
    assert_schema_list_contains "bug-update-input" &&
    assert_schema_list_contains "product-create-input" &&
    assert_schema_list_contains "product-update-input" &&
    assert_schema_list_contains "component-create-input" &&
    assert_schema_list_contains "component-update-input" &&
    assert_schema_list_contains "user-create-input" &&
    assert_schema_list_contains "user-update-input" &&
    assert_schema_list_contains "group-create-input" &&
    assert_schema_list_contains "group-update-input" &&
    assert_schema_list_contains "error"; then test_pass; fi
```

- [ ] **Step 2: Add per-schema smoke checks**

After the existing `schema bug-update-input` test, insert:

```bash
test_begin "118a. schema product-create-input"
run_bzr schema product-create-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "118b. schema component-update-input"
run_bzr schema component-update-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "118c. schema user-update-input"
run_bzr schema user-update-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "118d. schema group-update-input"
run_bzr schema group-update-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi
```

- [ ] **Step 3: Refresh `tests/functional/README.md`**

Replace the stale "Test Structure" section with:

```markdown
## Test Structure

Tests run in dependency order across the phase files sourced by
`tests/functional/run-tests.sh`:

1. Build and isolated config setup
2. Server/auth detection and fixture capability setup
3. Products and components
4. Fields, classifications, users, and groups
5. Bug create/read/update/search/paging/relationships/collision/clone workflows
6. Batch updates and convenience verbs
7. My-bug filters, templates, and saved queries
8. Comments and attachments, including private-resource hybrid/XML-RPC paths
9. Global options, argument validation, completion, schema, and sequence tests

The suite creates real Bugzilla data in the running container and reads it back
through the CLI. Count and paging assertions use per-run unique whiteboard
markers so repeated runs against an already-started container stay stable.

TLS functional coverage for ad-hoc `--server-tls-*` flags is intentionally not
part of this suite expansion. It needs an HTTPS fixture in front of Bugzilla and
is tracked separately in GitHub issue #406.
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset
BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
shellcheck tests/functional/phases/18-completion-schema.sh
shfmt -d tests/functional/phases/18-completion-schema.sh
```

Commit:

```bash
git add tests/functional/phases/18-completion-schema.sh tests/functional/README.md
git commit -m "test: refresh functional schema coverage docs"
```

### Task 11: Cross-Version Verification and Final Cleanup

**Files:**

- Read: `tests/functional/run-all-versions.sh`
- Read: all modified files

- [ ] **Step 1: Run focused Rust tests for changed command contracts**

Run:

```bash
cargo test --lib from_json
cargo test --lib dry_run
cargo test --lib query_run_count
cargo test --lib schema
```

Expected: all selected tests pass.

- [ ] **Step 2: Run shell checks on all functional scripts**

Run:

```bash
shellcheck tests/functional/*.sh tests/functional/phases/*.sh tests/functional/versions/*/*.sh
shfmt -d tests/functional/*.sh tests/functional/phases/*.sh tests/functional/versions/*/*.sh
```

Expected: shellcheck exits 0 and shfmt prints no diff.

- [ ] **Step 3: Run all Bugzilla versions**

Run:

```bash
tests/functional/run-all-versions.sh
```

Expected: `bz50`, `bz52`, and `bz53` pass. If a version-specific API endpoint
is unavailable, the relevant test must skip with an explicit message rather than
failing or silently passing without coverage.

- [ ] **Step 4: Review for unnecessary complexity**

Run:

```bash
git diff --stat
git diff -- tests/functional
```

Check:

- Every new test creates or targets real Bugzilla data unless it is an explicit
  parse-level conflict or no-op guard.
- Every exact count uses a per-run unique marker.
- No test relies on pre-existing shared database totals.
- No TLS functional fixture was added.
- No helper is used only once unless it centralizes a fragile shell pattern.

- [ ] **Step 5: Final commit**

Run:

```bash
git status --short
git add tests/functional
git add -f docs/superpowers/plans/2026-06-22-functional-cli-coverage.md
git commit -m "test: expand functional CLI coverage"
```

Expected: final commit contains any remaining test and documentation changes.

## Self-Review

Spec coverage:

- Credentialless real-server coverage is handled by Task 4.
- Admin `--from-json` and `--dry-run` are handled by Task 5.
- Bug update, create metadata, clone metadata, templates, verb guards, and
  `bug my` filters are handled by Tasks 6 and 7.
- Query, attachment, comment, no-op, schema, and README coverage are handled by
  Tasks 8 through 10.
- Cross-version verification is handled by Task 11.
- TLS functional coverage is deliberately excluded and tracked by #406.

Placeholder scan:

- The plan contains no placeholder markers, no unspecified test steps, and no instruction to
  add generic tests without concrete commands.

Type and naming consistency:

- Shell helper names are defined in Task 2 before later tasks use them.
- The new phase name `08d-bug-update-from-json` is used consistently in file
  paths and in `run-tests.sh`.
