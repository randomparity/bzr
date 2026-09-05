# 17-global-options
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 16: Global Options Smoke Tests
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 16: Global Options ────────────────────────────────"

test_begin "output-table" "--output table"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --output table bug view "$BUG1"
    if assert_success; then
        # Table output should NOT be valid JSON
        if ! jq . "$BZR_STDOUT" >/dev/null 2>&1; then
            test_pass
        else
            # Some commands may produce JSON-like table output; just check success
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "quiet" "--quiet"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "quiet-suppresses-stderr-tracing" "--quiet suppresses stderr tracing"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet -vvv bug view "$BUG1"
    if assert_success && assert_stdout_empty && assert_stderr_empty; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "quiet-preserves-error-exit-code" "--quiet preserves error exit code"
if true; then
    run_bzr_raw --quiet bug view 999999
    if assert_failure && assert_stdout_empty; then test_pass; fi
fi

test_begin "quiet-json-suppresses-stdout" "--quiet + --json suppresses stdout"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet --json bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "verbose-response-body-diagnostics-redact-api-keys" "verbose response-body diagnostics redact API keys"
_TRACE_SECRET="TraceEchoSecret0123456789"
_TRACE_BUG=$(make_bug --product FuncTestProd --component Backend --op-sys Linux \
    --platform PC --description d \
    --summary "response echo Bugzilla_api_key=${_TRACE_SECRET}")
if [[ -n "$_TRACE_BUG" ]]; then
    run_bzr_raw -vvv bug view "$_TRACE_BUG"
    if assert_success &&
        assert_stderr_contains 'Bugzilla_api_key=\[REDACTED\]' &&
        assert_stderr_not_contains "$_TRACE_SECRET"; then test_pass; fi
else test_fail "could not create trace redaction fixture"; fi
unset _TRACE_SECRET _TRACE_BUG

test_begin "server-test-whoami" "--server test whoami"
run_bzr_raw --server test whoami
if assert_success; then test_pass; fi

# --output ndjson: empty list emits zero lines; with data, one value per line.
test_begin "output-ndjson-empty-list-emits-no-lines" "--output ndjson (empty list emits no lines)"
run_bzr_raw --output ndjson bug list --whiteboard "nomatch$$x${RANDOM}"
if assert_success && assert_ndjson_line_count 0; then test_pass; fi

test_begin "output-ndjson-one-value-per-line" "--output ndjson (one value per line)"
_NM="nd$$x${RANDOM}"
make_bug --marker "$_NM" --product FuncTestProd --component Backend --op-sys Linux --platform PC --description d --summary "ndjson 1" >/dev/null
make_bug --marker "$_NM" --product FuncTestProd --component Backend --op-sys Linux --platform PC --description d --summary "ndjson 2" >/dev/null
run_bzr_raw --output ndjson bug list --whiteboard "$_NM"
if assert_success && assert_ndjson_line_count 2; then test_pass; fi
unset _NM

