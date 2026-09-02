# 08b-bugs-paging
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: marker-isolated fixtures only.
# shellcheck shell=bash
#
# Exercises --count / --limit / --offset / --paginate / --sort against a
# per-run-unique, marker-isolated fixture set (Test-Design Contract): exact
# counts and ordering are asserted only over this run's own bugs, never the
# shared corpus, and a foreign-marker bug proves the filter discriminates.

# ══════════════════════════════════════════════════════════════════════
# Phase 8b: Bug paging, counting & sorting (marker-isolated)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8b: Bug paging / count / sort ─────────────────────"

# Per-run-unique markers (PID + $RANDOM); the two must not be substrings of
# each other, since --whiteboard matches by substring.
_PGMARK="pgtest$$x${RANDOM}"
_PGOTHER="pgother$$x${RANDOM}"
_PG_CREATE=(--product FuncTestProd --component Backend --op-sys Linux --platform PC --description d)

P1=$(make_bug --marker "$_PGMARK" "${_PG_CREATE[@]}" --summary "paging 1")
make_bug --marker "$_PGMARK" "${_PG_CREATE[@]}" --summary "paging 2" >/dev/null
P3=$(make_bug --marker "$_PGMARK" "${_PG_CREATE[@]}" --summary "paging 3")
PX=$(make_bug --marker "$_PGOTHER" "${_PG_CREATE[@]}" --summary "paging foreign")

test_begin "bug-list-count-exact-marker-isolated" "bug list --count (exact, marker-isolated)"
run_bzr bug list --whiteboard "$_PGMARK" --count
if assert_success && assert_count 3; then test_pass; fi

test_begin "bug-list-limit-caps-results" "bug list --limit caps results"
run_bzr bug list --whiteboard "$_PGMARK" --limit 2
if assert_success && assert_json_array_length '.' 2; then test_pass; fi

test_begin "bug-list-offset-skips-into-the-page" "bug list --offset skips into the page"
run_bzr bug list --whiteboard "$_PGMARK" --limit 2 --offset 2
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "bug-list-paginate-returns-every-page" "bug list --paginate returns every page"
run_bzr bug list --whiteboard "$_PGMARK" --paginate --limit 2
if assert_success && assert_json_array_length '.' 3; then test_pass; fi

# #462: --progress ndjson streams page/done events on stderr while stdout stays
# the clean 3-element JSON array (proving stdout is unaffected by the flag).
test_begin "bug-list-paginate-progress-ndjson-streams-events-on-stderr" "bug list --paginate --progress ndjson streams events on stderr"
run_bzr bug list --whiteboard "$_PGMARK" --paginate --limit 2 --progress ndjson
if assert_success && assert_json_array_length '.' 3 &&
    assert_stderr_contains '"event":"page"' &&
    assert_stderr_contains '"event":"done"'; then
    _PG_PAGE_COUNT=$(grep -c '"event":"page"' "$BZR_STDERR" 2>/dev/null || true)
    if [[ $_PG_PAGE_COUNT == 3 ]]; then
        test_pass
    else
        test_fail "page event count = ${_PG_PAGE_COUNT:-0}, expected 3"
    fi
fi

test_begin "bug-list-sort-bug-id-order-desc" "bug list --sort bug_id --order desc"
run_bzr bug list --whiteboard "$_PGMARK" --sort bug_id --order desc
if assert_success && assert_json '[.[].id][0]' "$P3" && assert_json '[.[].id][-1]' "$P1"; then test_pass; fi

test_begin "bug-list-sort-bug-id-order-asc" "bug list --sort bug_id --order asc"
run_bzr bug list --whiteboard "$_PGMARK" --sort bug_id --order asc
if assert_success && assert_json '[.[].id][0]' "$P1" && assert_json '[.[].id][-1]' "$P3"; then test_pass; fi

# Discriminating filter: this run's own bug is present, the foreign-marker bug
# is absent — proving --whiteboard actually filters rather than returning all.
test_begin "filter-isolates-own-fixture-present-from-foreign-absent" "filter isolates own fixture (present) from foreign (absent)"
run_bzr bug list --whiteboard "$_PGMARK"
if assert_success && assert_json_exists ".[] | select(.id==$P1)" &&
    assert_json "[.[] | select(.id==$PX)] | length" "0"; then test_pass; fi

# --version filter: two products with distinct, per-run-unique versions prove the
# filter discriminates (present-by-id / absent-by-id). Versions are created via
# `product create --version`, so this needs no container fixture or code change.
_VPA="vpa$$x${RANDOM}"
_VPB="vpb$$x${RANDOM}"
_VVA="va$$x${RANDOM}"
_VVB="vb$$x${RANDOM}"
run_bzr product create --name "$_VPA" --description d --version "$_VVA"
run_bzr component create --product "$_VPA" --name C --description d --default-assignee "$ADMIN_EMAIL"
run_bzr product create --name "$_VPB" --description d --version "$_VVB"
run_bzr component create --product "$_VPB" --name C --description d --default-assignee "$ADMIN_EMAIL"
VBA=$(make_bug --product "$_VPA" --component C --op-sys Linux --platform PC --description d --version "$_VVA" --summary "ver filter a")
VBB=$(make_bug --product "$_VPB" --component C --op-sys Linux --platform PC --description d --version "$_VVB" --summary "ver filter b")

test_begin "bug-list-version-filters-discriminatingly" "bug list --version filters discriminatingly"
run_bzr bug list --version "$_VVA"
if assert_success && assert_json_exists ".[] | select(.id==$VBA)" &&
    assert_json "[.[] | select(.id==$VBB)] | length" "0"; then test_pass; fi

unset _PGMARK _PGOTHER _PG_CREATE _PG_PAGE_COUNT P1 P3 PX _VPA _VPB _VVA _VVB VBA VBB
echo ""
