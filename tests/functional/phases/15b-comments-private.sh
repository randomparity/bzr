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
#   - The forced-REST arms pass because 01-config.sh pins the shared
#     `test` server to --auth-method query_param. Pin header instead
#     and they return the public subset on 5.0.6 and 5.2 alike, since
#     neither honours that header on REST.
#   - Under a header auth_method, 5.2 loses it in DEFAULT mode too,
#     because version_to_api_mode maps 5.2 to Rest; 5.0.x maps to
#     Hybrid and dispatches these reads XML-RPC-first, so 5.0.6 keeps
#     them in default mode, and 5.3.3+ honours the header.
# Not covered here, deliberately: no case uses a header auth_method,
# the one configuration that still loses private content over REST
# (5.0.6 and 5.2 only). It is reached either by pinning
# --auth-method header, or by inheriting one a pre-#713 bzr persisted,
# which upgrading does not revisit (ADR-0056 owns that population).
# Since #713 merged, a freshly written config persists query_param on
# those versions and is complete everywhere. ADR-0059 records both.
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
