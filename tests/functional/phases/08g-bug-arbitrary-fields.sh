# 08g-bug-arbitrary-fields
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: its own bugs.
# shellcheck shell=bash
#
# `bug create` / `bug update` --field and --field-json (ADR 0053, issues #283
# and #671). Both verbs live here because they share one validation path: every
# key is checked against the server's own field catalogue before dispatch, so
# these assertions are about the catalogue contract rather than about create or
# update individually.
#
# The default containers declare no cf_* fields, so `whiteboard` — a declared
# built-in bzr also exposes as a typed flag — carries the accept cases, exactly
# as the python-bugzilla comparison harness drives it. The reject cases use a
# name no Bugzilla declares.

# ══════════════════════════════════════════════════════════════════════
# Phase 8g: Bug arbitrary field passthrough (--field / --field-json)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8g: Bug --field / --field-json ────────────────────"

_AF=(--product FuncTestProd --component Backend --op-sys Linux --platform PC --description d)
_AF_DIR=$(mktemp -d /tmp/bzr-func-field.XXXXXX)
_AF_INITIAL="af$$x${RANDOM}"
_AF_UPDATED="${_AF_INITIAL}-updated"
_AF_JSON="${_AF_INITIAL}-json"
write_json_fixture "$_AF_DIR/fields.json" "{\"whiteboard\":\"$_AF_JSON\"}"
write_json_fixture "$_AF_DIR/bogus.json" '{"cf_no_such_field_here":"x"}'
write_json_fixture "$_AF_DIR/not-object.json" '["whiteboard"]'

test_begin "bug-create-field-sets-a-declared-field" "bug create --field sets a declared field"
AFID=$(make_bug "${_AF[@]}" --summary "field create" --field "whiteboard=$_AF_INITIAL")
if [[ -z "$AFID" ]]; then
    test_fail "bug create --field did not return a bug ID"
else
    run_bzr bug view "$AFID"
    if assert_success && assert_json '.whiteboard' "$_AF_INITIAL"; then test_pass; fi
fi

test_begin "bug-update-field-sets-a-declared-field" "bug update --field sets a declared field"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr bug update "$AFID" --field "whiteboard=$_AF_UPDATED"
    if assert_success; then
        run_bzr bug view "$AFID"
        if assert_json '.whiteboard' "$_AF_UPDATED"; then test_pass; fi
    fi
fi

test_begin "bug-update-field-json-sets-a-declared-field" "bug update --field-json sets a declared field"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr bug update "$AFID" --field-json "$_AF_DIR/fields.json"
    if assert_success; then
        run_bzr bug view "$AFID"
        if assert_json '.whiteboard' "$_AF_JSON"; then test_pass; fi
    fi
fi

# An empty value is how Bugzilla clears a field; it must survive parsing rather
# than being rejected as a missing value.
test_begin "bug-update-field-empty-value-clears-the-field" "bug update --field with an empty value clears the field"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr bug update "$AFID" --field "whiteboard="
    if assert_success; then
        run_bzr bug view "$AFID"
        if assert_json '.whiteboard' ""; then test_pass; fi
    fi
fi

# The defect this feature exists to avoid: python-bugzilla's CLI passes an
# undeclared key straight through and Bugzilla answers 200 having changed
# nothing. bzr refuses locally instead, before any write.
test_begin "bug-update-undeclared-field-exits-7" "bug update --field with an undeclared name exits 7"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr bug update "$AFID" --field "cf_no_such_field_here=1"
    if assert_exit_code 7 &&
        assert_stderr_contains "cf_no_such_field_here" &&
        assert_stderr_contains "bzr field list"; then test_pass; fi
fi

# The rejection above is the one message a user is guaranteed to read, because
# they only see it when they are already stuck. Advice that fails when followed
# is a defect, so run the command it names and require it to work. Since #718
# that command is `bzr field list` with no argument, which enumerates the whole
# accepted set rather than the custom-field subset.
test_begin "undeclared-field-advice-names-a-command-that-works" "the undeclared-field message names a command that works"
run_bzr field list
if assert_success && assert_json 'length > 0' true; then test_pass; fi

test_begin "bug-create-undeclared-field-exits-7" "bug create --field with an undeclared name exits 7"
run_bzr bug create "${_AF[@]}" --summary "field reject" --field "cf_no_such_field_here=1"
if assert_exit_code 7 && assert_stderr_contains "cf_no_such_field_here"; then test_pass; fi

test_begin "bug-create-field-json-undeclared-field-exits-7" "bug create --field-json with an undeclared name exits 7"
run_bzr bug create "${_AF[@]}" --summary "field-json reject" \
    --field-json "$_AF_DIR/bogus.json"
