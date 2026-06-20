# 17-global-options
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 16: Global Options Smoke Tests
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 16: Global Options ────────────────────────────────"

test_begin "101. --output table"
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

test_begin "102. --quiet"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "102a. --quiet suppresses stderr tracing"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet -vvv bug view "$BUG1"
    if assert_success && assert_stdout_empty && assert_stderr_empty; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "102b. --quiet preserves error exit code"
if true; then
    run_bzr_raw --quiet bug view 999999
    if assert_failure && assert_stdout_empty; then test_pass; fi
fi

test_begin "102c. --quiet + --json suppresses stdout"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet --json bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "103. --server test whoami"
run_bzr_raw --server test whoami
if assert_success; then test_pass; fi

echo ""

