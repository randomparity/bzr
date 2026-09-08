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

# First network use of the `auto` server, which is what runs auth detection --
# keep this case ahead of `server-auto-whoami`. `config set-server` does no
# network I/O, so detection has not run before this point. The valid_login
# fallback asserted on here is only reached when rest/whoami is unavailable;
# bz53 is built from bugzilla/bugzilla master, which serves whoami, so
# detect_auth_method returns before the code under test runs.
test_begin "auto-server-probe-matches-anonymous-response" "header-auth probe compares the anonymous response, then keeps query-param"
case "$BZ_VERSION" in
bz53)
  test_skip "rest/whoami short-circuits the valid_login fallback on $BZ_VERSION"
  ;;
*)
  _SA_PROBE_FAIL=""
  RUST_LOG=bzr=debug run_bzr --server auto whoami
  if [[ $BZR_EXIT -ne 0 ]]; then
    _SA_PROBE_FAIL="detection command exited $BZR_EXIT"
  elif ! grep -q "matched the anonymous response" "$BZR_STDERR"; then
    _SA_PROBE_FAIL="header-auth probe did not report comparing the anonymous response"
  else
    run_bzr config show
    if [[ $BZR_EXIT -ne 0 ]]; then
      _SA_PROBE_FAIL="config show exited $BZR_EXIT"
    elif ! jq -e '.servers.auto.auth_method == "query_param"' "$BZR_STDOUT" >/dev/null; then
      _SA_PROBE_FAIL="auto server did not persist query_param auth"
    fi
  fi
  if [[ -z $_SA_PROBE_FAIL ]]; then test_pass; else test_fail "$_SA_PROBE_FAIL"; fi
  unset _SA_PROBE_FAIL
  ;;
esac

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

# ══════════════════════════════════════════════════════════════════════
# auth_method provenance stamp (ADR-0066)
#
# `auth_method` is detected once per server and cached, and before ADR-0066
# nothing re-ran detection, so a config written by a bzr predating the
# differential probe (ADR-0056) kept a method the server ignores. A provenance
# stamp decides which cached values may be trusted. These cases drive the stamp
# against a real container; they read and edit `config.toml` directly because
# `config show` is a curated view that does not carry the stamp.
# ══════════════════════════════════════════════════════════════════════
_SA_CONFIG="$XDG_CONFIG_HOME/bzr/config.toml"

# Print one key's value from the `[servers.<name>]` table, or nothing.
_sa_server_key() {
    python3 - "$_SA_CONFIG" "$1" "$2" <<'PY'
import re
import sys

path, server, key = sys.argv[1:4]
in_table = False
for line in open(path):
    stripped = line.strip()
    if stripped.startswith("["):
        in_table = stripped == "[servers.%s]" % server
        continue
    if in_table:
        m = re.match(r"%s\s*=\s*(.*)" % re.escape(key), stripped)
        if m:
            print(m.group(1).strip().strip('"'))
            break
PY
}

# Replace `key = value` lines inside the `[servers.<name>]` table. TOML rejects
# duplicate keys, so each key is deleted from the table before the replacement
# is inserted under the header; passing a bare key name deletes it outright.
_sa_server_set() {
    local server="$1"
    shift
    python3 - "$_SA_CONFIG" "$server" "$@" <<'PY'
import sys

path, server = sys.argv[1:3]
additions = [a for a in sys.argv[3:] if "=" in a]
keys = {a.split("=", 1)[0].strip() for a in sys.argv[3:]}
out = []
in_table = False
for line in open(path).read().splitlines():
    stripped = line.strip()
    if stripped.startswith("["):
        in_table = stripped == "[servers.%s]" % server
        out.append(line)
        if in_table:
            out.extend(additions)
        continue
    if in_table and stripped.split("=", 1)[0].strip() in keys:
        continue
    out.append(line)
open(path, "w").write("\n".join(out) + "\n")
PY
}

test_begin "auth-method-stamp-written-by-detection" "detection stamps the auth_method provenance"
_SA_STAMP_FAIL=""
_SA_DETECTED_METHOD=$(_sa_server_key auto auth_method)
if [[ -z $_SA_DETECTED_METHOD ]]; then
    _SA_STAMP_FAIL="auto server has no detected auth_method to stamp"
elif [[ $(_sa_server_key auto auth_method_source) != "differential-probe" ]]; then
    _SA_STAMP_FAIL="detection did not stamp auth_method_source=differential-probe"
elif [[ $(_sa_server_key test auth_method_source) != "pinned" ]]; then
    # Phase 1 configured `test` with `--auth-method query_param`.
    _SA_STAMP_FAIL="--auth-method did not stamp auth_method_source=pinned"
fi
if [[ -z $_SA_STAMP_FAIL ]]; then test_pass; else test_fail "$_SA_STAMP_FAIL"; fi
unset _SA_STAMP_FAIL

test_begin "auth-method-unstamped-entry-redetects" "an unstamped auth_method is re-detected and re-stamped"
# The population this exists for: a value cached before the stamp existed. Only
# the stamp is removed, so what re-detects here is the ADR-0066 gate and not
# some other difference.
_SA_REDETECT_FAIL=""
_sa_server_set auto 'auth_method = "header"' auth_method_source
if [[ $(_sa_server_key auto auth_method) != "header" ]] ||
    [[ -n $(_sa_server_key auto auth_method_source) ]]; then
    # Without this the case passes vacuously when the edit misses the table:
    # `auto` would still be correctly stamped and take the cached path.
    _SA_REDETECT_FAIL="fixture edit did not land on [servers.auto]"
