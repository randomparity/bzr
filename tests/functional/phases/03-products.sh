# 03-products
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 3: Products
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 3: Products ───────────────────────────────────────"

test_begin "9. product create"
run_bzr product create --name FuncTestProd --description "Functional test product"
if [[ $BZR_EXIT -eq 0 ]] && assert_json_exists '.id'; then
    test_pass
elif [[ $BZR_EXIT -ne 0 ]] && grep -q "already exists" "$BZR_STDERR" 2>/dev/null; then
    test_pass # idempotent: product exists from a prior run
else
    assert_success # will call test_fail with details
fi

test_begin "10. product list"
run_bzr product list
if assert_success && assert_stdout_contains "FuncTestProd"; then test_pass; fi

test_begin "11. product list --type enterable"
run_bzr product list --type enterable
if assert_success; then test_pass; fi

test_begin "12. product view FuncTestProd"
run_bzr product view FuncTestProd
if assert_success && assert_json '.name' "FuncTestProd"; then test_pass; fi

test_begin "13. product update FuncTestProd"
run_bzr product update FuncTestProd --description "Updated desc"
if assert_success; then test_pass; fi

# Per-run-unique product so create never collides on the non-reset container.
_PV="pv$$x${RANDOM}"

test_begin "13a. product create --version round-trips"
run_bzr product create --name "$_PV" --description "version test" --version "7.7"
if assert_success; then
    run_bzr product view "$_PV"
    if assert_success && assert_json_contains '[.versions[].name] | join(",")' "7.7"; then test_pass; fi
fi

test_begin "13b. product update --is-open false reflects in is_active"
run_bzr product update "$_PV" --is-open false
if assert_success; then
    run_bzr product view "$_PV"
    if assert_json '.is_active' "false"; then test_pass; fi
fi

_PJSON_DIR=$(mktemp -d /tmp/bzr-func-product-json.XXXXXX)
_PJ_NAME=$(unique_name prodjson)
write_json_fixture "$_PJSON_DIR/create.json" \
    "{\"name\":\"$_PJ_NAME\",\"description\":\"product json\",\"version\":\"8.8\",\"is_open\":true}"
write_json_fixture "$_PJSON_DIR/update.json" \
    "{\"name\":\"$_PJ_NAME\",\"description\":\"product json updated\",\"is_open\":false}"
write_json_fixture "$_PJSON_DIR/bad.json" \
    "{\"name\":\"bad\",\"description\":\"bad\",\"unknown\":true}"

test_begin "13c. product create --from-json"
run_bzr product create --from-json "$_PJSON_DIR/create.json"
if assert_success; then
    run_bzr product view "$_PJ_NAME"
    if assert_json '.name' "$_PJ_NAME" &&
        assert_json_contains '[.versions[].name] | join(",")' "8.8"; then test_pass; fi
fi

test_begin "13d. product update --from-json"
run_bzr product update --from-json "$_PJSON_DIR/update.json"
if assert_success; then
    run_bzr product view "$_PJ_NAME"
    if assert_json '.is_active' "false"; then test_pass; fi
fi

test_begin "13e. product create --from-json unknown key"
run_bzr product create --from-json "$_PJSON_DIR/bad.json"
if assert_exit_code 7 && assert_stderr_contains "unknown field"; then test_pass; fi

test_begin "13f. product create --from-json CLI override"
_PJ_OVERRIDE=$(unique_name prodjson-override)
run_bzr product create --from-json "$_PJSON_DIR/create.json" --name "$_PJ_OVERRIDE"
if assert_success; then
    run_bzr product view "$_PJ_OVERRIDE"
    if assert_json '.name' "$_PJ_OVERRIDE"; then test_pass; fi
fi

rm -r "$_PJSON_DIR"
unset _PV _PJSON_DIR _PJ_NAME _PJ_OVERRIDE
echo ""
