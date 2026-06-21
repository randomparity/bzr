# 08c-bugs-create-fields
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: its own bugs.
# shellcheck shell=bash
#
# Metadata fields on `bug create` and the `--from-json` structured input added
# since v0.4.4. (--alias is intentionally not asserted: bug aliases are disabled
# on these default-config containers, so the field silently no-ops.)
#
# NB: --from-json input is fed by redirection from a temp file, never by a pipe.
# `printf ... | run_bzr` would run run_bzr in a subshell, so its BZR_EXIT capture
# would not propagate to this shell and exit-code assertions would read a stale
# value.

# ══════════════════════════════════════════════════════════════════════
# Phase 8c: Bug create metadata fields & --from-json
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8c: Bug create fields / --from-json ───────────────"

_CF=(--product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d)
_FJ=$(mktemp -d /tmp/bzr-func-fromjson.XXXXXX)
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"fj object","op_sys":"Linux","rep_platform":"PC","description":"d","priority":"High"}' >"$_FJ/obj.json"
printf '%s' '[{"product":"FuncTestProd","component":"Backend","summary":"fj arr a","op_sys":"Linux","rep_platform":"PC","description":"d"},{"product":"FuncTestProd","component":"Backend","summary":"fj arr b","op_sys":"Linux","rep_platform":"PC","description":"d"}]' >"$_FJ/arr.json"
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"x","op_sys":"Linux","rep_platform":"PC","description":"d","boguskey":1}' >"$_FJ/bad.json"
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"json sum","op_sys":"Linux","rep_platform":"PC","description":"d"}' >"$_FJ/override.json"

test_begin "142. bug create metadata fields round-trip (url/whiteboard/cc)"
_WB="cf$$x${RANDOM}"
CFID=$(make_bug "${_CF[@]}" --summary "create meta" --url "http://example.com/cf" --whiteboard "$_WB" --cc admin@test.bzr)
run_bzr bug view "$CFID"
if assert_success && assert_json '.url' "http://example.com/cf" &&
    assert_json '.whiteboard' "$_WB" && assert_json_contains '.cc | join(",")' "admin@test.bzr"; then test_pass; fi

test_begin "143. bug create --from-json object via stdin (-)"
run_bzr bug create --from-json - <"$_FJ/obj.json"
if assert_success; then
    FID=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$FID"
    if assert_json '.summary' "fj object" && assert_json '.priority' "High"; then test_pass; fi
fi

test_begin "144. bug create --from-json array creates multiple bugs"
run_bzr bug create --from-json "$_FJ/arr.json"
if assert_success && assert_json '.created | length' "2" && assert_json '.failed | length' "0"; then test_pass; fi

test_begin "145. bug create --from-json unknown key (exit 7)"
run_bzr bug create --from-json "$_FJ/bad.json"
if assert_exit_code 7 && assert_stderr_contains "unknown field"; then test_pass; fi

test_begin "146. bug create --from-json with CLI flag override"
run_bzr bug create --from-json - --summary "cli wins" <"$_FJ/override.json"
if assert_success; then
    OID=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$OID"
    if assert_json '.summary' "cli wins"; then test_pass; fi
fi

# --keywords round-trip. The fix-needed keyword is seeded only on bz52+; gate so
# older Bugzilla (no keyword definition) skips cleanly rather than erroring.
test_begin "146a. bug create --keywords round-trips"
if require_version 520 "fix-needed keyword seeded on bz52+"; then
    KID=$(make_bug "${_CF[@]}" --summary "kw create" --keywords fix-needed)
    run_bzr bug view "$KID"
    if assert_success && assert_json_contains '.keywords | join(",")' "fix-needed"; then test_pass; fi
fi
unset KID

rm -rf "$_FJ"
unset _CF _FJ _WB CFID FID OID
echo ""
