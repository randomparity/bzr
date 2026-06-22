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
    test_pass # idempotent
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

echo ""
