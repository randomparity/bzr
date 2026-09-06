# 16b-attachments-private
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 15b: Private attachment visibility (#133 hybrid fallback)
# ══════════════════════════════════════════════════════════════════════
# The fixture entrypoints already configure insidergroup (for #125),
# which is also the precondition for private attachments existing here.
#
# Every dispatch mode is covered, REST included: ADR-0059 measured REST
# as returning private attachments on Bugzilla 5.0.6, 5.2 and 5.3.3+
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
# those versions and is complete everywhere. ADR-0059 records both. The private
# download cases below cover Hybrid and XML-RPC only; the REST arm of
# `get_attachment` is evidenced by ADR-0059's measurement, not here.
#
# The REST arm additionally omits the `data` field by design
# (exclude_fields); these assertions check entry visibility, not bodies.
echo "── Phase 15b: Private attachments (all dispatch modes) ───────"

test_begin "attachment-upload-private" "attachment upload --private"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
        --summary "Private test attachment" --private
    if assert_success && assert_json_exists '.id'; then
        PRIVATE_ATTACH_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || echo "")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-returns-private-attachment-in-hybrid-mode" "attachment list returns private attachment in Hybrid mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api hybrid attachment list "$BUG1"
    # Several public attachments are uploaded earlier in the run and this
    # section adds one private; the list must include ≥3 total AND the
    # private one must be visible (is_private: true present).
    if assert_success &&
        assert_json_array_min_length '.' 3 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-returns-private-attachment-in-xml-rpc-mode" "attachment list returns private attachment in XML-RPC mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api xmlrpc attachment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 3 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-returns-private-attachment-in-default-mode" "attachment list returns private attachment in default mode"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 3 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-returns-private-attachment-in-rest-mode" "attachment list returns private attachment in REST mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api rest attachment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 3 &&
        assert_json_array_min_length '[.[] | select(.is_private == true)]' 1; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-download-private-in-hybrid-mode" "attachment download (private) in Hybrid mode"
if [[ -n "${PRIVATE_ATTACH_ID:-}" ]] && [[ "$PRIVATE_ATTACH_ID" != "null" ]]; then
    rm -f "$FUNC_PRIVATE_HYBRID_FILE"
    run_bzr --api hybrid attachment download "$PRIVATE_ATTACH_ID" \
        --out "$FUNC_PRIVATE_HYBRID_FILE"
    if assert_success && assert_file_contains "$FUNC_PRIVATE_HYBRID_FILE" "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no private attachment ID"
fi

test_begin "attachment-download-private-in-xml-rpc-mode" "attachment download (private) in XML-RPC mode"
if [[ -n "${PRIVATE_ATTACH_ID:-}" ]] && [[ "$PRIVATE_ATTACH_ID" != "null" ]]; then
    rm -f "$FUNC_PRIVATE_XMLRPC_FILE"
    run_bzr --api xmlrpc attachment download "$PRIVATE_ATTACH_ID" \
        --out "$FUNC_PRIVATE_XMLRPC_FILE"
    if assert_success && assert_file_contains "$FUNC_PRIVATE_XMLRPC_FILE" "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no private attachment ID"
fi

echo ""
