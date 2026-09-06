# 05-fields-classifications
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 5: Fields & Classifications
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 5: Fields & Classifications ───────────────────────"

test_begin "field-list-status-alias-resolution" "field list status (alias resolution)"
run_bzr field list status
if assert_success && assert_stdout_contains "CONFIRMED"; then test_pass; fi

test_begin "field-list-priority" "field list priority"
run_bzr field list priority
if assert_success; then test_pass; fi

test_begin "field-list-severity-alias-resolution" "field list severity (alias resolution)"
run_bzr field list severity
if assert_success; then test_pass; fi

test_begin "field-list-resolution" "field list resolution"
run_bzr field list resolution
if assert_success && assert_stdout_contains "FIXED"; then test_pass; fi

test_begin "field-list-bug-status-internal-name-still-works" "field list bug_status (internal name still works)"
run_bzr field list bug_status
if assert_success && assert_stdout_contains "CONFIRMED"; then test_pass; fi

test_begin "field-list-nonexistent-xyz-error-case" "field list nonexistent_xyz (error case)"
run_bzr field list nonexistent_xyz
if assert_failure; then test_pass; fi

test_begin "field-list-short-desc-no-values" "field list short_desc has no values"
run_bzr_raw --output table field list short_desc
if ! assert_exit_code 0; then
    :
elif [[ $(<"$BZR_STDOUT_RAW") != "No values for field 'short_desc'." ]]; then
    test_fail "expected exact short_desc no-values message"
else
    test_pass
fi

test_begin "field-list-deadline-no-values" "field list deadline has no values"
run_bzr_raw --output table field list deadline
if ! assert_exit_code 0; then
    :
elif [[ $(<"$BZR_STDOUT_RAW") != "No values for field 'deadline'." ]]; then
    test_fail "expected exact deadline no-values message"
else
    test_pass
fi

test_begin "field-list-bug-id-no-values" "field list bug_id has no values"
run_bzr_raw --output table field list bug_id
if ! assert_exit_code 0; then
    :
elif [[ $(<"$BZR_STDOUT_RAW") != "No values for field 'bug_id'." ]]; then
    test_fail "expected exact bug_id no-values message"
else
    test_pass
fi

test_begin "field-aliases" "field aliases"
run_bzr field aliases
if assert_success && assert_stdout_contains "status" && assert_stdout_contains "bug_status"; then test_pass; fi

# `field list` with no argument enumerates the whole accepted --field set: the
# server's catalogue names and the REST names bzr models (ADR 0062, issue #718).
# Asserting BOTH sources appear is what discriminates the union from either half
# alone -- a catalogue-only regression drops every `bzr` row, and a
# BUG_FIELDS-only regression drops every `server` row.
test_begin "field-list-no-argument-lists-both-sources" "field list (no argument) lists both sources"
run_bzr field list
if assert_success &&
    assert_json 'any(.[]; .source == "server")' true &&
    assert_json 'any(.[]; .source == "bzr")' true; then test_pass; fi

# The concrete asymmetry the issue is about: Bugzilla's catalogue reports
# `status_whiteboard`, the write API takes `whiteboard`, and both are accepted.
# Naming the pair makes this fail on the real regression rather than on an
# abstraction of it.
test_begin "field-list-no-argument-marks-internal-and-rest-names" "field list marks internal and REST spellings"
run_bzr field list
if assert_success &&
    assert_json 'map(select(.name == "status_whiteboard")) | .[0].source' "server" &&
    assert_json 'map(select(.name == "whiteboard")) | .[0].source' "bzr"; then test_pass; fi

test_begin "field-list-no-argument-fields-projects-keys" "field list (no argument) --fields projects keys"
run_bzr field list --fields name
if assert_success && assert_json '.[0] | keys == ["name"]' true; then test_pass; fi

# `sort_key` is a valid key of the *named* form and an invalid key of this one,
# so this fails if the handler validates against FIELD_VALUE_FIELDS. A nonsense
# token would be rejected either way and would prove nothing.
test_begin "field-list-no-argument-fields-unknown-exits-7" "field list (no argument) --fields unknown exits 7"
run_bzr field list --fields sort_key
if assert_exit_code 7; then test_pass; fi

# The catalogue is anonymously readable, so the listing must work with no
# credential.
test_begin "credentialless-field-list-no-argument" "credentialless field list (no argument)"
run_bzr_raw --json --server public field list
if assert_success &&
    assert_json 'any(.[]; .source == "server")' true &&
    assert_json 'any(.[]; .source == "bzr")' true; then test_pass; fi

test_begin "classification-view-unclassified" "classification view Unclassified"
run_bzr classification view Unclassified
if assert_success && assert_json '.name' "Unclassified"; then test_pass; fi

test_begin "classification-list" "classification list"
run_bzr classification list
if assert_success && assert_json_array_length '.' 1 && assert_json '.[0].name' "Unclassified" &&
    assert_stderr_contains "Note: only the default 'Unclassified' classification exists; this server likely has classifications disabled."; then test_pass; fi

test_begin "credentialless-classification-list-disabled" "credentialless classification list when disabled"
run_bzr_raw --output table --server public classification list
if ! assert_exit_code 0; then
    :
elif [[ $(<"$BZR_STDOUT_RAW") != "Note: only the default 'Unclassified' classification exists; this server likely has classifications disabled." ]]; then
    test_fail "expected exact disabled-classifications note on stdout"
elif [[ -s "$BZR_STDERR" ]]; then
    test_fail "expected empty stderr for disabled classifications"
else
    test_pass
fi

test_begin "field-list-fields-projects-keys" "field list --fields projects keys"
# Project to `name` only; assert the key set is exactly {name} (element 0 may be
# the null-named default entry, so don't assert a non-null value here).
run_bzr field list status --fields name
if assert_success && assert_json '.[0] | keys == ["name"]' true; then test_pass; fi

test_begin "field-list-fields-unknown-exits-7" "field list --fields unknown exits 7"
run_bzr field list status --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

test_begin "classification-view-fields-projects-keys" "classification view --fields projects keys"
run_bzr classification view Unclassified --fields id,name
if assert_success && assert_json 'keys | length' 2 && assert_json_exists '.name'; then
    test_pass
fi

test_begin "classification-list-fields-unknown-exits-7" "classification list --fields unknown exits 7"
run_bzr classification list --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

echo ""
