# 14-queries
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 13.5: Saved Queries
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 13.5: Saved Queries ─────────────────────────────────"

# ── CRUD lifecycle ───────────────────────────────────────────────────

test_begin "query-save-list-kind" "query save (list kind)"
run_bzr query save prod-bugs --product FuncTestProd --status NEW --status CONFIRMED --limit 10
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "query-save-search-kind" "query save (search kind)"
run_bzr query save search-bugs --search "Bug one" --limit 5
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "query-save-multi-filter" "query save (multi-filter)"
run_bzr query save complex --product FuncTestProd --component Backend --priority Normal --severity normal --status NEW --status CONFIRMED --limit 20
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "query-list" "query list"
run_bzr_raw query list
if assert_success && assert_stdout_contains "prod-bugs" && assert_stdout_contains "search-bugs" && assert_stdout_contains "complex"; then test_pass; fi

test_begin "query-show" "query show"
run_bzr query show complex
if assert_success && assert_json '.kind' "list" && assert_json '.product[0]' "FuncTestProd" && assert_json '.priority[0]' "Normal"; then test_pass; fi

test_begin "query-save-update-existing" "query save (update existing)"
run_bzr query save prod-bugs --product FuncTestProd --status NEW --limit 5
if assert_success && assert_json '.action' "updated"; then test_pass; fi

# ── Run queries against real Bugzilla ────────────────────────────────

test_begin "query-run-product-status-filter" "query run (product+status filter)"
run_bzr query run prod-bugs
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "query-run-quicksearch" "query run (quicksearch)"
run_bzr query run search-bugs
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "query-run-multi-filter-complex" "query run (multi-filter complex)"
run_bzr query run complex
if assert_success; then test_pass; fi

test_begin "query-run-with-limit-override" "query run with limit override"
run_bzr query run prod-bugs --limit 1
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "query-run-nonzero-offset" "query run honors a nonzero offset"
run_bzr query run prod-bugs --limit 1 --offset 0
if assert_success && assert_json_array_length '.' 1; then
    _Q_FIRST_ID=$(jq -r '.[0].id' "$BZR_STDOUT")
    run_bzr query run prod-bugs --limit 1 --offset 1
    if assert_success && assert_json_array_length '.' 1; then
        _Q_OFFSET_ID=$(jq -r '.[0].id' "$BZR_STDOUT")
        if [[ "$_Q_OFFSET_ID" != "$_Q_FIRST_ID" ]]; then
            test_pass
        else
            test_fail "nonzero offset returned the first query result again"
        fi
    fi
fi
unset _Q_FIRST_ID _Q_OFFSET_ID

test_begin "query-run-with-fields-override" "query run with fields override"
run_bzr query run prod-bugs --fields id,summary,status
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "query-run-count" "query run --count"
_QCOUNT_MARK=$(unique_name query-count)
make_bug --marker "$_QCOUNT_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --platform PC --description d --summary "query count 1" >/dev/null
make_bug --marker "$_QCOUNT_MARK" --product FuncTestProd --component Backend \
    --op-sys Linux --platform PC --description d --summary "query count 2" >/dev/null
run_bzr query save count-bugs --product FuncTestProd --whiteboard "$_QCOUNT_MARK" --limit 1
if assert_success && assert_json '.action' "saved"; then
    run_bzr query run count-bugs --count
    if assert_success && assert_count 2; then test_pass; fi
fi
unset _QCOUNT_MARK

test_begin "query-update-from-url" "query update --from-url"
_Q_URL="${BZ_URL}/buglist.cgi?product=FuncTestProd&component=Backend&bug_status=NEW&query_format=advanced"
run_bzr query update complex --from-url "$_Q_URL" --limit 2
if assert_success; then
    run_bzr query show complex
    if assert_json '.product[0]' "FuncTestProd" &&
        assert_json '.component[0]' "Backend" &&
        assert_json '.limit' "2"; then test_pass; fi
fi
unset _Q_URL

test_begin "weekly-status-query-collection-shape" "weekly-status query collection shape"
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

test_begin "query-delete" "query delete"
run_bzr query delete search-bugs
if assert_success && assert_json '.action' "deleted"; then test_pass; fi

test_begin "query-show-deleted-expect-failure" "query show (deleted, expect failure)"
run_bzr query show search-bugs
if assert_failure; then test_pass; fi

test_begin "query-run-deleted-expect-failure" "query run (deleted, expect failure)"
run_bzr query run search-bugs
if assert_failure; then test_pass; fi

test_begin "query-save-empty-expect-failure" "query save (empty, expect failure)"
run_bzr query save empty-q
if assert_failure; then test_pass; fi

test_begin "query-delete-remaining" "query delete remaining"
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
