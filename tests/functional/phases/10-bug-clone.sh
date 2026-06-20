# 10-bug-clone
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 10: Bug Clone
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 10: Bug Clone ───────────────────────────────────────"

test_begin "54. bug clone (defaults)"
if [[ -n "$BUG3" ]]; then
    # Pass --op-sys and --rep-platform since some Bugzilla versions require them
    # and the Bug struct doesn't include these fields for automatic copying
    run_bzr bug clone "$BUG3" --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then
        CLONE_ID=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG3"; fi

test_begin "55. bug view (verify clone fields)"
if [[ -n "$CLONE_ID" ]]; then
    run_bzr bug view "$CLONE_ID"
    if assert_success && assert_json '.summary' "Clone source bug" && assert_json '.priority' "Highest"; then
        test_pass
    fi
else test_skip "no CLONE_ID"; fi

test_begin "56. bug clone (with overrides)"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3" --summary "Overridden summary" --no-comment --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG3"; fi

test_begin "57. bug clone --add-depends-on"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3" --summary "Depends on source" --add-depends-on --no-cc --no-keywords --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG3"; fi

echo ""

