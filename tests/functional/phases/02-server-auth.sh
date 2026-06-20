# 02-server-auth
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 2: Server & Auth
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 2: Server & Auth ──────────────────────────────────"

test_begin "7. server info"
run_bzr server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "8. whoami"
run_bzr whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi

test_begin "8a. --server auto whoami"
run_bzr_raw --json --server auto whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi

echo ""

