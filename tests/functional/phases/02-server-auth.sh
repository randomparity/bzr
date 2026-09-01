# 02-server-auth
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 2: Server & Auth
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 2: Server & Auth ──────────────────────────────────"

test_begin "server-info" "server info"
run_bzr server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

# TODO(#626): this credentialed assertion never checks max_attachment_size, so a
# permanently null value has always passed here. The credentialless `null`
# assertion further down is correct under accepted ADR 0005 and stays; #626 owns
# adding the non-null credentialed case.
test_begin "server-capabilities" "server capabilities"
run_bzr server capabilities
if assert_success && assert_json_exists '.version' &&
    assert_json_array_min_length '.api_modes' 1 &&
    assert_json_array_min_length '.status_transitions' 1; then test_pass; fi

test_begin "whoami" "whoami"
run_bzr whoami
if assert_success && assert_json_exists '.id' &&
    assert_json_exists '.server_name' &&
    assert_json '.auth_mode' 'api_key'; then test_pass; fi

test_begin "server-auto-whoami" "--server auto whoami"
run_bzr_raw --json --server auto whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi

test_begin "fixture-flag-types-exist" "fixture flag types exist"
_FLAG_SQL=$(mktemp /tmp/bzr-func-flags.XXXXXX.sql)
cat >"$_FLAG_SQL" <<'SQL'
INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'bzr_bug_review', 'Functional test review flag for bugs', 'b', 1, 1, 1, 1, 10
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'bzr_bug_review' AND target_type = 'b'
);

INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'bzr_attachment_review', 'Functional test review flag for attachments', 'a', 1, 1, 1, 1, 10
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'bzr_attachment_review' AND target_type = 'a'
);

INSERT INTO flaginclusions (type_id, product_id, component_id)
SELECT id, NULL, NULL
FROM flagtypes
WHERE name IN ('bzr_bug_review', 'bzr_attachment_review')
  AND target_type IN ('b', 'a')
  AND NOT EXISTS (
      SELECT 1 FROM flaginclusions WHERE flaginclusions.type_id = flagtypes.id
  );
SQL
if run_bugzilla_sql_file "$_FLAG_SQL"; then
    test_pass
else
    test_fail "could not seed functional flag types"
fi
rm -f "$_FLAG_SQL"
unset _FLAG_SQL

test_begin "credentialless-named-server-info" "credentialless named server info"
run_bzr_raw --json --server public server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "credentialless-named-server-capabilities-attachment-size-null" "credentialless named server capabilities (attachment size null)"
run_bzr_raw --json --server public server capabilities
if assert_success && assert_json_exists '.version' &&
    assert_json '.max_attachment_size' 'null'; then test_pass; fi

test_begin "credentialless-named-whoami-fails-before-network-auth" "credentialless named whoami fails before network auth"
run_bzr_raw --json --server public whoami
if assert_exit_code 3 && assert_stderr_contains "requires credentials"; then test_pass; fi

test_begin "credentialless-named-write-fails-before-mutation" "credentialless named write fails before mutation"
run_bzr_raw --json --server public bug create \
    --product FuncTestProd --component Backend --summary "public write" \
    --description "should not write" --op-sys Linux --rep-platform PC
if assert_exit_code 3 && assert_stderr_contains "requires credentials"; then test_pass; fi

test_begin "inline-credentialless-server-info" "inline credentialless server info"
run_bzr_raw --json --server-url "$BZ_URL" server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "inline-credentialed-whoami" "inline credentialed whoami"
export BZR_FUNC_INLINE_KEY="$API_KEY"
run_bzr_raw --json --server-url "$BZ_URL" \
    --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" whoami
if assert_success && assert_json_exists '.id' &&
    assert_json '.server_name' '(inline)' &&
    assert_json '.auth_mode' 'api_key'; then test_pass; fi
unset BZR_FUNC_INLINE_KEY

echo ""
