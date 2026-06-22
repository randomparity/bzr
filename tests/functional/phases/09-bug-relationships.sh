# 09-bug-relationships
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 9: Bug Relationships
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 9: Bug Relationships ────────────────────────────────"

test_begin "49. bug view (verify create relationships)"
if [[ -n "$BUG4" ]]; then
    run_bzr bug view "$BUG4"
    if assert_success && assert_stdout_contains "$BUG1"; then test_pass; fi
else test_skip "no BUG4"; fi

test_begin "50. bug update --blocks-add"
if [[ -n "$BUG2" ]] && [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG2" --blocks-add "$BUG3"
    if assert_success; then test_pass; fi
else test_skip "no BUG2/BUG3"; fi

test_begin "51. bug update --depends-on-add"
# Use BUG3 on BUG3 itself (add BUG2 to BUG3's depends_on — independent of blocks chain)
if [[ -n "$BUG3" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug update "$BUG3" --depends-on-add "$BUG2"
    if assert_success; then test_pass; fi
else test_skip "no BUG3/BUG2"; fi

test_begin "52. bug update --blocks-remove"
if [[ -n "$BUG2" ]] && [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG2" --blocks-remove "$BUG3"
    if assert_success; then test_pass; fi
else test_skip "no BUG2/BUG3"; fi

test_begin "53. bug update --depends-on-remove"
if [[ -n "$BUG3" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug update "$BUG3" --depends-on-remove "$BUG2"
    if assert_success; then test_pass; fi
else test_skip "no BUG3/BUG2"; fi

test_begin "53a. bug update --keywords-add (single keyword)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --keywords-add "fix-needed"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "53b. bug view shows new keyword"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1" --json
    if assert_success && assert_stdout_contains "fix-needed"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "53c. bug update --keywords-remove"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --keywords-remove "fix-needed"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "53d. bug update --cc-add (single user)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --cc-add "testuser@test.bzr"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "53e. bug view shows new cc"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1" --json
    if assert_success && assert_stdout_contains "testuser@test.bzr"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "53f. bug update --cc-remove"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --cc-remove "testuser@test.bzr"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

echo ""
