# 16b-attachments-private
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 15b: Private attachment visibility (#133 hybrid fallback)
# ══════════════════════════════════════════════════════════════════════
# Mirrors the issue reporter's deployment: REST silently filters private
# attachments under non-admin scope, while XML-RPC Bug.attachments returns
# the full set. The fixture entrypoints already configure insidergroup
# (for #125), which is also the precondition for private attachments.
echo "── Phase 15b: Private attachments (Hybrid mode) ──────────────"

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
        [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-returns-private-attachment-in-xml-rpc-mode" "attachment list returns private attachment in XML-RPC mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api xmlrpc attachment list "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 3 &&
        [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
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
