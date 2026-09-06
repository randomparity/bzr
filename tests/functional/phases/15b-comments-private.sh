# 15b-comments-private
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 14b: Private comment visibility (#125 hybrid fallback)
# ══════════════════════════════════════════════════════════════════════
# The fixture entrypoints set insidergroup=admin so the admin test user
# can mark comments private — without that, private comments could not
# exist here and there would be nothing to assert.
#
# Every dispatch mode is covered, REST included: ADR-0059 measured REST
# as returning private comments on Bugzilla 5.0.6, 5.2 and 5.3.3+
# whenever the server honoured the credential.
#
# Two separate facts about the failing case, both from ADR-0059:
#   - The forced-REST arms pass only because 01-config.sh pins the
#     shared `test` server to --auth-method query_param. Under the
#     header auth bzr selects otherwise, they return the public subset
#     on 5.0.6 and 5.2 alike, since neither honours the header on REST.
#   - Only >= 5.1 servers are exposed in DEFAULT mode, because
#     version_to_api_mode maps 5.0.x to Hybrid and so dispatches these
#     reads XML-RPC-first there.
# Not covered here, deliberately: no case exercises the unpinned auth
# method, so the out-of-the-box 5.2 loss is recorded in ADR-0059 and
# tracked by issue #713, not by any assertion below.
echo "── Phase 14b: Private comments (all dispatch modes) ──────────"

test_begin "comment-add-private" "comment add --private"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "Private test comment" --private
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "comment-list-returns-private-comment-in-hybrid-mode" "comment list returns private comment in Hybrid mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api hybrid comment list "$BUG1"
    # 1 description (count 0) + 2 public + 1 private = >= 4
    # AND the private one must be visible (is_private: true present).
    if assert_success &&
        assert_json_array_min_length '.' 4 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "comment-list-returns-private-comment-in-xml-rpc-mode" "comment list returns private comment in XML-RPC mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api xmlrpc comment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 4 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "comment-list-returns-private-comment-in-default-mode" "comment list returns private comment in default mode"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 4 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "comment-list-returns-private-comment-in-rest-mode" "comment list returns private comment in REST mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api rest comment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 4 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

echo ""
