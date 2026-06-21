# 06-users
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 6: Users
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 6: Users ──────────────────────────────────────────"

test_begin "21. user create"
run_bzr user create --email testuser@test.bzr --full-name "Test User" --password "TestPass1!"
if [[ $BZR_EXIT -eq 0 ]]; then
    test_pass
elif grep -q "already" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent
else
    assert_success
fi

# Re-enable testuser in case it was disabled by a prior run (test 24 sets disable_login=true)
test_begin "21b. user re-enable (idempotent fix)"
run_bzr user update testuser@test.bzr --disable-login false --login-denied-text ""
if assert_success; then test_pass; fi

test_begin "22. user search testuser"
run_bzr user search testuser
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "23. user search testuser --details"
run_bzr user search testuser --details
if assert_success; then test_pass; fi

test_begin "24. user update testuser"
# Note: Bugzilla 5.0 REST API does not support real_name updates
# (set_real_name method not found). Use login_denied_text instead.
run_bzr user update testuser@test.bzr --disable-login true --login-denied-text "test disabled"
if assert_success; then test_pass; fi

echo ""

