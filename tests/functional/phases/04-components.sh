# 04-components
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 4: Components
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 4: Components ─────────────────────────────────────"

test_begin "14. component create"
run_bzr component create --product FuncTestProd --name Backend --description "Backend component" --default-assignee "$ADMIN_EMAIL"
if [[ $BZR_EXIT -eq 0 ]]; then
    COMP_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || echo "")
    test_pass
elif grep -q "already" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent
else
    assert_success
fi

test_begin "15. component update"
# Component update REST endpoint is not available on Bugzilla 5.0 or 5.2
if [[ -n "${COMP_ID:-}" ]] && [[ "$COMP_ID" != "null" ]]; then
    run_bzr component update "$COMP_ID" --description "Updated backend"
    if [[ $BZR_EXIT -eq 0 ]]; then
        test_pass
    elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
        test_skip "component update REST endpoint not available"
    else
        assert_success  # report the actual error
    fi
else
    # Component was already created in a prior run; try to look up the ID
    COMP_ID=$(curl -sf "${BZ_URL}/rest/component?product=FuncTestProd&name=Backend&Bugzilla_api_key=${API_KEY}" 2>/dev/null | jq -r '.components[0].id // empty' 2>/dev/null || echo "")
    if [[ -n "$COMP_ID" ]] && [[ "$COMP_ID" != "null" ]]; then
        run_bzr component update "$COMP_ID" --description "Updated backend"
        if [[ $BZR_EXIT -eq 0 ]]; then
            test_pass
        elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
            test_skip "component update REST endpoint not available"
        else
            assert_success
        fi
    else
        test_skip "no component ID available"
    fi
fi

echo ""

