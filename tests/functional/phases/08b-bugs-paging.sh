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
_PG_CREATE=(--product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d)

P1=$(make_bug --marker "$_PGMARK" "${_PG_CREATE[@]}" --summary "paging 1")
P2=$(make_bug --marker "$_PGMARK" "${_PG_CREATE[@]}" --summary "paging 2")
P3=$(make_bug --marker "$_PGMARK" "${_PG_CREATE[@]}" --summary "paging 3")
PX=$(make_bug --marker "$_PGOTHER" "${_PG_CREATE[@]}" --summary "paging foreign")

test_begin "135. bug list --count (exact, marker-isolated)"
run_bzr bug list --whiteboard "$_PGMARK" --count
if assert_success && assert_count 3; then test_pass; fi

test_begin "136. bug list --limit caps results"
run_bzr bug list --whiteboard "$_PGMARK" --limit 2
if assert_success && assert_json_array_length '.' 2; then test_pass; fi

test_begin "137. bug list --offset skips into the page"
run_bzr bug list --whiteboard "$_PGMARK" --limit 2 --offset 2
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "138. bug list --paginate returns every page"
run_bzr bug list --whiteboard "$_PGMARK" --paginate --limit 2
if assert_success && assert_json_array_length '.' 3; then test_pass; fi

test_begin "139. bug list --sort bug_id --order desc"
run_bzr bug list --whiteboard "$_PGMARK" --sort bug_id --order desc
if assert_success && assert_json '[.[].id][0]' "$P3" && assert_json '[.[].id][-1]' "$P1"; then test_pass; fi

test_begin "140. bug list --sort bug_id --order asc"
run_bzr bug list --whiteboard "$_PGMARK" --sort bug_id --order asc
if assert_success && assert_json '[.[].id][0]' "$P1" && assert_json '[.[].id][-1]' "$P3"; then test_pass; fi

# Discriminating filter: this run's own bug is present, the foreign-marker bug
# is absent — proving --whiteboard actually filters rather than returning all.
test_begin "141. filter isolates own fixture (present) from foreign (absent)"
run_bzr bug list --whiteboard "$_PGMARK"
if assert_success && assert_json_exists ".[] | select(.id==$P1)" &&
    assert_json "[.[] | select(.id==$PX)] | length" "0"; then test_pass; fi

unset _PGMARK _PGOTHER _PG_CREATE P1 P2 P3 PX
echo ""
