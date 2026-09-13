#!/bin/bash
# shellcheck disable=SC2016 # jq filters intentionally retain their variable syntax.
# Real-server catalogue for RHBZ's Red Hat custom fields and sub-components.

RHBZ_FIELDS_TOKEN=$(unique_name rhbz-fields)
RHBZ_FIELDS_PRODUCT=TestProduct
RHBZ_FIELDS_COMPONENT=TestComponent
RHBZ_FIELDS_RELEASE="${RHBZ_FIELDS_TOKEN}-release"
RHBZ_FIELDS_SUB_COMPONENT="${RHBZ_FIELDS_TOKEN}-sub-component"
RHBZ_FIELDS_FIXED_IN="${RHBZ_FIELDS_TOKEN}-fixed-in"
RHBZ_FIELDS_DEVEL_WHITEBOARD="${RHBZ_FIELDS_TOKEN}-devel"
RHBZ_FIELDS_INTERNAL_WHITEBOARD="${RHBZ_FIELDS_TOKEN}-internal"
RHBZ_FIELDS_QA_WHITEBOARD="${RHBZ_FIELDS_TOKEN}-qa"

rhbz_fields_prepare() {
    local controls_file="$COMPARE_EXCHANGE_DIR/rhbz-fields-controls.sql"
    local -a controls

    [[ ${RHBZ_FIELDS_READY:-0} -eq 1 ]] && return 0

    printf '%s\n' \
        "SELECT p.id FROM components c JOIN products p ON p.id = c.product_id JOIN profiles u ON u.userid = c.initialowner WHERE p.name = '$RHBZ_FIELDS_PRODUCT' AND c.name = '$RHBZ_FIELDS_COMPONENT' AND c.isactive = 1 AND u.login_name = '$COMPARE_ADMIN_EMAIL' LIMIT 1;" \
        "SELECT c.id FROM components c JOIN products p ON p.id = c.product_id JOIN profiles u ON u.userid = c.initialowner WHERE p.name = '$RHBZ_FIELDS_PRODUCT' AND c.name = '$RHBZ_FIELDS_COMPONENT' AND c.isactive = 1 AND u.login_name = '$COMPARE_ADMIN_EMAIL' LIMIT 1;" \
        "SELECT COUNT(DISTINCT g.name) FROM profiles p JOIN user_group_map m ON m.user_id = p.userid JOIN groups g ON g.id = m.group_id WHERE p.login_name = '$COMPARE_ADMIN_EMAIL' AND g.name IN ('devel', 'editbugs', 'qa', 'redhat');" \
        "INSERT INTO releases (product_id, value, sortkey, isactive) SELECT p.id, '$RHBZ_FIELDS_RELEASE', 0, 1 FROM products p WHERE p.name = '$RHBZ_FIELDS_PRODUCT' AND NOT EXISTS (SELECT 1 FROM releases r WHERE r.product_id = p.id AND r.value = '$RHBZ_FIELDS_RELEASE');" \
        "INSERT INTO rh_sub_components (name, component_id, initialowner, description, isactive, sortkey, level, full_name) SELECT '$RHBZ_FIELDS_SUB_COMPONENT', c.id, p.userid, '$RHBZ_FIELDS_TOKEN RHBZ comparison sub-component', 1, 0, 0, '$RHBZ_FIELDS_SUB_COMPONENT' FROM components c JOIN products product ON product.id = c.product_id JOIN profiles p ON p.login_name = '$COMPARE_ADMIN_EMAIL' WHERE product.name = '$RHBZ_FIELDS_PRODUCT' AND c.name = '$RHBZ_FIELDS_COMPONENT' AND NOT EXISTS (SELECT 1 FROM rh_sub_components sc WHERE sc.component_id = c.id AND sc.name = '$RHBZ_FIELDS_SUB_COMPONENT' AND sc.parent_id IS NULL);" \
        "SELECT id FROM releases WHERE value = '$RHBZ_FIELDS_RELEASE' LIMIT 1;" \
        "SELECT id FROM rh_sub_components WHERE name = '$RHBZ_FIELDS_SUB_COMPONENT' LIMIT 1;" \
        >"$controls_file"
    mapfile -t controls < <(run_bugzilla_sql_file "$controls_file" | awk '/^[0-9]+$/')
    [[ ${controls[0]:-} =~ ^[1-9][0-9]*$ && ${controls[1]:-} =~ ^[1-9][0-9]*$ &&
        ${controls[2]:-} == 4 && ${controls[3]:-} =~ ^[1-9][0-9]*$ &&
        ${controls[4]:-} =~ ^[1-9][0-9]*$ ]] || return 1
    RHBZ_FIELDS_PRODUCT_ID=${controls[0]}
    RHBZ_FIELDS_COMPONENT_ID=${controls[1]}
    RHBZ_FIELDS_READY=1
}

