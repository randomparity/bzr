#!/bin/bash
# Bounded authenticated core journey for the isolated RHBZ comparison runner.

RHBZ_CORE_TOKEN=$(unique_name rhbz-core)
RHBZ_CORE_SUMMARY="$RHBZ_CORE_TOKEN core journey"
RHBZ_CORE_UPDATED_SUMMARY="$RHBZ_CORE_TOKEN updated"
RHBZ_CORE_BUG_ID=""
RHBZ_CORE_SUB_COMPONENT="${RHBZ_FIELDS_BZR_SUB_COMPONENT:-}"

rhbz_core_bug_id() {
    resource_positive_id "$COMPARE_EXCHANGE_DIR/rhbz-core-create.bzr.stdout.json" '.id'
}

rhbz_core_bug_matches() {
    local path="$1" expected="$2"

    jq -e --argjson id "$RHBZ_CORE_BUG_ID" --arg summary "$expected" \
        '(type == "object" and .id == $id and .summary == $summary) or
         (type == "array" and any(.[]; .id == $id and .summary == $summary))' \
        "$path" >/dev/null
}

test_begin "create" "RHBZ creates an authenticated core bug"
if [[ -n $RHBZ_CORE_SUB_COMPONENT ]] &&
    rhbz_core_create_fields=$(jq -cn --arg component TestComponent \
        --arg value "$RHBZ_CORE_SUB_COMPONENT" '{rh_sub_components:{($component):[$value]}}') &&
    printf '%s\n' "$rhbz_core_create_fields" | resource_bzr rhbz-core-create rest REST bug create \
    --product TestProduct --component TestComponent --summary "$RHBZ_CORE_SUMMARY" \
    --description "RHBZ core comparison fixture" --op-sys Linux --platform PC \
    --priority Normal --severity normal --field-json - &&
    RHBZ_CORE_BUG_ID=$(rhbz_core_bug_id) && [[ $RHBZ_CORE_BUG_ID =~ ^[1-9][0-9]*$ ]]; then
    test_pass
else
    test_fail "RHBZ core create did not return a positive bug ID"
fi

test_begin "view-search" "RHBZ REST and XML-RPC view and search the core bug"
if [[ -n $RHBZ_CORE_BUG_ID ]] &&
    resource_bzr rhbz-core-view-rest rest REST bug view "$RHBZ_CORE_BUG_ID" \
        --fields id,summary &&
    resource_bzr rhbz-core-view-xmlrpc xmlrpc XMLRPC bug view "$RHBZ_CORE_BUG_ID" \
        --fields id,summary &&
    resource_bzr rhbz-core-search-rest rest REST bug list --id "$RHBZ_CORE_BUG_ID" \
        --fields id,summary &&
    resource_bzr rhbz-core-search-xmlrpc xmlrpc XMLRPC bug list --id "$RHBZ_CORE_BUG_ID" \
        --fields id,summary &&
    rhbz_core_bug_matches "$COMPARE_EXCHANGE_DIR/rhbz-core-view-rest.bzr.stdout.json" \
        "$RHBZ_CORE_SUMMARY" &&
    rhbz_core_bug_matches "$COMPARE_EXCHANGE_DIR/rhbz-core-view-xmlrpc.bzr.stdout.json" \
        "$RHBZ_CORE_SUMMARY" &&
    rhbz_core_bug_matches "$COMPARE_EXCHANGE_DIR/rhbz-core-search-rest.bzr.stdout.json" \
        "$RHBZ_CORE_SUMMARY" &&
    rhbz_core_bug_matches "$COMPARE_EXCHANGE_DIR/rhbz-core-search-xmlrpc.bzr.stdout.json" \
        "$RHBZ_CORE_SUMMARY"; then
    test_pass
else
    test_fail "RHBZ core view or search did not preserve the created bug"
fi

test_begin "update-readback" "RHBZ updates and reads back the core bug"
if [[ -n $RHBZ_CORE_BUG_ID ]] &&
    resource_bzr rhbz-core-update-rest rest REST bug update "$RHBZ_CORE_BUG_ID" \
        --summary "$RHBZ_CORE_UPDATED_SUMMARY" &&
    resource_bzr rhbz-core-readback-rest rest REST bug view "$RHBZ_CORE_BUG_ID" \
        --fields id,summary &&
    resource_bzr rhbz-core-readback-xmlrpc xmlrpc XMLRPC bug view "$RHBZ_CORE_BUG_ID" \
        --fields id,summary &&
    rhbz_core_bug_matches "$COMPARE_EXCHANGE_DIR/rhbz-core-readback-rest.bzr.stdout.json" \
        "$RHBZ_CORE_UPDATED_SUMMARY" &&
    rhbz_core_bug_matches "$COMPARE_EXCHANGE_DIR/rhbz-core-readback-xmlrpc.bzr.stdout.json" \
        "$RHBZ_CORE_UPDATED_SUMMARY"; then
    test_pass
else
    test_fail "RHBZ core update was not persisted through both reads"
fi

test_begin "named-identity" "RHBZ named API-key identity is reported"
if resource_bzr rhbz-core-whoami rest REST whoami &&
    jq -e --arg login "$COMPARE_ADMIN_EMAIL" \
        '(.name == $login) or (.login == $login)' \
        "$COMPARE_EXCHANGE_DIR/rhbz-core-whoami.bzr.stdout.json" >/dev/null; then
    test_pass
else
    test_fail "RHBZ named API-key identity did not match the configured account"
fi
