# 06-users
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 6: Users
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 6: Users ──────────────────────────────────────────"

test_begin "user-create" "user create"
run_bzr user create --email testuser@test.bzr --full-name "Test User" --password "TestPass1!"
if [[ $BZR_EXIT -eq 0 ]]; then
  test_pass
elif grep -q "already" "$BZR_STDERR" 2>/dev/null; then
  test_pass # idempotent
else
  assert_success
fi

# Re-enable testuser in case it was disabled by a prior run (test 24 sets disable_login=true)
test_begin "user-re-enable-idempotent-fix" "user re-enable (idempotent fix)"
run_bzr user update testuser@test.bzr --disable-login false --login-denied-text ""
if assert_success; then test_pass; fi

test_begin "user-update-real-name-bz52" "user update real name uses full_name on bz52"
if [[ "$BZ_VERSION" == "bz52" ]]; then
  run_bzr user update testuser@test.bzr --real-name "Test User Updated"
  if assert_success; then
    run_bzr user search testuser@test.bzr --details
    if assert_success &&
      assert_json '[.[] | select(.name == "testuser@test.bzr")][0].real_name' \
        "Test User Updated"; then test_pass; fi
  fi
else
  test_skip "Bugzilla 5.2 is the required real-name update conformance arm"
fi

test_begin "user-search-testuser" "user search testuser"
run_bzr user search testuser
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "user-search-testuser-details" "user search testuser --details"
run_bzr user search testuser --details
if assert_success; then test_pass; fi

test_begin "user-search-fields-projects-keys" "user search --fields projects keys"
run_bzr user search testuser --fields id,email
if assert_success && assert_json '.[0] | keys | length' 2 &&
  assert_json_exists '.[0].id'; then test_pass; fi

test_begin "user-search-fields-unknown-exits-7" "user search --fields unknown exits 7"
run_bzr user search testuser --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

test_begin "user-update-testuser" "user update testuser"
# Note: Bugzilla 5.0 REST API does not support real_name updates
# (set_real_name method not found). Use login_denied_text instead.
run_bzr user update testuser@test.bzr --disable-login true --login-denied-text "test disabled"
if assert_success; then test_pass; fi

test_begin "production-shaped-user-group-create-ids" "production-shaped user and group create ids"
export BZR_FUNC_INLINE_KEY="$API_KEY"
if redhat_shape_start "$BZ_PORT"; then
  trap 'cleanup; redhat_shape_stop' EXIT
  _UC_PROXY_URL="http://127.0.0.1:${REDHAT_SHAPE_PORT}"
  _UC_LOGIN="$(unique_name shapeuser)@test.bzr"
  _UC_GROUP="$(unique_name shapegrp)"
  _UC_OK=1
  run_bzr_raw --json --server-url "$_UC_PROXY_URL" \
    --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" \
    user create --email "$_UC_LOGIN" --full-name "Shape User" --password "TestPass1!"
  if [[ $BZR_EXIT -ne 0 ]] ||
    ! jq -e '.id | type == "number"' "$BZR_STDOUT" >/dev/null; then _UC_OK=0; fi
  run_bzr_raw --json --server-url "$_UC_PROXY_URL" \
    --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" \
    group create --name "$_UC_GROUP" --description "Shape group"
  if [[ $BZR_EXIT -ne 0 ]] ||
    ! jq -e '.id | type == "number"' "$BZR_STDOUT" >/dev/null; then _UC_OK=0; fi
  if ! grep -Eq 'user-group-shaped route=user-create count=[1-9][0-9]*' \
    "$REDHAT_SHAPE_LOG" ||
    ! grep -Eq 'user-group-shaped route=group-create count=[1-9][0-9]*' \
      "$REDHAT_SHAPE_LOG"; then _UC_OK=0; fi
  redhat_shape_stop || _UC_OK=0
  trap cleanup EXIT
  if [[ $_UC_OK -eq 1 ]]; then test_pass; else
    test_fail "production-shaped create ID proof failed; proxy log: $REDHAT_SHAPE_LOG"
  fi
else
  test_fail "response-shape proxy did not become ready: $REDHAT_SHAPE_LOG"
fi
unset BZR_FUNC_INLINE_KEY
unset _UC_PROXY_URL _UC_LOGIN _UC_GROUP _UC_OK

_UJSON_DIR=$(mktemp -d /tmp/bzr-func-user-json.XXXXXX)
_UJ_LOGIN="$(unique_name userjson)@test.bzr"
write_json_fixture "$_UJSON_DIR/create.json" \
  "{\"email\":\"$_UJ_LOGIN\",\"full_name\":\"User Json\",\"password\":\"TestPass1!\"}"
write_json_fixture "$_UJSON_DIR/update.json" \
  "{\"user\":\"$_UJ_LOGIN\",\"disable_login\":true,\"login_denied_text\":\"json disabled\"}"

test_begin "user-create-from-json" "user create --from-json"
run_bzr user create --from-json "$_UJSON_DIR/create.json"
if assert_success; then
  run_bzr user search "$_UJ_LOGIN" --details
  if assert_stdout_contains "$_UJ_LOGIN"; then test_pass; fi
fi

test_begin "user-update-from-json" "user update --from-json"
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