rhbz_fields_metadata_ready() {
    curl -fsS --get "$BZ_URL/rest/field/bug" \
        --data-urlencode "Bugzilla_api_key=$BZR_COMPARE_API_KEY" \
        >"$COMPARE_EXCHANGE_DIR/rhbz-fields-metadata.json" &&
        jq -e --argjson names \
            '["rh_sub_components", "target_release", "cf_fixed_in", "cf_devel_whiteboard", "cf_internal_whiteboard", "cf_qa_whiteboard"]' \
            '. as $response | all($names[]; . as $name | $response.fields | any(.[]; .name == $name))' \
            "$COMPARE_EXCHANGE_DIR/rhbz-fields-metadata.json" >/dev/null
}

rhbz_fields_create_bug() {
    local label="$1"
    local sql_file="$COMPARE_EXCHANGE_DIR/rhbz-fields-${label}.sql"

    printf '%s\n' \
        "INSERT INTO bugs (assigned_to, bug_severity, bug_status, creation_ts, delta_ts, short_desc, op_sys, priority, product_id, rep_platform, reporter, version, component_id, everconfirmed) VALUES (1, 'normal', 'NEW', NOW(), NOW(), '$RHBZ_FIELDS_TOKEN $label comparison bug', 'Linux', 'Normal', $RHBZ_FIELDS_PRODUCT_ID, 'PC', 1, 'unspecified', $RHBZ_FIELDS_COMPONENT_ID, 1);" \
        'SELECT LAST_INSERT_ID();' >"$sql_file"
    run_bugzilla_sql_file "$sql_file" | tail -n1
}

rhbz_fields_read() {
    local bug_id="$1" fields="$2" name="$3"

    curl -fsS --get "$BZ_URL/rest/bug/$bug_id" \
        --data-urlencode "include_fields=id,$fields" \
        --data-urlencode "Bugzilla_api_key=$BZR_COMPARE_API_KEY" \
        >"$COMPARE_EXCHANGE_DIR/rhbz-fields-${name}-state.json"
}

rhbz_fields_probe_bzr() {
    local name="$1" bug_id="$2" field="$3" value="$4" state_filter="$5" expected="$6"

    RUST_LOG=bzr=debug run_bzr --server "$RESOURCE_SERVER" --api REST bug update "$bug_id" \
        --field "$field=$value"
    resource_capture_bzr "rhbz-fields-${name}"
    if [[ $BZR_EXIT -eq 0 ]] && jq -e . "$BZR_STDOUT" >/dev/null &&
        observe_bzr_transport && [[ $BZR_TRANSPORT == REST ]] &&
        rhbz_fields_read "$bug_id" "$field" "${name}-bzr" &&
        jq -e --arg expected "$expected" "$state_filter" \
            "$COMPARE_EXCHANGE_DIR/rhbz-fields-${name}-bzr-state.json" >/dev/null; then
        test_pass
        return 0
    fi
    test_fail "bzr $name probe did not persist its value"
    resource_gap_allow
    resource_expect_gap 775
}

