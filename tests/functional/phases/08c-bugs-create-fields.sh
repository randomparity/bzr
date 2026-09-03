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

_CF=(--product FuncTestProd --component Backend --op-sys Linux --platform PC --description d)
_FJ=$(mktemp -d /tmp/bzr-func-fromjson.XXXXXX)
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"fj object","op_sys":"Linux","platform":"PC","description":"d","priority":"High"}' >"$_FJ/obj.json"
printf '%s' '[{"product":"FuncTestProd","component":"Backend","summary":"fj arr a","op_sys":"Linux","platform":"PC","description":"d"},{"product":"FuncTestProd","component":"Backend","summary":"fj arr b","op_sys":"Linux","platform":"PC","description":"d"}]' >"$_FJ/arr.json"
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"x","op_sys":"Linux","platform":"PC","description":"d","boguskey":1}' >"$_FJ/bad.json"
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"json sum","op_sys":"Linux","platform":"PC","description":"d"}' >"$_FJ/override.json"
printf '%s' '{"product":"FuncTestProd","component":"Backend","summary":"legacy platform alias","op_sys":"Linux","rep_platform":"PC","description":"d"}' >"$_FJ/legacy-platform.json"

test_begin "bug-create-metadata-fields-round-trip-url-whiteboard-cc" "bug create metadata fields round-trip (url/whiteboard/cc)"
_WB="cf$$x${RANDOM}"
CFID=$(make_bug "${_CF[@]}" --summary "create meta" --url "http://example.com/cf" --whiteboard "$_WB" --cc admin@test.bzr)
run_bzr bug view "$CFID"
if assert_success && assert_json '.url' "http://example.com/cf" &&
    assert_json '.whiteboard' "$_WB" && assert_json_contains '.cc | join(",")' "admin@test.bzr"; then test_pass; fi

test_begin "bug-create-from-json-object-via-stdin" "bug create --from-json object via stdin (-)"
run_bzr bug create --from-json - <"$_FJ/obj.json"
if assert_success; then
    FID=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$FID"
    if assert_json '.summary' "fj object" && assert_json '.priority' "High"; then test_pass; fi
fi

test_begin "bug-create-from-json-rejects-removed-platform-alias" "bug create --from-json rejects removed rep_platform alias"
run_bzr bug create --from-json "$_FJ/legacy-platform.json"
if assert_failure; then test_pass; fi

test_begin "bug-create-from-json-array-creates-multiple-bugs" "bug create --from-json array creates multiple bugs"
run_bzr bug create --from-json "$_FJ/arr.json"
if assert_success && assert_json '.created | length' "2" && assert_json '.failed | length' "0"; then test_pass; fi

# #462: --progress ndjson streams batch/done events on stderr for the array form;
# stdout stays the clean partial-failure result object.
test_begin "bug-create-from-json-array-progress-ndjson-streams-events" "bug create --from-json array --progress ndjson streams events"
run_bzr bug create --from-json "$_FJ/arr.json" --progress ndjson
if assert_success && assert_json '.created | length' "2" &&
    assert_stderr_contains '"event":"batch"' &&
    assert_stderr_contains '"event":"done"'; then test_pass; fi

test_begin "bug-create-from-json-unknown-key-exit-7" "bug create --from-json unknown key (exit 7)"
run_bzr bug create --from-json "$_FJ/bad.json"
if assert_exit_code 7 && assert_stderr_contains "unknown field"; then test_pass; fi

test_begin "bug-create-from-json-with-cli-flag-override" "bug create --from-json with CLI flag override"
run_bzr bug create --from-json - --summary "cli wins" <"$_FJ/override.json"
if assert_success; then
    OID=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$OID"
    if assert_json '.summary' "cli wins"; then test_pass; fi
fi

# --keywords round-trip. The fix-needed keyword is seeded only on bz52+; gate so
# older Bugzilla (no keyword definition) skips cleanly rather than erroring.
test_begin "bug-create-keywords-round-trips" "bug create --keywords round-trips"
if require_version 520 "fix-needed keyword seeded on bz52+"; then
    KID=$(make_bug "${_CF[@]}" --summary "kw create" --keywords fix-needed)
    run_bzr bug view "$KID"
    if assert_success && assert_json_contains '.keywords | join(",")' "fix-needed"; then test_pass; fi
fi
unset KID

test_begin "bug-create-target-milestone-and-deadline-round-trip" "bug create --target-milestone and --deadline round-trip"
MID=$(make_bug "${_CF[@]}" --summary "milestone create" \
    --target-milestone=--- --deadline 2026-12-30)