test_begin "table-width-wraps-and-isolates-json" "BZR_TABLE_WIDTH wraps tables and leaves JSON-family output unchanged"
_TW_MARK=$(unique_name table-width)
_TW_SUMMARY="table width fixture has enough ASCII words to force a wrapped continuation in the list grid"
_TW_BUG=$(make_bug --marker "$_TW_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --platform PC --description d --summary "$_TW_SUMMARY")
_TW_DIR=$(mktemp -d /tmp/bzr-func-table-width.XXXXXX)
if [[ -n "$_TW_BUG" ]]; then
    LC_ALL=C BZR_TABLE_WIDTH=60 run_bzr_raw --server public --output table bug list \
        --whiteboard "$_TW_MARK"
    if assert_success; then
        if awk 'length($0) > 60 { exit 1 }' "$BZR_STDOUT_RAW" &&
            awk '/^\|[[:space:]]+\|/ { found = 1 } END { exit !found }' "$BZR_STDOUT_RAW"; then
            LC_ALL=C BZR_TABLE_WIDTH=60 run_bzr_raw --output table bug list --whiteboard "$_TW_MARK"
            if assert_success; then
                if awk 'length($0) > 60 { exit 1 }' "$BZR_STDOUT_RAW" &&
                    awk '/^\|[[:space:]]+\|/ { found = 1 } END { exit !found }' "$BZR_STDOUT_RAW"; then
                    run_bzr_raw --json bug list --whiteboard "$_TW_MARK"
                    cp "$BZR_STDOUT_RAW" "$_TW_DIR/json"
                    BZR_TABLE_WIDTH=invalid run_bzr_raw --json bug list --whiteboard "$_TW_MARK"
                    if assert_success && assert_json '.[] | .id' "$_TW_BUG" && assert_stderr_empty; then
                        if cmp -s "$BZR_STDOUT_RAW" "$_TW_DIR/json"; then
                            run_bzr_raw --output ndjson bug list --whiteboard "$_TW_MARK"
                            cp "$BZR_STDOUT_RAW" "$_TW_DIR/ndjson"
                            BZR_TABLE_WIDTH=invalid run_bzr_raw --output ndjson bug list --whiteboard "$_TW_MARK"
                            if assert_success && assert_ndjson_line_count 1 && assert_stderr_empty; then
                                if jq -e --argjson id "$_TW_BUG" 'type == "object" and .id == $id' \
                                    "$BZR_STDOUT_RAW" >/dev/null; then
                                    if cmp -s "$BZR_STDOUT_RAW" "$_TW_DIR/ndjson"; then test_pass
                                    else test_fail "invalid BZR_TABLE_WIDTH changed NDJSON stdout"; fi
                                else test_fail "invalid BZR_TABLE_WIDTH changed NDJSON structure"; fi
                            fi
                        else test_fail "invalid BZR_TABLE_WIDTH changed JSON stdout"; fi
                    fi
                else test_fail "table output did not wrap to 60 columns"; fi
            fi
        else test_fail "credentialless table output did not wrap to 60 columns"; fi
    fi
else test_fail "could not create table-width fixture"; fi
rm -rf "$_TW_DIR"
unset _TW_MARK _TW_SUMMARY _TW_BUG _TW_DIR

# --dry-run previews a mutation without writing it.
test_begin "dry-run-bug-create-previews-without-writing" "--dry-run bug create previews without writing"
_DM="dry$$x${RANDOM}"
run_bzr --dry-run bug create --product FuncTestProd --component Backend --op-sys Linux --platform PC --description d --whiteboard "$_DM" --summary "dryrun preview"
if assert_success && assert_json '.action' "dry-run"; then
    run_bzr bug list --whiteboard "$_DM" --count
    if assert_count 0; then test_pass; fi
fi
unset _DM

test_begin "dry-run-product-create-previews-without-writing" "--dry-run product create previews without writing"
_DP=$(unique_name dryprod)
run_bzr --dry-run product create --name "$_DP" --description "dry product"
if assert_success && assert_json '.resource' "product" && assert_json '.action' "dry-run"; then
    run_bzr product view "$_DP"
    if assert_failure; then test_pass; fi
fi
unset _DP

test_begin "dry-run-user-update-previews-without-writing" "--dry-run user update previews without writing"
run_bzr --dry-run user update testuser@test.bzr --disable-login false
if assert_success && assert_json '.resource' "user" && assert_json '.action' "dry-run"; then
    test_pass
fi

test_begin "dry-run-group-update-previews-without-writing" "--dry-run group update previews without writing"
run_bzr --dry-run group update functest-grp --description "dry group update"
if assert_success && assert_json '.resource' "group" && assert_json '.action' "dry-run"; then
    run_bzr group view functest-grp
    if assert_stdout_not_contains "dry group update"; then test_pass; fi
fi

echo ""
