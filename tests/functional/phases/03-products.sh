# 03-products
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 3: Products
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 3: Products ───────────────────────────────────────"

test_begin "product-create" "product create"
run_bzr product create --name FuncTestProd --description "Functional test product"
if [[ $BZR_EXIT -eq 0 ]] && assert_json_exists '.id'; then
    test_pass
elif [[ $BZR_EXIT -ne 0 ]] && grep -q "already exists" "$BZR_STDERR" 2>/dev/null; then
    test_pass # idempotent: product exists from a prior run
else
    assert_success # will call test_fail with details
fi

test_begin "product-list" "product list"
run_bzr product list
if assert_success && assert_stdout_contains "FuncTestProd"; then test_pass; fi

test_begin "product-list-type-enterable" "product list --type enterable"
run_bzr product list --type enterable
if assert_success; then test_pass; fi

test_begin "product-view-functestprod" "product view FuncTestProd"
run_bzr product view FuncTestProd
if assert_success && assert_json '.name' "FuncTestProd"; then test_pass; fi

test_begin "product-view-fields-projects-keys" "product view --fields projects keys"
run_bzr product view FuncTestProd --fields id,name
if assert_success && assert_json 'keys | length' 2 && assert_json_exists '.name'; then
    test_pass
fi

test_begin "product-list-help-type-semantics" "product list --help describes --type correctly"
run_bzr product list --help
if assert_success &&
    assert_stdout_contains "selectable\` -- products the caller can choose when querying" &&
    assert_stdout_contains "the caller can file a new bug against" &&
    assert_stdout_not_contains "selectable\` -- products the caller can file bugs against"; then
    test_pass
fi

test_begin "product-view-help-matches-output" "product view --help matches format_product_detail"
run_bzr product view --help
if assert_success &&
    assert_stdout_contains "Prints the product's description, the list of components" &&
    assert_stdout_not_contains "classification" &&
    assert_stdout_not_contains "CC lists"; then
    test_pass
fi

test_begin "product-update-help-attributes-rename-to-bzr" "product update --help attributes missing rename to bzr"
run_bzr product update --help
if assert_success &&
    assert_stdout_contains "\`bzr\` does not support renaming a product" &&
    assert_stdout_not_contains "not supported by the"; then
    test_pass
fi

test_begin "product-list-fields-unknown-exits-7" "product list --fields unknown exits 7"
run_bzr product list --fields bogus_xyz
if assert_exit_code 7; then test_pass; fi

