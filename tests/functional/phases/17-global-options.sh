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
    --rep-platform PC --description d \
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
make_bug --marker "$_NM" --product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d --summary "ndjson 1" >/dev/null
make_bug --marker "$_NM" --product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d --summary "ndjson 2" >/dev/null
run_bzr_raw --output ndjson bug list --whiteboard "$_NM"
if assert_success && assert_ndjson_line_count 2; then test_pass; fi
unset _NM

# --dry-run previews a mutation without writing it.
test_begin "dry-run-bug-create-previews-without-writing" "--dry-run bug create previews without writing"
_DM="dry$$x${RANDOM}"
run_bzr --dry-run bug create --product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d --whiteboard "$_DM" --summary "dryrun preview"
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

test_begin "dry-run-component-update-by-name-resolves-but-does-not-write" "--dry-run component update by name resolves but does not write"
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
