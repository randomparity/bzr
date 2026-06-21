# 07-groups
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 7: Groups
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 7: Groups ─────────────────────────────────────────"

test_begin "25. group create"
run_bzr group create --name functest-grp --description "Test group"
if [[ $BZR_EXIT -eq 0 ]]; then
    test_pass
elif grep -q "already exists" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent
else
    assert_success
fi

test_begin "26. group view functest-grp"
run_bzr group view functest-grp
if [[ $BZR_EXIT -eq 0 ]] && assert_json '.name' "functest-grp"; then
    test_pass
else
    assert_success
fi

test_begin "26a. group view functest-grp with --api rest"
run_bzr_raw --json --server test --api rest group view functest-grp
if [[ $BZR_EXIT -eq 0 ]] && assert_json '.name' "functest-grp"; then
    test_pass
else
    assert_success
fi

test_begin "27. group update functest-grp"
run_bzr group update functest-grp --description "Updated group desc"
if assert_success; then test_pass; fi

# Re-enable testuser before group membership tests (test 24 disables it)
test_begin "27b. user re-enable for group tests"
run_bzr user update testuser@test.bzr --disable-login false --login-denied-text ""
if assert_success; then test_pass; fi

test_begin "28. group add-user"
run_bzr group add-user --group functest-grp --user testuser@test.bzr
if assert_success; then test_pass; fi

test_begin "29. group list-users"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "30. group list-users --details"
run_bzr group list-users --group functest-grp --details
if assert_success; then test_pass; fi

test_begin "31. group remove-user"
run_bzr group remove-user --group functest-grp --user testuser@test.bzr
if assert_success; then test_pass; fi

# Re-disable testuser so it's excluded from list-users results (Bugzilla 5.0
# default user search hides disabled users, which is also what test 24 does)
run_bzr user update testuser@test.bzr --disable-login true --login-denied-text "test disabled" >/dev/null 2>&1 || true

test_begin "32. group list-users (after remove)"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_not_contains "testuser@test.bzr"; then test_pass; fi

echo ""

