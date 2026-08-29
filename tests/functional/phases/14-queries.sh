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

test_begin "82a. query run --count"
_QCOUNT_MARK=$(unique_name query-count)
make_bug --marker "$_QCOUNT_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --rep-platform PC --description d --summary "query count 1" >/dev/null
make_bug --marker "$_QCOUNT_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --rep-platform PC --description d --summary "query count 2" >/dev/null
run_bzr query save count-bugs --product FuncTestProd --whiteboard "$_QCOUNT_MARK" --limit 1
if assert_success && assert_json '.action' "saved"; then
    run_bzr query run count-bugs --count
    if assert_success && assert_count 2; then test_pass; fi
fi
unset _QCOUNT_MARK

test_begin "82b. query update --from-url"
_Q_URL="${BZ_URL}/buglist.cgi?product=FuncTestProd&component=Backend&bug_status=NEW&query_format=advanced"
run_bzr query update complex --from-url "$_Q_URL" --limit 2
if assert_success; then
    run_bzr query show complex
    if assert_json '.product[0]' "FuncTestProd" &&
        assert_json '.component[0]' "Backend" &&
        assert_json '.limit' "2"; then test_pass; fi
fi
unset _Q_URL

test_begin "82c. weekly-status query collection shape"
_Q_WEEKLY_URL="${BZ_URL}/buglist.cgi?product=FuncTestProd&query_format=advanced"
run_bzr query save weekly-status-fixture --from-url "$_Q_WEEKLY_URL"
if assert_success; then
    run_bzr query run weekly-status-fixture \
        --fields id,summary,status,resolution,assigned_to,priority,severity,target_milestone,deadline,last_change_time,whiteboard,blocks,depends_on \
        --paginate
    if assert_success && assert_json_array_min_length '.' 1 &&
        assert_json '.[0] | has("id")' "true" &&
        assert_json '.[0] | has("last_change_time")' "true" &&
        assert_json '.[0] | has("blocks")' "true" &&
        assert_json '.[0] | has("depends_on")' "true"; then
        run_bzr bug history "$BUG1"
        if assert_success && assert_json_array_min_length '.' 1 &&
            assert_json '.[0] | has("field")' "true" &&
            assert_json '.[0] | has("when")' "true"; then test_pass; fi
    fi
fi
unset _Q_WEEKLY_URL

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
    run_bzr query delete count-bugs
    if assert_success; then
        run_bzr query delete complex
        if assert_success; then test_pass; fi
    fi
fi
run_bzr query delete weekly-status-fixture
[ "$BZR_EXIT" -eq 0 ] || test_fail "weekly-status fixture query cleanup failed"

echo ""
