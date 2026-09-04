#!/bin/bash
# Product catalogue, component creation, and Red Hat update-surface comparisons.

printf -v PRODUCT_RUN_TOKEN '%x-%x-%x' "$$" "$RANDOM" "$RANDOM"
PRODUCT_RUN_TOKEN="${PRODUCT_RUN_TOKEN:0:18}"
PRODUCT_BZR_NAME="bzr-compare-${PRODUCT_RUN_TOKEN}"
PRODUCT_PYBZ_NAME="pybz-compare-${PRODUCT_RUN_TOKEN}"
COMPONENT_NAME="Core-${PRODUCT_RUN_TOKEN}"
COMPONENT_DESCRIPTION="comparison component ${PRODUCT_RUN_TOKEN}"

test_begin "product-catalogues" "accessible, enterable, and selectable catalogues"
_PRODUCT_CATALOGUES_OK=1
for _product_catalogue in accessible enterable selectable; do
    if ! resource_bzr "catalogue-${_product_catalogue}-bzr" rest REST product list \
        --type "$_product_catalogue" ||
        ! resource_pybz "catalogue-${_product_catalogue}-pybz" product_catalogue \
            "$(jq -cn --arg catalogue "$_product_catalogue" \
                '{transport:"REST",catalogue:$catalogue}')" REST; then
        _PRODUCT_CATALOGUES_OK=0
        break
    fi
    jq '[.[].name] | sort' \
        "$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}-bzr.bzr.stdout.json" \
        >"$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}.bzr.json"
    jq '[.[].name] | sort' \
        "$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}-pybz.pybz.result.json" \
        >"$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}.pybz.json"
    if ! jq -e 'index("TestProduct") != null' \
        "$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}.bzr.json" >/dev/null ||
        ! jq -e 'index("TestProduct") != null' \
            "$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}.pybz.json" >/dev/null ||
        ! resource_equal "catalogue-${_product_catalogue}" \
            "$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}.bzr.json" \
            "$COMPARE_EXCHANGE_DIR/catalogue-${_product_catalogue}.pybz.json"; then
        _PRODUCT_CATALOGUES_OK=0
        break
    fi
done
if [[ $_PRODUCT_CATALOGUES_OK -eq 1 ]]; then
    test_pass
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "product catalogues differ or lack the positive control"
fi
unset _PRODUCT_CATALOGUES_OK _product_catalogue

component_normalize() {
    jq --arg name "$COMPONENT_NAME" \
        'select(.name == $name) |
         {name:"paired",description,default_assignee,is_active}' "$1" >"$2"
}

test_begin "component-create" "component creation persisted outcome"
if resource_bzr component-bzr-product rest REST product create \
    --name "$PRODUCT_BZR_NAME" --description "comparison product" &&
    resource_bzr component-pybz-product rest REST product create \
        --name "$PRODUCT_PYBZ_NAME" --description "comparison product" &&
    resource_bzr component-bzr-create rest REST component create \
        --product "$PRODUCT_BZR_NAME" --name "$COMPONENT_NAME" \
        --description "$COMPONENT_DESCRIPTION" \
        --default-assignee "$COMPARE_ADMIN_EMAIL" &&
    resource_require_positive_id \
        "$COMPARE_EXCHANGE_DIR/component-bzr-create.bzr.stdout.json" '.id' \
        bzr-component-create &&
    resource_pybz component-pybz-create component_add \
        "$(jq -cn --arg product "$PRODUCT_PYBZ_NAME" --arg component "$COMPONENT_NAME" \
            --arg description "$COMPONENT_DESCRIPTION" --arg owner "$COMPARE_ADMIN_EMAIL" \
            '{transport:"XMLRPC",params:{product:$product,name:$component,
              description:$description,default_assignee:$owner}}')" XMLRPC &&
    resource_require_positive_id \
        "$COMPARE_EXCHANGE_DIR/component-pybz-create.pybz.result.json" '.id' \
        python-bugzilla-component-create &&
    resource_bzr component-bzr-view rest REST component view \
        "$PRODUCT_BZR_NAME" "$COMPONENT_NAME" &&
    resource_bzr component-pybz-view rest REST component view \
        "$PRODUCT_PYBZ_NAME" "$COMPONENT_NAME"; then
    component_normalize \
        "$COMPARE_EXCHANGE_DIR/component-bzr-view.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/component.bzr.json"
    component_normalize \
        "$COMPARE_EXCHANGE_DIR/component-pybz-view.bzr.stdout.json" \
        "$COMPARE_EXCHANGE_DIR/component.pybz.json"
    if jq -e --arg description "$COMPONENT_DESCRIPTION" \
        '.description == $description and .default_assignee != null' \
        "$COMPARE_EXCHANGE_DIR/component.bzr.json" >/dev/null &&
        jq -e --arg description "$COMPONENT_DESCRIPTION" \
            '.description == $description and .default_assignee != null' \
            "$COMPARE_EXCHANGE_DIR/component.pybz.json" >/dev/null &&
        resource_equal component-create "$COMPARE_EXCHANGE_DIR/component.bzr.json" \
            "$COMPARE_EXCHANGE_DIR/component.pybz.json"; then
        test_pass
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "component persisted outcome differs"
    fi
fi

test_begin "component-update-redhat" "Red Hat component update client surface"
resource_gap_reset
if resource_pybz component-update-shape component_update_shape \
    "$(jq -cn --arg product "$PRODUCT_PYBZ_NAME" --arg component "$COMPONENT_NAME" \
        --arg owner "$COMPARE_ADMIN_EMAIL" \
        '{params:{product:$product,component:$component,initialowner:$owner,
          description:"updated comparison component",is_active:false}}')" LOCAL &&
    jq -e --arg product "$PRODUCT_PYBZ_NAME" --arg component "$COMPONENT_NAME" \
        --arg owner "$COMPARE_ADMIN_EMAIL" \
        '.request == {names:[{product:$product,component:$component}],
          updates:{default_assignee:$owner,description:"updated comparison component",
            is_active:false}}' \
        "$COMPARE_EXCHANGE_DIR/component-update-shape.pybz.result.json" >/dev/null; then
    run_bzr --server "$RESOURCE_SERVER" component update
    if [[ $BZR_EXIT -eq 0 ]]; then
        test_pass
    elif [[ $BZR_EXIT -eq 2 ]] &&
        grep -Fxq "error: unrecognized subcommand 'update'" "$BZR_STDERR" &&
        grep -Fxq 'Usage: bzr component [OPTIONS] <COMMAND>' "$BZR_STDERR"; then
        test_fail "bzr component update surface is not implemented"
        resource_gap_allow
    else
        test_fail "bzr component update parser result was not the controlled gap"
    fi
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "python-bugzilla component update request-shape proof is invalid"
fi
resource_expect_gap 675
