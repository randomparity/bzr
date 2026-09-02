# 04-components
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 4: Components
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 4: Components ─────────────────────────────────────"

test_begin "component-create" "component create"
run_bzr component create --product FuncTestProd --name Backend --description "Backend component" --default-assignee "$ADMIN_EMAIL"
if [[ $BZR_EXIT -eq 0 ]]; then
    test_pass
elif grep -q "already" "$BZR_STDERR" 2>/dev/null; then
    test_pass # idempotent
else
    assert_success
fi

test_begin "component-update-removed" "component update is not a subcommand"
run_bzr_raw component update
if assert_exit_code 2 && assert_stderr_contains "unrecognized subcommand 'update'"; then
    test_pass
fi

test_begin "component-list-product" "component list --product"
run_bzr component list --product FuncTestProd
if assert_success && assert_json_array_min_length '.' 1 &&
    assert_json_contains '[.[].name] | join(",")' "Backend"; then test_pass; fi

test_begin "component-view-product-component" "component view <product> <component>"
run_bzr component view FuncTestProd Backend
if assert_success && assert_json '.name' "Backend" &&
    assert_json '.default_assignee' "$ADMIN_EMAIL"; then test_pass; fi

test_begin "component-list-fields-projects-keys" "component list --fields projects keys"
run_bzr component list --product FuncTestProd --fields id,name
if assert_success && assert_json '.[0] | keys | length' 2 &&
    assert_json_exists '.[0].name'; then test_pass; fi

test_begin "component-view-fields-unknown-exits-7" "component view --fields unknown exits 7"
run_bzr component view FuncTestProd Backend --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

_CJSON_DIR=$(mktemp -d /tmp/bzr-func-component-json.XXXXXX)
_CJ_NAME=$(unique_name compjson)
write_json_fixture "$_CJSON_DIR/create.json" \
    "{\"product\":\"FuncTestProd\",\"name\":\"$_CJ_NAME\",\"description\":\"component json\",\"default_assignee\":\"$ADMIN_EMAIL\"}"

test_begin "component-create-from-json" "component create --from-json"
run_bzr component create --from-json "$_CJSON_DIR/create.json"
if assert_success; then
    run_bzr component view FuncTestProd "$_CJ_NAME"
    if assert_json '.name' "$_CJ_NAME"; then test_pass; fi
fi

rm -r "$_CJSON_DIR"
unset _CJSON_DIR _CJ_NAME

echo ""
