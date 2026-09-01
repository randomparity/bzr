# 11-batch-update
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 11: Batch Update
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 11: Batch Update ────────────────────────────────────"

test_begin "bug-update-batch-two-bugs" "bug update (batch — two bugs)"
if [[ -n "$BUG2" ]] && [[ -n "$BUG4" ]]; then
    run_bzr bug update "$BUG2" "$BUG4" --whiteboard "batch-test"
    if assert_success; then test_pass; fi
else test_skip "no BUG2/BUG4"; fi

test_begin "bug-view-verify-batch-bug2" "bug view (verify batch — bug2)"
if [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG2"
    if assert_success && assert_json '.whiteboard' "batch-test"; then test_pass; fi
else test_skip "no BUG2"; fi

test_begin "bug-view-verify-batch-bug4" "bug view (verify batch — bug4)"
if [[ -n "$BUG4" ]]; then
    run_bzr bug view "$BUG4"
    if assert_success && assert_json '.whiteboard' "batch-test"; then test_pass; fi
else test_skip "no BUG4"; fi

echo ""
