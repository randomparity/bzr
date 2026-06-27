# 05-fields-classifications
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 5: Fields & Classifications
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 5: Fields & Classifications ───────────────────────"

test_begin "16. field list status (alias resolution)"
run_bzr field list status
if assert_success && assert_stdout_contains "CONFIRMED"; then test_pass; fi

test_begin "17. field list priority"
run_bzr field list priority
if assert_success; then test_pass; fi

test_begin "18. field list severity (alias resolution)"
run_bzr field list severity
if assert_success; then test_pass; fi

test_begin "19. field list resolution"
run_bzr field list resolution
if assert_success && assert_stdout_contains "FIXED"; then test_pass; fi

test_begin "19a. field list bug_status (internal name still works)"
run_bzr field list bug_status
if assert_success && assert_stdout_contains "CONFIRMED"; then test_pass; fi

test_begin "19b. field list nonexistent_xyz (error case)"
run_bzr field list nonexistent_xyz
if assert_failure; then test_pass; fi

test_begin "19c. field aliases"
run_bzr field aliases
if assert_success && assert_stdout_contains "status" && assert_stdout_contains "bug_status"; then test_pass; fi

test_begin "20. classification view Unclassified"
run_bzr classification view Unclassified
if assert_success && assert_json '.name' "Unclassified"; then test_pass; fi

test_begin "20a. classification list"
run_bzr classification list
if assert_success && assert_json_array_min_length '.' 1 &&
    assert_json_contains '[.[].name] | join(",")' "Unclassified"; then test_pass; fi

test_begin "20b. field list --fields projects keys"
# Project to `name` only; assert the key set is exactly {name} (element 0 may be
# the null-named default entry, so don't assert a non-null value here).
run_bzr field list status --fields name
if assert_success && assert_json '.[0] | keys == ["name"]' true; then test_pass; fi

test_begin "20c. field list --fields unknown exits 7"
run_bzr field list status --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

test_begin "20d. classification view --fields projects keys"
run_bzr classification view Unclassified --fields id,name
if assert_success && assert_json 'keys | length' 2 && assert_json_exists '.name'; then
    test_pass
fi

test_begin "20e. classification list --fields unknown exits 7"
run_bzr classification list --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

echo ""
