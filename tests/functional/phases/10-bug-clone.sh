# 10-bug-clone
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 10: Bug Clone
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 10: Bug Clone ───────────────────────────────────────"

test_begin "bug-clone-defaults" "bug clone (defaults)"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3"
    if assert_success && assert_json_exists '.id'; then
        CLONE_ID=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG3"; fi

test_begin "bug-view-verify-clone-fields" "bug view (verify clone fields)"
if [[ -n "$CLONE_ID" ]]; then
    run_bzr bug view "$CLONE_ID"
    if assert_success && assert_json '.summary' "Clone source bug" &&
        assert_json '.priority' "Highest" && assert_json '.platform' "PC"; then
        test_pass
    fi
else test_skip "no CLONE_ID"; fi

test_begin "bug-clone-with-overrides" "bug clone (with overrides)"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3" --summary "Overridden summary" --no-comment --op-sys Linux --platform PC
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG3"; fi

test_begin "bug-clone-add-depends-on" "bug clone --add-depends-on"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3" --summary "Depends on source" --add-depends-on --no-cc --no-keywords --op-sys Linux --platform PC
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG3"; fi

test_begin "bug-clone-copies-source-metadata" "bug clone copies source metadata"
if [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG3" --url "http://example.com/source-$BUG3" \
        --whiteboard "clone-source-$BUG3" --target-milestone=--- \
        --deadline 2026-12-29
    if assert_success; then
        run_bzr bug clone "$BUG3" --op-sys Linux --platform PC --no-comment
        if assert_success && assert_json_exists '.id'; then
            _CL_META=$(jq -r '.id' "$BZR_STDOUT")
            run_bzr bug view "$_CL_META"
            if assert_json '.url' "http://example.com/source-$BUG3" &&
                assert_json '.whiteboard' "clone-source-$BUG3" &&
                assert_json '.deadline' "2026-12-29"; then test_pass; fi
        fi
    fi
else test_skip "no BUG3"; fi

test_begin "bug-clone-metadata-overrides" "bug clone metadata overrides"
if [[ -n "$BUG3" ]]; then
    _CL_WB="clone-override-$$"
    run_bzr bug clone "$BUG3" --op-sys Linux --platform PC --no-comment \
        --url "http://example.com/clone-override" --whiteboard "$_CL_WB" \
        --target-milestone=--- --deadline 2026-12-28 \
        --cc "$ADMIN_EMAIL" --flag 'bzr_bug_review?'
    if assert_success && assert_json_exists '.id'; then
        _CL_OVERRIDE=$(jq -r '.id' "$BZR_STDOUT")
        run_bzr bug view "$_CL_OVERRIDE"
        if assert_json '.url' "http://example.com/clone-override" &&
            assert_json '.whiteboard' "$_CL_WB" &&
            assert_json_contains '[.flags[].name] | join(",")' "bzr_bug_review"; then test_pass; fi
    fi
else test_skip "no BUG3"; fi
unset _CL_META _CL_WB _CL_OVERRIDE

echo ""