if assert_exit_code 7 && assert_stderr_contains "cf_no_such_field_here"; then test_pass; fi

test_begin "bug-field-json-non-object-exits-7" "bug create --field-json rejects a non-object document"
run_bzr bug create "${_AF[@]}" --summary "field-json shape" \
    --field-json "$_AF_DIR/not-object.json"
if assert_exit_code 7 && assert_stderr_contains "JSON object"; then test_pass; fi

test_begin "bug-field-malformed-pair-exits-7" "bug create --field without KEY=VALUE exits 7"
run_bzr bug create "${_AF[@]}" --summary "field shape" --field whiteboard
if assert_exit_code 7 && assert_stderr_contains "KEY=VALUE"; then test_pass; fi

test_begin "bug-field-duplicate-key-exits-7" "bug create --field with a duplicated key exits 7"
run_bzr bug create "${_AF[@]}" --summary "field dupe" \
    --field "whiteboard=a" --field "whiteboard=b"
if assert_exit_code 7 && assert_stderr_contains "more than once"; then test_pass; fi

# --field never silently overrides a typed flag, and never silently loses to
# one; the collision is refused so the user resolves it.
test_begin "bug-field-colliding-with-a-typed-flag-exits-7" "bug update --field colliding with a typed flag exits 7"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr bug update "$AFID" --whiteboard typed --field "whiteboard=passthrough"
    if assert_exit_code 7 && assert_stderr_contains "dedicated flag"; then test_pass; fi
fi

# --field does not weaken the credential requirement. Both verbs that accept it
# are mutations, so a credentialless invocation is refused at exit 3 before the
# connection -- and therefore before the field catalogue is ever consulted.
# There is no anonymous path that reaches --field validation.
test_begin "credentialless-field-update-still-requires-credentials" "credentialless bug update --field still requires credentials (exit 3)"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr_raw --json --server public bug update "$AFID" --field "cf_no_such_field_here=1"
    if assert_exit_code 3 && assert_stderr_contains "requires credentials"; then test_pass; fi
fi

# --dry-run makes no connection, so it neither validates nor writes; the
# previewed payload still carries the passthrough key.
test_begin "bug-update-field-dry-run-previews-without-writing" "bug update --field --dry-run previews the passthrough key"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr bug update "$AFID" --field "whiteboard=never-written" --dry-run
    if assert_success && assert_json '.action' "dry-run" &&
        assert_json '.changes.whiteboard' "never-written"; then
        run_bzr bug view "$AFID"
        if assert_json '.whiteboard' ""; then test_pass; fi
    fi
fi

# Acceptance criterion 2 against a real server: anything the listing shows is
# accepted. `short_desc` is pinned in the selector, so what the jq computes is
# "is short_desc present with source == server"; reading it back OUT of the
# listing is what makes the block bite rather than pass vacuously -- an absent
# row yields an empty name and fails at the guard, and a listed name the
# validator rejects exits 7 below. It proves bzr does not refuse a
# catalogue-only name; it cannot prove Bugzilla honours the key.
#
# Placed last in the phase deliberately: it writes to $AFID, and the block above
# asserts that bug's whiteboard is still empty. `short_desc` is pinned rather
# than taking an arbitrary .[0], which could land on a read-only catalogue field
# Bugzilla refuses on its own; 05-fields-classifications.sh already proves these
# containers declare it, and `summary` (not `short_desc`) is the BUG_FIELDS
# canonical, so `source: server` is the grounded expectation.
#
# The guards are nested inside the `assert_success` arm on purpose: assert_success
# already calls test_fail on a non-zero exit, so a sibling `if [[ -z ... ]]`
# would fire a second test_fail and count one failing test twice.
test_begin "field-list-agrees-with-field-validator" "a server-only name from field list is accepted by --field"
run_bzr field list
if assert_success; then
    _AF_SERVER_NAME=$(jq -r 'map(select(.source == "server" and .name == "short_desc")) | .[0].name // empty' "$BZR_STDOUT")
    if [[ -z "$_AF_SERVER_NAME" ]]; then
        test_fail "field list did not report short_desc as a server-declared name"
    elif [[ -z "$AFID" ]]; then
        test_fail "no fixture bug: the --field create above did not succeed"
    else
        run_bzr bug update "$AFID" --field "${_AF_SERVER_NAME}=oracle"
        if assert_success; then test_pass; fi
    fi
fi

rm -r "$_AF_DIR"
unset _AF _AF_DIR _AF_INITIAL _AF_UPDATED _AF_JSON _AF_SERVER_NAME AFID
echo ""