test_begin "production-shaped-product-and-field-metadata" "production-shaped product and field metadata"
if redhat_shape_start "$BZ_PORT"; then
    trap 'cleanup; redhat_shape_stop' EXIT
    _PP_PROXY_URL="http://127.0.0.1:${REDHAT_SHAPE_PORT}"
    _PP_OK=1
    # The exact invocation that failed with exit 8 against bugzilla.kernel.org.
    run_bzr_raw --json --server-url "$_PP_PROXY_URL" \
        product list --type accessible --fields id,name,is_active
    if [[ $BZR_EXIT -ne 0 ]] || ! jq -e 'length > 0' "$BZR_STDOUT" >/dev/null; then
        _PP_OK=0
    fi
    run_bzr_raw --json --server-url "$_PP_PROXY_URL" \
        product list --fields id,name
    if [[ $BZR_EXIT -ne 0 ]] || ! jq -e 'length > 0' "$BZR_STDOUT" >/dev/null; then
        _PP_OK=0
    fi
    run_bzr_raw --json --server-url "$_PP_PROXY_URL" field list status
    if [[ $BZR_EXIT -ne 0 ]] ||
        ! jq -e 'any(.[]; .sort_key != null and .sort_key < 0)' \
            "$BZR_STDOUT" >/dev/null; then
        _PP_OK=0
    fi
    run_bzr_raw --json --server-url "$_PP_PROXY_URL" product list --type accessible
    if [[ $BZR_EXIT -ne 0 ]] ||
        ! jq -e 'any(.[]; any((.versions // [])[]; .sort_key != null and .sort_key < 0) or any((.milestones // [])[]; .sort_key != null and .sort_key < 0))' \
            "$BZR_STDOUT" >/dev/null; then
        _PP_OK=0
    fi
    _PP_FIELD_COUNT=$(awk \
        '/metadata-sort-keys shaped route=field count=[1-9][0-9]*/ { count++ } END { print count + 0 }' \
        "$REDHAT_SHAPE_LOG")
    _PP_PRODUCT_COUNT=$(awk \
        '/metadata-sort-keys shaped route=product count=[1-9][0-9]*/ { count++ } END { print count + 0 }' \
        "$REDHAT_SHAPE_LOG")
    if [[ $_PP_FIELD_COUNT -lt 1 ]] || [[ $_PP_PRODUCT_COUNT -lt 1 ]]; then
        _PP_OK=0
    fi
    run_bzr_raw --json --server-url "$_PP_PROXY_URL" server capabilities
    _PP_FIELD_COUNT_AFTER=$(awk \
        '/metadata-sort-keys shaped route=field count=[1-9][0-9]*/ { count++ } END { print count + 0 }' \
        "$REDHAT_SHAPE_LOG")
    if [[ $BZR_EXIT -ne 0 ]] || [[ $_PP_FIELD_COUNT_AFTER -le $_PP_FIELD_COUNT ]]; then
        _PP_OK=0
    fi
    redhat_shape_stop || _PP_OK=0
    trap cleanup EXIT
    if [[ $_PP_OK -eq 1 ]]; then test_pass; else
        test_fail "string product IDs failed; proxy log: $REDHAT_SHAPE_LOG"
    fi
else
    test_fail "Red Hat response-shape proxy did not become ready: $REDHAT_SHAPE_LOG"
fi
unset _PP_PROXY_URL _PP_OK _PP_FIELD_COUNT _PP_PRODUCT_COUNT _PP_FIELD_COUNT_AFTER

test_begin "product-update-functestprod" "product update FuncTestProd"
run_bzr product update FuncTestProd --description "Updated desc"
if assert_success; then test_pass; fi

# Per-run-unique product so create never collides on the non-reset container.
_PV="pv$$x${RANDOM}"

test_begin "product-create-version-round-trips" "product create --version round-trips"
run_bzr product create --name "$_PV" --description "version test" --version "7.7"
if assert_success; then
    run_bzr product view "$_PV"
    if assert_success && assert_json_contains '[.versions[].name] | join(",")' "7.7"; then test_pass; fi
fi

test_begin "product-update-is-open-false-reflects-in-is-active" "product update --is-open false reflects in is_active"
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

test_begin "product-create-from-json" "product create --from-json"
run_bzr product create --from-json "$_PJSON_DIR/create.json"
if assert_success; then
    run_bzr product view "$_PJ_NAME"
    if assert_json '.name' "$_PJ_NAME" &&
        assert_json_contains '[.versions[].name] | join(",")' "8.8"; then test_pass; fi
fi

test_begin "product-update-from-json" "product update --from-json"
run_bzr product update --from-json "$_PJSON_DIR/update.json"
if assert_success; then
    run_bzr product view "$_PJ_NAME"
    if assert_json '.is_active' "false"; then test_pass; fi
fi

test_begin "product-create-from-json-unknown-key" "product create --from-json unknown key"
run_bzr product create --from-json "$_PJSON_DIR/bad.json"
if assert_exit_code 7 && assert_stderr_contains "unknown field"; then test_pass; fi

test_begin "product-create-from-json-cli-override" "product create --from-json CLI override"
_PJ_OVERRIDE=$(unique_name prodjson-override)
run_bzr product create --from-json "$_PJSON_DIR/create.json" --name "$_PJ_OVERRIDE"
if assert_success; then
    run_bzr product view "$_PJ_OVERRIDE"
    if assert_json '.name' "$_PJ_OVERRIDE"; then test_pass; fi
fi

rm -r "$_PJSON_DIR"
unset _PV _PJSON_DIR _PJ_NAME _PJ_OVERRIDE
echo ""
