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
