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

test_begin "57a. bug clone copies source metadata"
if [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG3" --url "http://example.com/source-$BUG3" \
        --whiteboard "clone-source-$BUG3" --target-milestone=--- \
        --deadline 2026-12-29
    if assert_success; then
        run_bzr bug clone "$BUG3" --op-sys Linux --rep-platform PC --no-comment
        if assert_success && assert_json_exists '.id'; then
            _CL_META=$(jq -r '.id' "$BZR_STDOUT")
            run_bzr bug view "$_CL_META"
            if assert_json '.url' "http://example.com/source-$BUG3" &&
                assert_json '.whiteboard' "clone-source-$BUG3" &&
                assert_json '.deadline' "2026-12-29"; then test_pass; fi
        fi
    fi
else test_skip "no BUG3"; fi

test_begin "57b. bug clone metadata overrides"
if [[ -n "$BUG3" ]]; then
    _CL_WB="clone-override-$$"
    run_bzr bug clone "$BUG3" --op-sys Linux --rep-platform PC --no-comment \
        --url "http://example.com/clone-override" --whiteboard "$_CL_WB" \
        --target-milestone=--- --deadline 2026-12-28 \
        --cc "$ADMIN_EMAIL" --flag 'review?'
    if assert_success && assert_json_exists '.id'; then
        _CL_OVERRIDE=$(jq -r '.id' "$BZR_STDOUT")
        run_bzr bug view "$_CL_OVERRIDE"
        if assert_json '.url' "http://example.com/clone-override" &&
            assert_json '.whiteboard' "$_CL_WB" &&
            assert_json_contains '[.flags[].name] | join(",")' "review"; then test_pass; fi
    fi
else test_skip "no BUG3"; fi
unset _CL_META _CL_WB _CL_OVERRIDE

echo ""
