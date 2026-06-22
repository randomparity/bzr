# 13-templates
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 13: Templates
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 13: Templates ───────────────────────────────────────"

test_begin "65. template save"
run_bzr template save func-tmpl --product FuncTestProd --component Backend --priority Normal --severity normal
if assert_success; then test_pass; fi

test_begin "66. template list"
run_bzr_raw template list
if assert_success && assert_stdout_contains "func-tmpl"; then test_pass; fi

test_begin "67. template show"
run_bzr template show func-tmpl
if assert_success && assert_json '.product' "FuncTestProd"; then test_pass; fi

test_begin "68. bug create --template"
run_bzr bug create --template func-tmpl --summary "Bug from template" \
    --description "Description from template test" --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    TMPL_BUG=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "69. bug view (verify template fields)"
if [[ -n "$TMPL_BUG" ]]; then
    run_bzr bug view "$TMPL_BUG"
    if assert_success && assert_json '.product' "FuncTestProd" && assert_json '.component' "Backend" && assert_json '.priority' "Normal"; then
        test_pass
    fi
else test_skip "no TMPL_BUG"; fi

test_begin "69a. template save metadata fields"
run_bzr template save meta-tmpl --product FuncTestProd --component Backend \
    --priority Normal --severity normal --url "http://example.com/template" \
    --whiteboard "template-wb" --target-milestone=--- --deadline 2026-12-27 \
    --cc "$ADMIN_EMAIL" --flag 'bzr_bug_review?'
if assert_success; then
    run_bzr template show meta-tmpl
    if assert_json '.url' "http://example.com/template" &&
        assert_json '.whiteboard' "template-wb" &&
        assert_json '.deadline' "2026-12-27"; then test_pass; fi
fi

test_begin "69b. bug create --template metadata applies"
run_bzr bug create --template meta-tmpl --summary "Bug from meta template" \
    --description "Description from meta template" --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    _TMETA_BUG=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$_TMETA_BUG"
    if assert_json '.url' "http://example.com/template" &&
        assert_json '.whiteboard' "template-wb" &&
        assert_json_contains '[.flags[].name] | join(",")' "bzr_bug_review"; then test_pass; fi
fi

test_begin "69c. template update --clear metadata"
run_bzr template update meta-tmpl --clear url --clear whiteboard --cc "$ADMIN_EMAIL"
if assert_success; then
    run_bzr template show meta-tmpl
    if assert_json '.url' "null" &&
        assert_json '.whiteboard' "null" &&
        assert_json_contains '.cc | join(",")' "$ADMIN_EMAIL"; then test_pass; fi
fi

test_begin "69d. template delete metadata template"
run_bzr template delete meta-tmpl
if assert_success; then
    run_bzr template show meta-tmpl
    if assert_failure; then test_pass; fi
fi
unset _TMETA_BUG

test_begin "70. template delete"
run_bzr template delete func-tmpl
if assert_success; then test_pass; fi

test_begin "71. template show (deleted, expect failure)"
run_bzr template show func-tmpl
if assert_failure; then test_pass; fi

echo ""
