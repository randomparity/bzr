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

test_begin "server-capabilities" "server capabilities"
run_bzr server capabilities
if assert_success && assert_json_exists '.version' &&
  assert_json_exists '.max_attachment_size' &&
  assert_json_array_min_length '.api_modes' 1 &&
  assert_json_array_min_length '.status_transitions' 1 &&
  assert_json 'all(.status_transitions[]; .from != "")' 'true'; then test_pass; fi

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

INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'bzr-bug-review', 'Functional test hyphenated review flag for bugs', 'b', 1, 1, 1, 1, 20
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'bzr-bug-review' AND target_type = 'b'
);

INSERT INTO flagtypes
    (name, description, target_type, is_active, is_requestable,
     is_requesteeble, is_multiplicable, sortkey)
SELECT 'bzr-attachment-review', 'Functional test hyphenated review flag for attachments', 'a', 1, 1, 1, 1, 20
WHERE NOT EXISTS (
    SELECT 1 FROM flagtypes WHERE name = 'bzr-attachment-review' AND target_type = 'a'
);

INSERT INTO flaginclusions (type_id, product_id, component_id)
SELECT id, NULL, NULL
FROM flagtypes
WHERE name IN ('bzr_bug_review', 'bzr_attachment_review', 'bzr-bug-review', 'bzr-attachment-review')
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
  --description "should not write" --op-sys Linux --platform PC
if assert_exit_code 3 && assert_stderr_contains "requires credentials"; then test_pass; fi

test_begin "inline-credentialless-server-info" "inline credentialless server info"
run_bzr_raw --json --server-url "$BZ_URL" server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "inline-credentialed-whoami" "inline credentialed whoami"
export BZR_FUNC_INLINE_KEY="$API_KEY"
if redhat_shape_start "$BZ_PORT"; then
  trap 'cleanup; redhat_shape_stop' EXIT
  _SA_PROXY_URL="http://127.0.0.1:${REDHAT_SHAPE_PORT}"
  _SA_OK=1
  run_bzr_raw --json --server-url "$_SA_PROXY_URL" \
    --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" whoami
  if [[ $BZR_EXIT -ne 0 ]] ||
    ! jq -e '.id | type == "number"' "$BZR_STDOUT" >/dev/null ||
    ! jq -e '.server_name == "(inline)" and .auth_mode == "api_key"' \
      "$BZR_STDOUT" >/dev/null; then
    _SA_OK=0
  fi
  case "$BZ_VERSION" in
  bz53)
    if ! grep -Eq 'user-group-shaped route=whoami count=[1-9][0-9]*' \
      "$REDHAT_SHAPE_LOG"; then _SA_OK=0; fi
    ;;
  bz50 | bz52)
    if ! grep -Eq 'user-group-shaped route=user-read count=[1-9][0-9]*' \
      "$REDHAT_SHAPE_LOG" ||
      grep -Eq 'user-group-shaped route=whoami count=[1-9][0-9]*' \
        "$REDHAT_SHAPE_LOG"; then _SA_OK=0; fi
    ;;
  esac
  redhat_shape_stop || _SA_OK=0
  trap cleanup EXIT
  if [[ $_SA_OK -eq 1 ]]; then test_pass; else
    test_fail "version-aware shaped whoami failed; proxy log: $REDHAT_SHAPE_LOG"
  fi
else
  test_fail "response-shape proxy did not become ready: $REDHAT_SHAPE_LOG"
fi
unset BZR_FUNC_INLINE_KEY
unset _SA_PROXY_URL _SA_OK

test_begin "inline-server-email-help-versions" "inline server email help names whoami versions"
run_bzr_raw --help
if assert_success && assert_stdout_contains "Bugzilla 5.0/5.2 whoami fallback" &&
  assert_stdout_contains "5.3+/BMO-derived"; then test_pass; fi

test_begin "proxy-default-ports-stay-distinct-near-limit" "proxy default ports stay valid and distinct near the limit"
_HIGH_BACKEND_PORT=65000
_TLS_DEFAULT_PORT=$(functional_proxy_default_port "$_HIGH_BACKEND_PORT" 1000)
_REDHAT_DEFAULT_PORT=$(functional_proxy_default_port "$_HIGH_BACKEND_PORT" 2000)
if [[ $_TLS_DEFAULT_PORT -eq 64000 && $_REDHAT_DEFAULT_PORT -eq 63000 ]]; then
    test_pass
else
    test_fail "derived proxy ports must be valid and distinct"
fi
unset _HIGH_BACKEND_PORT _TLS_DEFAULT_PORT _REDHAT_DEFAULT_PORT

test_begin "production-shaped-server-capabilities" "production-shaped server capabilities"
export BZR_FUNC_REDHAT_MODE=server-capabilities
if redhat_shape_start "$BZ_PORT"; then
    unset BZR_FUNC_REDHAT_MODE
    trap 'cleanup; redhat_shape_stop' EXIT
    export BZR_FUNC_INLINE_KEY="$API_KEY"
    run_bzr_raw --json \
        --server-url "http://127.0.0.1:${REDHAT_SHAPE_PORT}" \
        --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" \
        server capabilities
    _SERVER_CAPABILITIES_SHAPE_OK=1
    if ! assert_success ||
        ! assert_json '.version' '5.2+' ||
        ! assert_json_exists '.max_attachment_size' ||
        ! assert_json '.api_modes == ["rest"]' 'true' ||
        ! assert_json 'all(.status_transitions[]; .from != "")' 'true' ||
        ! assert_json 'any(.custom_fields[]; .name == "cf_bzr_proxy_probe" and .type == "single_select")' 'true'; then
        _SERVER_CAPABILITIES_SHAPE_OK=0
    fi
    for _SERVER_CAPABILITIES_ROUTE in version parameters status field-type; do
        if ! grep -Fq \
            "server-capability shaped route=${_SERVER_CAPABILITIES_ROUTE} count=1" \
            "$REDHAT_SHAPE_LOG"; then
            _SERVER_CAPABILITIES_SHAPE_OK=0
        fi
    done
    redhat_shape_stop || _SERVER_CAPABILITIES_SHAPE_OK=0
    trap cleanup EXIT
    unset BZR_FUNC_INLINE_KEY
    if [[ $_SERVER_CAPABILITIES_SHAPE_OK -eq 1 ]]; then
        test_pass
    else
        test_fail "production-shaped server capabilities failed; proxy log: $REDHAT_SHAPE_LOG"
    fi
else
    unset BZR_FUNC_REDHAT_MODE
    test_fail "server-capability response-shape proxy did not become ready: $REDHAT_SHAPE_LOG"
fi
unset _SERVER_CAPABILITIES_ROUTE _SERVER_CAPABILITIES_SHAPE_OK

echo ""
