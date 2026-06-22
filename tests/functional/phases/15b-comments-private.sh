# 15b-comments-private
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 14b: Private comment visibility (#125 hybrid fallback)
# ══════════════════════════════════════════════════════════════════════
# Mirrors the issue reporter's deployment, which has `insidergroup`
# configured (otherwise private comments couldn't exist there in the
# first place). The fixture entrypoints set insidergroup=admin so the
# admin test user can mark comments private — this is what makes #125
# reproducible at all.
echo "── Phase 14b: Private comments (Hybrid mode) ─────────────────"

test_begin "94a. comment add --private"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "Private test comment" --private
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "94b. comment list returns private comment in Hybrid mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api hybrid comment list "$BUG1"
    # 1 description (count 0) + 2 public + 1 private = >= 4
    # AND the private one must be visible (is_private: true present).
    if assert_success &&
        assert_json_array_min_length '.' 4 &&
        [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "94c. comment list returns private comment in XML-RPC mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api xmlrpc comment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 4 &&
        [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

echo ""
