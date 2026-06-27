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
    test_pass # idempotent
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
        assert_success # report the actual error
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

test_begin "15a. component list --product"
run_bzr component list --product FuncTestProd
if assert_success && assert_json_array_min_length '.' 1 &&
    assert_json_contains '[.[].name] | join(",")' "Backend"; then test_pass; fi

test_begin "15b. component view <product> <component>"
run_bzr component view FuncTestProd Backend
if assert_success && assert_json '.name' "Backend"; then test_pass; fi

test_begin "15c. component list --fields projects keys"
run_bzr component list --product FuncTestProd --fields id,name
if assert_success && assert_json '.[0] | keys | length' 2 &&
    assert_json_exists '.[0].name'; then test_pass; fi

test_begin "15d. component view --fields unknown exits 7"
run_bzr component view FuncTestProd Backend --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

_CJSON_DIR=$(mktemp -d /tmp/bzr-func-component-json.XXXXXX)
_CJ_NAME=$(unique_name compjson)
write_json_fixture "$_CJSON_DIR/create.json" \
    "{\"product\":\"FuncTestProd\",\"name\":\"$_CJ_NAME\",\"description\":\"component json\",\"default_assignee\":\"$ADMIN_EMAIL\"}"
write_json_fixture "$_CJSON_DIR/update-by-name.json" \
    "{\"product\":\"FuncTestProd\",\"component\":\"$_CJ_NAME\",\"description\":\"component json updated\"}"

test_begin "15c. component create --from-json"
run_bzr component create --from-json "$_CJSON_DIR/create.json"
if assert_success; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.name' "$_CJ_NAME"; then test_pass; fi
fi

test_begin "15d. component update --product --component target"
run_bzr component update --product FuncTestProd --component "$_CJ_NAME" \
    --description "component named target updated"
if [[ $BZR_EXIT -eq 0 ]]; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.description' "component named target updated"; then test_pass; fi
elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
    test_skip "component update REST endpoint not available"
else
    assert_success
fi

test_begin "15e. component update --from-json named target"
run_bzr component update --from-json "$_CJSON_DIR/update-by-name.json"
if [[ $BZR_EXIT -eq 0 ]]; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.description' "component json updated"; then test_pass; fi
elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
    test_skip "component update REST endpoint not available"
else
    assert_success
fi

rm -r "$_CJSON_DIR"
unset _CJSON_DIR _CJ_NAME

echo ""