rhbz_fields_run() {
    local name="$1" operation="$2" payload="$3" pybz_fields="$4" bzr_field="$5"
    local pybz_filter="$6" pybz_expected="$7" bzr_value="$8" bzr_filter="$9"
    local bzr_expected="${10}" bug_id

    test_begin "$name" "RHBZ $name persists a configured field"
    resource_gap_reset
    if rhbz_fields_prepare && rhbz_fields_metadata_ready &&
        bug_id=$(rhbz_fields_create_bug "$name") && [[ $bug_id =~ ^[1-9][0-9]*$ ]] &&
        resource_pybz "rhbz-fields-${name}" "$operation" \
            "$(jq -cn --argjson bug_id "$bug_id" --argjson payload "$payload" '$payload + {bug_id:$bug_id,transport:"REST"}')" REST &&
        rhbz_fields_read "$bug_id" "$pybz_fields" "$name" &&
        jq -e --arg expected "$pybz_expected" "$pybz_filter" \
            "$COMPARE_EXCHANGE_DIR/rhbz-fields-${name}-state.json" >/dev/null; then
        rhbz_fields_probe_bzr "$name" "$bug_id" "$bzr_field" "$bzr_value" "$bzr_filter" "$bzr_expected"
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "RHBZ $name positive control failed"
    fi
}

rhbz_fields_run sub-components rhbz_sub_component \
    "$(jq -cn --arg component "$RHBZ_FIELDS_COMPONENT" --arg value "$RHBZ_FIELDS_SUB_COMPONENT" '{component:$component,sub_component:$value}')" \
    sub_components rh_sub_components \
    '.bugs[0].sub_components | any(.[]; (if type == "object" then .name else . end) == $expected)' \
    "$RHBZ_FIELDS_SUB_COMPONENT" \
    "${RHBZ_FIELDS_SUB_COMPONENT}-bzr" \
    '.bugs[0].sub_components | any(.[]; (if type == "object" then .name else . end) == $expected)' \
    "${RHBZ_FIELDS_SUB_COMPONENT}-bzr"

rhbz_fields_run target-release rhbz_target_release \
    "$(jq -cn --arg value "$RHBZ_FIELDS_RELEASE" '{target_release:$value}')" \
    target_release target_release \
    '.bugs[0].target_release | index($expected)' "$RHBZ_FIELDS_RELEASE" \
    "${RHBZ_FIELDS_RELEASE}-bzr" \
    '.bugs[0].target_release | index($expected)' "${RHBZ_FIELDS_RELEASE}-bzr"

rhbz_fields_run fixed-in rhbz_fixed_in \
    "$(jq -cn --arg value "$RHBZ_FIELDS_FIXED_IN" '{fixed_in:$value}')" \
    cf_fixed_in cf_fixed_in \
    '.bugs[0].cf_fixed_in == $expected' "$RHBZ_FIELDS_FIXED_IN" \
    "${RHBZ_FIELDS_FIXED_IN}-bzr" \
    '.bugs[0].cf_fixed_in == $expected' "${RHBZ_FIELDS_FIXED_IN}-bzr"

rhbz_fields_run whiteboards rhbz_whiteboards \
    "$(jq -cn --arg devel "$RHBZ_FIELDS_DEVEL_WHITEBOARD" --arg internal "$RHBZ_FIELDS_INTERNAL_WHITEBOARD" --arg qa "$RHBZ_FIELDS_QA_WHITEBOARD" '{devel_whiteboard:$devel,internal_whiteboard:$internal,qa_whiteboard:$qa}')" \
    cf_devel_whiteboard,cf_internal_whiteboard,cf_qa_whiteboard cf_devel_whiteboard \
    '($expected | split(",")) as $values | .bugs[0].cf_devel_whiteboard == $values[0] and .bugs[0].cf_internal_whiteboard == $values[1] and .bugs[0].cf_qa_whiteboard == $values[2]' \
    "$RHBZ_FIELDS_DEVEL_WHITEBOARD,$RHBZ_FIELDS_INTERNAL_WHITEBOARD,$RHBZ_FIELDS_QA_WHITEBOARD" \
    "${RHBZ_FIELDS_DEVEL_WHITEBOARD}-bzr" \
    '.bugs[0].cf_devel_whiteboard == $expected' "${RHBZ_FIELDS_DEVEL_WHITEBOARD}-bzr"
