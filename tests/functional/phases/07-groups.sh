# 07-groups
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 7: Groups
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 7: Groups ─────────────────────────────────────────"

test_begin "group-create" "group create"
run_bzr group create --name functest-grp --description "Test group"
if [[ $BZR_EXIT -eq 0 ]]; then
    test_pass
elif grep -q "already exists" "$BZR_STDERR" 2>/dev/null; then
    test_pass # idempotent
else
    assert_success
fi

test_begin "group-view-functest-grp" "group view functest-grp"
run_bzr group view functest-grp
if [[ $BZR_EXIT -eq 0 ]] && assert_json '.name' "functest-grp"; then
    test_pass
else
    assert_success
fi

test_begin "group-view-fields-projects-keys" "group view --fields projects keys"
run_bzr group view functest-grp --fields id,name
if assert_success && assert_json 'keys | length' 2 && assert_json_exists '.name'; then
    test_pass
fi

test_begin "group-view-fields-unknown-exits-7" "group view --fields unknown exits 7"
run_bzr group view functest-grp --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

test_begin "group-view-functest-grp-with-api-rest" "group view functest-grp with --api rest"
run_bzr_raw --json --server test --api rest group view functest-grp
if [[ $BZR_EXIT -eq 0 ]] && assert_json '.name' "functest-grp"; then
    test_pass
else
    assert_success
fi

test_begin "group-update-functest-grp" "group update functest-grp"
run_bzr group update functest-grp --description "Updated group desc"
if assert_success; then test_pass; fi

test_begin "fixture-group-enabled-for-functestprod-bugs" "fixture group enabled for FuncTestProd bugs"
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

# Re-enable testuser before group membership tests (test 24 disables it)
test_begin "user-re-enable-for-group-tests" "user re-enable for group tests"
run_bzr user update testuser@test.bzr --disable-login false --login-denied-text ""
if assert_success; then test_pass; fi

# The enabled non-member fixture (issue #617). It exists so #625 can assert that
# `group list-users --group functest-grp` excludes a user the server would
# otherwise return. That assertion is red until #625 lands the group-filter fix,
# so this phase provisions and validates the fixture and stops there.
# Invariant: nothing may leave $NONMEMBER_EMAIL in a group. Containers are
# reused between runs, so a membership added here survives into the next one.
test_begin "fixture-enabled-non-member-user" "fixture enabled non-member user"
if ! ensure_enabled_nonmember_user; then
    test_fail "could not provision the enabled non-member fixture user"
elif assert_user_login_enabled "$NONMEMBER_EMAIL"; then
    test_pass
fi

test_begin "group-add-user" "group add-user"
run_bzr group add-user --group functest-grp --user testuser@test.bzr
if assert_success; then test_pass; fi

# The fixture's non-membership is what #625's assertion will rest on, so assert
# it rather than trusting that nothing added the user to a group — containers
# are reused across runs, so a stray membership persists indefinitely. The
# testuser half is the positive control: it proves the harness can see
# membership at all, so the nonmember half cannot pass on an empty `groups`.
# Both read the user resource, not `group list-users`, so neither depends on
# the group filter #625 owns.
test_begin "fixture-non-member-is-not-in-the-group" "fixture non-member is not in the group"
if assert_user_group_membership "testuser@test.bzr" functest-grp in &&
    assert_user_group_membership "$NONMEMBER_EMAIL" functest-grp out; then
    test_pass
fi

# TODO(#625): these list-users assertions pass whether or not the group filter is
# honored. An added member appears in an unfiltered listing too, and the absence
# assertion below is only reached after the user is disabled, which hides it from
# user search regardless. #625 owns the `groups=` fix and the replacement
# assertion, which uses the enabled $NONMEMBER_EMAIL fixture above.
test_begin "group-list-users" "group list-users"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "group-list-users-details" "group list-users --details"
run_bzr group list-users --group functest-grp --details
if assert_success; then test_pass; fi

test_begin "group-list-users-fields-projects-keys" "group list-users --fields projects keys"
run_bzr group list-users --group functest-grp --fields id,email
if assert_success && assert_json '.[0] | keys | length' 2 &&
    assert_json_exists '.[0].id'; then test_pass; fi

test_begin "group-list-users-fields-unknown-exits-7" "group list-users --fields unknown exits 7"
run_bzr group list-users --group functest-grp --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

test_begin "group-remove-user" "group remove-user"
run_bzr group remove-user --group functest-grp --user testuser@test.bzr
if assert_success; then test_pass; fi

# Re-disable testuser so it's excluded from list-users results (Bugzilla 5.0
# default user search hides disabled users, which is also what test 24 does)
# TODO(#625): this re-disable is what makes the absence assertion below pass; it
# is not evidence that `group remove-user` worked.
run_bzr user update testuser@test.bzr --disable-login true --login-denied-text "test disabled" >/dev/null 2>&1 || true

test_begin "group-list-users-after-remove" "group list-users (after remove)"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_not_contains "testuser@test.bzr"; then test_pass; fi

_GJSON_DIR=$(mktemp -d /tmp/bzr-func-group-json.XXXXXX)
_GJ_NAME=$(unique_name groupjson)
write_json_fixture "$_GJSON_DIR/create.json" \
    "{\"name\":\"$_GJ_NAME\",\"description\":\"group json\",\"is_active\":true}"
write_json_fixture "$_GJSON_DIR/update.json" \
    "{\"group\":\"$_GJ_NAME\",\"description\":\"group json updated\",\"is_active\":false}"

test_begin "group-create-from-json" "group create --from-json"
run_bzr group create --from-json "$_GJSON_DIR/create.json"
if assert_success; then
    run_bzr group view "$_GJ_NAME"
    if assert_json '.name' "$_GJ_NAME"; then test_pass; fi
fi

test_begin "group-update-from-json" "group update --from-json"
run_bzr group update --from-json "$_GJSON_DIR/update.json"
if assert_success; then
    run_bzr group view "$_GJ_NAME"
    if assert_json '.description' "group json updated"; then test_pass; fi
fi

rm -r "$_GJSON_DIR"
unset _GJSON_DIR _GJ_NAME

echo ""
