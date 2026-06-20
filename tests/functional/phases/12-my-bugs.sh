# 12-my-bugs
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 12: My Bugs
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 12: My Bugs ─────────────────────────────────────────"

test_begin "61. bug my (assigned)"
run_bzr bug my
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "62. bug my --created"
run_bzr bug my --created
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "63. bug my --all"
run_bzr bug my --all
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64. bug my --all --status NEW"
run_bzr bug my --all --status NEW
if assert_success; then test_pass; fi

test_begin "64a. bug my --status multiple (OR)"
run_bzr bug my --all --status NEW --status REOPENED
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64b. bug my --status negation (NOT RESOLVED)"
run_bzr bug my --all --status '!RESOLVED'
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64c. bug my --status mixed positive and negated"
run_bzr bug my --all --status NEW --status '!RESOLVED'
if assert_success; then test_pass; fi

echo ""

