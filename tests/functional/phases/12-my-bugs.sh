# 12-my-bugs
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 12: My Bugs
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 12: My Bugs ─────────────────────────────────────────"

test_begin "bug-my-assigned" "bug my (assigned)"
run_bzr bug my
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-my-zero-offset" "bug my preserves a valid zero-offset window"
run_bzr bug my --limit 1 --offset 0
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "bug-my-created" "bug my --created"
run_bzr bug my --created
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-my-all" "bug my --all"
run_bzr bug my --all
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-my-all-status-new" "bug my --all --status NEW"
run_bzr bug my --all --status NEW
if assert_success; then test_pass; fi

test_begin "bug-my-status-multiple-or" "bug my --status multiple (OR)"
run_bzr bug my --all --status NEW --status REOPENED
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-my-status-negation-not-resolved" "bug my --status negation (NOT RESOLVED)"
run_bzr bug my --all --status '!RESOLVED'
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-my-status-mixed-positive-and-negated" "bug my --status mixed positive and negated"
run_bzr bug my --all --status NEW --status '!RESOLVED'
if assert_success; then test_pass; fi

test_begin "bug-my-product-component" "bug my --product --component"
run_bzr bug my --all --product FuncTestProd --component Backend
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-my-priority-severity" "bug my --priority --severity"
_MY_PS_MARK=$(unique_name my-priority)
_MY_PS_ID=$(make_bug --marker "$_MY_PS_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --platform PC --description d --summary "my priority filter" \
    --priority High --severity major)
run_bzr bug my --all --priority High --severity major --whiteboard "$_MY_PS_MARK"
if assert_success && assert_stdout_contains "$_MY_PS_ID"; then test_pass; fi

test_begin "bug-my-whiteboard-url" "bug my --whiteboard --url"
_MY_MARK="my-filter-$$"
_MY_ID=$(make_bug --marker "$_MY_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --platform PC --description d --summary "my filter" \
    --url "http://example.com/$_MY_MARK")
run_bzr bug my --all --whiteboard "$_MY_MARK" --url "$_MY_MARK"
if assert_success && assert_stdout_contains "$_MY_ID"; then test_pass; fi

test_begin "bug-my-changed-since-malformed" "bug my --changed-since malformed"
run_bzr bug my --changed-since "not-a-date"
if assert_exit_code 7; then test_pass; fi
unset _MY_PS_MARK _MY_PS_ID _MY_MARK _MY_ID

echo ""
