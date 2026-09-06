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

# The catalogue probe needs no credentials, so an undeclared key is refused on
# the public server too — locally, before the write that would have failed on
# auth. This pins the ordering: validation precedes dispatch.
test_begin "credentialless-undeclared-field-exits-7" "credentialless bug update --field with an undeclared name exits 7"
if [[ -z "$AFID" ]]; then
    test_fail "no fixture bug: the --field create above did not succeed"
else
    run_bzr_raw --json --server public bug update "$AFID" --field "cf_no_such_field_here=1"
    if assert_exit_code 7 && assert_stderr_contains "cf_no_such_field_here"; then test_pass; fi
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

rm -r "$_AF_DIR"
unset _AF _AF_DIR _AF_INITIAL _AF_UPDATED _AF_JSON AFID
echo ""
