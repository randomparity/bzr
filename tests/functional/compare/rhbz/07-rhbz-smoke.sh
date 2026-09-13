test_begin "extensions" "RHBZ extensions are advertised"
if curl -fsS "$BZ_URL/rest/extensions" | jq -e '.extensions | has("ExternalBugs") and has("SubComponents") and has("RedHat")' >/dev/null; then
    test_pass
else
    test_fail "RHBZ did not advertise ExternalBugs, SubComponents, and RedHat"
fi

rhbz_component_array_bug() {
    local sql_file="$COMPARE_EXCHANGE_DIR/rhbz-component-array.sql"

    printf '%s\n' \
        "INSERT INTO bugs (assigned_to, bug_severity, bug_status, creation_ts, delta_ts, short_desc, op_sys, priority, product_id, rep_platform, reporter, version, component_id, everconfirmed) SELECT 1, 'normal', 'NEW', NOW(), NOW(), 'RHBZ XML-RPC component array comparison', 'Linux', 'Normal', p.id, 'PC', 1, 'unspecified', c.id, 1 FROM products p JOIN components c ON c.product_id = p.id WHERE p.name = 'TestProduct' AND c.name = 'TestComponent' LIMIT 1;" \
        'SELECT LAST_INSERT_ID();' >"$sql_file"
    run_bugzilla_sql_file "$sql_file" | tail -n1
}

test_begin "xmlrpc-component-array" "RHBZ XML-RPC preserves component arrays"
rhbz_component_bug_id=$(rhbz_component_array_bug)
if [[ $rhbz_component_bug_id =~ ^[1-9][0-9]*$ ]] &&
    resource_bzr rhbz-component-array-rest rest REST bug view "$rhbz_component_bug_id" --fields id,component &&
    resource_bzr rhbz-component-array-xmlrpc xmlrpc XMLRPC bug view "$rhbz_component_bug_id" --fields id,component &&
    jq -e '.component == ["TestComponent"]' \
        "$COMPARE_EXCHANGE_DIR/rhbz-component-array-rest.bzr.stdout.json" >/dev/null &&
    jq -e '.component == ["TestComponent"]' \
        "$COMPARE_EXCHANGE_DIR/rhbz-component-array-xmlrpc.bzr.stdout.json" >/dev/null; then
    test_pass
else
    test_fail "RHBZ component array did not match between REST and XML-RPC"
fi
