# 01-config
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 1: Config Commands (no network needed)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 1: Config Commands ────────────────────────────────"

test_begin "1. config set-server test"
run_bzr config set-server test --url "$BZ_URL" --api-key "$API_KEY" --auth-method query_param --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "2. config show"
run_bzr config show
if assert_success; then test_pass; fi

test_begin "3. config set-server alt"
run_bzr config set-server alt --url "http://localhost:9999" --api-key "fake-key-for-alt-server"
if assert_success; then test_pass; fi

test_begin "3a. config set-server auto-detect"
run_bzr config set-server auto --url "$BZ_URL" --api-key "$API_KEY" --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "4. config set-default alt"
run_bzr config set-default alt
if assert_success; then test_pass; fi

test_begin "5. config set-default test (restore)"
run_bzr config set-default test
if assert_success; then test_pass; fi

test_begin "6. config set-default nonexistent (expect failure)"
run_bzr config set-default nonexistent
if assert_exit_code 3; then test_pass; fi

echo ""