run_bzr bug view "$MID"
if assert_success &&
    assert_json '.target_milestone' "---" &&
    assert_json '.deadline' "2026-12-30"; then test_pass; fi

test_begin "bug-create-groups-restricts-public-access" "bug create --groups restricts public access"
_GROUP_WB=$(unique_name group-restrict)
GID=$(make_bug --marker "$_GROUP_WB" "${_CF[@]}" --summary "group create" --groups functest-grp)
if [[ -n "$GID" ]]; then
    run_bzr bug view "$GID"
    if assert_success && assert_json '.id' "$GID"; then
        run_bzr_raw --json --server public bug view "$GID"
        # #504: `assert_failure` alone passes for exit 2, 4, 5 and 9 alike, so
        # it could not tell an access error from bzr masking one as
        # "bug not found". Pin the code (ADR 0015); phase 08e covers the
        # authenticated member/non-member directions.
        if assert_exit_code 4 &&
            assert_stderr_json '.error.api_code' "102" &&
            assert_stderr_not_contains "not found"; then
            run_bzr_raw --json --server public bug list --whiteboard "$_GROUP_WB"
            if assert_success && assert_json_array_length '.' 0; then test_pass; fi
        fi
    fi
fi

test_begin "bug-create-flag-round-trips" "bug create --flag round-trips"
FID=$(make_bug "${_CF[@]}" --summary "flag create" --flag 'bzr-bug-review?')
run_bzr bug view "$FID"
if assert_success &&
    assert_json_contains '[.flags[].name] | join(",")' "bzr-bug-review"; then test_pass; fi

# #458: compound create — bug + first comment + attachment in one invocation.
_CA_FILE="$_FJ/trace.log"
printf 'boot trace %s\n' "$RANDOM" >"$_CA_FILE"
_CA_MARK="compound$$x${RANDOM}"

test_begin "bug-create-with-comment-with-attachment-compound-flags" "bug create --with-comment --with-attachment (compound flags)"
CCID=$(make_bug "${_CF[@]}" --summary "compound create" \
    --with-comment "${_CA_MARK} reproduced" \
    --with-attachment "$_CA_FILE" --attachment-description "boot trace log")
if [[ -n "$CCID" ]]; then
    run_bzr comment list "$CCID"
    if assert_success && assert_json_contains '[.[].text] | join("\n")' "$_CA_MARK"; then
        run_bzr attachment list "$CCID"
        if assert_success &&
            assert_json_contains '[.[].summary] | join("\n")' "boot trace log"; then test_pass; fi
    fi
fi

test_begin "bug-create-compound-dry-run-previews-comment-attachment" "bug create compound --dry-run previews comment + attachment"
run_bzr bug create "${_CF[@]}" --summary "compound dry" \
    --with-comment "dry note" --with-attachment "$_CA_FILE" --attachment-description "dry trace" \
    --dry-run
if assert_success && assert_json '.action' "dry-run" &&
    assert_json '.changes.comment' "dry note" &&
    assert_json_exists '.changes.attachments[0].file_name'; then test_pass; fi

test_begin "bug-create-from-json-with-comment-attachments" "bug create --from-json with comment + attachments"
printf '%s' "{\"product\":\"FuncTestProd\",\"component\":\"Backend\",\"summary\":\"fj compound\",\"op_sys\":\"Linux\",\"platform\":\"PC\",\"description\":\"d\",\"comment\":{\"body\":\"${_CA_MARK} json\"},\"attachments\":[{\"file\":\"$_CA_FILE\",\"description\":\"json trace sum\"}]}" >"$_FJ/compound.json"
run_bzr bug create --from-json "$_FJ/compound.json"
if assert_success; then
    JCID=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr comment list "$JCID"
    if assert_success && assert_json_contains '[.[].text] | join("\n")' "${_CA_MARK} json"; then
        run_bzr attachment list "$JCID"
        if assert_success &&
            assert_json_contains '[.[].summary] | join("\n")' "json trace sum"; then test_pass; fi
    fi
fi

test_begin "bug-create-with-comment-empty-body-exits-7" "bug create --with-comment empty body exits 7"
run_bzr bug create "${_CF[@]}" --summary "empty compound comment" --with-comment "   "
if assert_exit_code 7; then test_pass; fi

rm -r "$_FJ"
unset _CF _FJ _WB _GROUP_WB CFID FID OID MID GID CCID JCID _CA_FILE _CA_MARK
echo ""
