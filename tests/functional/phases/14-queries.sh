# 14-queries
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 13.5: Saved Queries
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 13.5: Saved Queries ─────────────────────────────────"

# ── CRUD lifecycle ───────────────────────────────────────────────────

test_begin "72. query save (list kind)"
run_bzr query save prod-bugs --product FuncTestProd --status NEW --status CONFIRMED --limit 10
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "73. query save (search kind)"
run_bzr query save search-bugs --search "Bug one" --limit 5
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "74. query save (multi-filter)"
run_bzr query save complex --product FuncTestProd --component Backend --priority Normal --severity normal --status NEW --status CONFIRMED --limit 20
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "75. query list"
run_bzr_raw query list
if assert_success && assert_stdout_contains "prod-bugs" && assert_stdout_contains "search-bugs" && assert_stdout_contains "complex"; then test_pass; fi

test_begin "76. query show"
run_bzr query show complex
if assert_success && assert_json '.kind' "list" && assert_json '.product[0]' "FuncTestProd" && assert_json '.priority[0]' "Normal"; then test_pass; fi

test_begin "77. query save (update existing)"
run_bzr query save prod-bugs --product FuncTestProd --status NEW --limit 5
if assert_success && assert_json '.action' "updated"; then test_pass; fi

# ── Run queries against real Bugzilla ────────────────────────────────

test_begin "78. query run (product+status filter)"
run_bzr query run prod-bugs
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "79. query run (quicksearch)"
run_bzr query run search-bugs
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "80. query run (multi-filter complex)"
run_bzr query run complex
if assert_success; then test_pass; fi

test_begin "81. query run with limit override"
run_bzr query run prod-bugs --limit 1
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "82. query run with fields override"
run_bzr query run prod-bugs --fields id,summary,status
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

# ── Cleanup and error handling ───────────────────────────────────────

test_begin "83. query delete"
run_bzr query delete search-bugs
if assert_success && assert_json '.action' "deleted"; then test_pass; fi

test_begin "84. query show (deleted, expect failure)"
run_bzr query show search-bugs
if assert_failure; then test_pass; fi

test_begin "85. query run (deleted, expect failure)"
run_bzr query run search-bugs
if assert_failure; then test_pass; fi

test_begin "86. query save (empty, expect failure)"
run_bzr query save empty-q
if assert_failure; then test_pass; fi

test_begin "87. query delete remaining"
run_bzr query delete prod-bugs
if assert_success; then
    run_bzr query delete complex
    if assert_success; then test_pass; fi
fi

echo ""