fi
RUST_LOG=bzr=debug run_bzr --server auto whoami
if [[ -n $_SA_REDETECT_FAIL ]]; then
    : # already failed on the fixture plant
elif [[ $BZR_EXIT -ne 0 ]]; then
    _SA_REDETECT_FAIL="re-detecting connect exited $BZR_EXIT"
elif [[ $(_sa_server_key auto auth_method_source) != "differential-probe" ]]; then
    _SA_REDETECT_FAIL="re-detection did not re-stamp the auth_method"
else
    case "$BZ_VERSION" in
    bz50 | bz52)
        # The versions the differential probe changed the answer for: the
        # planted `header` must be overturned, and the change announced.
        if [[ $(_sa_server_key auto auth_method) != "query_param" ]]; then
            _SA_REDETECT_FAIL="stale header was not corrected to query_param"
        elif ! grep -q "re-detected auth method" "$BZR_STDERR"; then
            _SA_REDETECT_FAIL="changed auth method was not announced on stderr"
        elif ! grep -q -- "--auth-method header" "$BZR_STDERR"; then
            _SA_REDETECT_FAIL="warning did not name the command that pins header back"
        fi
        ;;
    esac
fi
if [[ -z $_SA_REDETECT_FAIL ]]; then test_pass; else test_fail "$_SA_REDETECT_FAIL"; fi
unset _SA_REDETECT_FAIL

test_begin "auth-method-pin-survives-connect" "a pinned auth_method is neither re-detected nor re-stamped"
# Pin the method this container actually uses, so the case turns on the stamp
# rather than on an auth failure. A pin overwritten by detection would show up
# as the stamp flipping to differential-probe.
_SA_PIN_FAIL=""
_SA_PIN_METHOD=$(_sa_server_key auto auth_method)
run_bzr config set-server pinned --url "$BZ_URL" --api-key "$API_KEY" \
    --email "$ADMIN_EMAIL" --auth-method "$_SA_PIN_METHOD"
if [[ $BZR_EXIT -ne 0 ]]; then
    _SA_PIN_FAIL="config set-server pinned exited $BZR_EXIT"
else
    # First connect fills in the missing api_mode; it must not restamp.
    run_bzr --server pinned whoami
    if [[ $BZR_EXIT -ne 0 ]]; then
        _SA_PIN_FAIL="first pinned connect exited $BZR_EXIT"
    elif [[ $(_sa_server_key pinned auth_method_source) != "pinned" ]]; then
        _SA_PIN_FAIL="api_mode detection overwrote the pin marker"
    else
        # Second connect is fully cached, so no auth probe may be issued.
        RUST_LOG=bzr=debug run_bzr --server pinned whoami
        if [[ $BZR_EXIT -ne 0 ]]; then
            _SA_PIN_FAIL="cached pinned connect exited $BZR_EXIT"
        elif [[ $(_sa_server_key pinned auth_method) != "$_SA_PIN_METHOD" ]]; then
            _SA_PIN_FAIL="pinned auth_method was overwritten by re-detection"
        elif [[ $(_sa_server_key pinned auth_method_source) != "pinned" ]]; then
            _SA_PIN_FAIL="pin marker was rewritten to the detection marker"
        elif grep -q "re-detected auth method" "$BZR_STDERR"; then
            _SA_PIN_FAIL="a pinned server reported a re-detected auth method"
        else
            case "$BZ_VERSION" in
            bz50 | bz52)
                if grep -q "matched the anonymous response" "$BZR_STDERR"; then
                    _SA_PIN_FAIL="a fully-cached server still ran the auth probe"
                fi
                ;;
            esac
        fi
    fi
fi
if [[ -z $_SA_PIN_FAIL ]]; then test_pass; else test_fail "$_SA_PIN_FAIL"; fi
unset _SA_PIN_FAIL _SA_PIN_METHOD

test_begin "auth-method-stamp-ignored-without-credentials" "an unknown stamp loads and never reaches a credentialless server"
# Two properties at once: an `auth_method_source` this build does not know must
# still deserialize (or the config file becomes unreadable to every command),
# and a server with no credential must never consult or rewrite the stamp.
_SA_ANON_FAIL=""
_sa_server_set public 'auth_method = "header"' 'auth_method_source = "from-the-future"'
run_bzr --server public server info
if [[ $BZR_EXIT -ne 0 ]]; then
    _SA_ANON_FAIL="credentialless connect with an unknown stamp exited $BZR_EXIT"
elif ! jq -e '.version' "$BZR_STDOUT" >/dev/null; then
    _SA_ANON_FAIL="credentialless server info returned no version"
elif [[ $(_sa_server_key public auth_method_source) != "from-the-future" ]]; then
    _SA_ANON_FAIL="a credentialless connect rewrote the stamp"
fi
# Leave `public` as later phases expect it.
_sa_server_set public auth_method auth_method_source
if [[ -z $_SA_ANON_FAIL ]]; then test_pass; else test_fail "$_SA_ANON_FAIL"; fi
unset _SA_ANON_FAIL _SA_DETECTED_METHOD _SA_CONFIG
unset -f _sa_server_key _sa_server_set

echo ""
