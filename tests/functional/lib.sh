#!/bin/bash
# Test helper library for bzr functional tests.
# Source this file; do not execute directly.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tests/functional/container-env.sh
source "$SCRIPT_DIR/container-env.sh"

# ── Version ──────────────────────────────────────────────────────────

bz_version_num() {
    case "$BZ_VERSION" in
    bz50) echo 500 ;;
    bz52) echo 520 ;;
    bz53) echo 530 ;;
    *) echo 0 ;;
    esac
    return 0
}

# Skip test if version is below minimum. Usage: require_version 520 "reason"
require_version() {
    local min="$1"
    local reason="${2:-requires newer Bugzilla}"
    if [[ $(bz_version_num) -lt $min ]]; then
        test_skip "$reason"
        return 1
    fi
    return 0
}

# ── Counters ─────────────────────────────────────────────────────────
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
GAP_COUNT=0
LAST_TEST_RESULT=""
LAST_TEST_REASON=""
LAST_GAP_ISSUE=""
LAST_TEST_SHOW_CAPTURE=0
TEST_RESULT_PENDING=0
GAP_APPLIED=0
CURRENT_TEST=""
CURRENT_TEST_GROUP=""
SEEN_TEST_IDS=$'\n'

# ── Colors ───────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    CYAN='\033[0;36m'
    RESET='\033[0m'
else
    GREEN='' RED='' YELLOW='' CYAN='' RESET=''
fi

# ── Test lifecycle ───────────────────────────────────────────────────

_render_test_result() {
    if [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        return 0
    fi

    case "$LAST_TEST_RESULT" in
    PASS)
        printf '%bPASS%b\n' "$GREEN" "$RESET"
        ;;
    FAIL)
        printf '%bFAIL%b' "$RED" "$RESET"
        if [[ -n "$LAST_TEST_REASON" ]]; then
            printf '  (%s)' "$LAST_TEST_REASON"
        fi
        printf '\n'
        if [[ $LAST_TEST_SHOW_CAPTURE -eq 1 ]]; then
            if [[ -f "$BZR_STDOUT" ]]; then
                echo "    stdout: $(head -5 "$BZR_STDOUT")"
            fi
            if [[ -f "$BZR_STDERR" ]]; then
                echo "    stderr: $(head -5 "$BZR_STDERR")"
            fi
        fi
        ;;
    GAP)
        printf '%bGAP%b (#%s)\n' "$YELLOW" "$RESET" "$LAST_GAP_ISSUE"
        ;;
    SKIP)
        printf '%bSKIP%b' "$YELLOW" "$RESET"
        if [[ -n "$LAST_TEST_REASON" ]]; then
            printf '  (%s)' "$LAST_TEST_REASON"
        fi
        printf '\n'
        ;;
    esac
    TEST_RESULT_PENDING=0
    return 0
}

test_begin() {
    if [[ $# -ne 2 ]]; then
        printf 'test_begin: expected exactly 2 arguments, got %d\n' "$#" >&2
        return 2
    fi

    local slug="$1"
    local description="$2"
    local phase_re='^[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*$'
    local slug_re='^[a-z0-9]+(-[a-z0-9]+)*$'
    local test_id

    if [[ ! $CURRENT_TEST_GROUP =~ $phase_re ]]; then
        printf "test_begin: invalid functional test group '%s'\n" "$CURRENT_TEST_GROUP" >&2
        return 2
    fi
    if [[ ! $slug =~ $slug_re ]]; then
        printf "test_begin: invalid functional test slug '%s'\n" "$slug" >&2
        return 2
    fi

    test_id="${TEST_ID_PREFIX:+$TEST_ID_PREFIX/}$CURRENT_TEST_GROUP/$slug"
    case $SEEN_TEST_IDS in
    *$'\n'"$test_id"$'\n'*)
        printf "test_begin: duplicate functional test ID '%s'\n" "$test_id" >&2
        return 2
        ;;
    esac

    _render_test_result
    SEEN_TEST_IDS="${SEEN_TEST_IDS}${test_id}"$'\n'
    CURRENT_TEST="$description"
    LAST_TEST_RESULT=""
    LAST_TEST_REASON=""
    LAST_GAP_ISSUE=""
    LAST_TEST_SHOW_CAPTURE=0
    TEST_RESULT_PENDING=0
    GAP_APPLIED=0
    printf "  ${CYAN}TEST${RESET}  [%s] %s ... " "$test_id" "$CURRENT_TEST"
    return 0
}

test_pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    LAST_TEST_RESULT="PASS"
    LAST_TEST_REASON=""
    LAST_TEST_SHOW_CAPTURE=0
    TEST_RESULT_PENDING=1
    if [[ ${TEST_ID_PREFIX:-} != compare ]]; then
        _render_test_result
    fi
    return 0
}

test_fail() {
    local reason="${1:-}"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    LAST_TEST_RESULT="FAIL"
    LAST_TEST_REASON="$reason"
    LAST_TEST_SHOW_CAPTURE=1
    TEST_RESULT_PENDING=1
    if [[ ${TEST_ID_PREFIX:-} != compare ]]; then
        _render_test_result
    fi
    return 0
}

test_skip() {
    local reason="${1:-}"
    SKIP_COUNT=$((SKIP_COUNT + 1))
    LAST_TEST_RESULT="SKIP"
    LAST_TEST_REASON="$reason"
    LAST_TEST_SHOW_CAPTURE=0
    TEST_RESULT_PENDING=1
    _render_test_result
    return 0
}

test_summary() {
    _render_test_result
    echo ""
    echo "════════════════════════════════════════════════════════════"
    printf "  ${GREEN}PASSED: %d${RESET}  " "$PASS_COUNT"
    printf "${RED}FAILED: %d${RESET}  " "$FAIL_COUNT"
    if [[ ${TEST_ID_PREFIX:-} == compare ]]; then
        printf "${YELLOW}SKIPPED: %d${RESET}  " "$SKIP_COUNT"
        printf "${YELLOW}GAPS: %d${RESET}\n" "$GAP_COUNT"
        echo "  TOTAL:  $((PASS_COUNT + FAIL_COUNT + SKIP_COUNT + GAP_COUNT))"
    else
        printf "${YELLOW}SKIPPED: %d${RESET}\n" "$SKIP_COUNT"
        echo "  TOTAL:  $((PASS_COUNT + FAIL_COUNT + SKIP_COUNT))"
    fi
    echo "════════════════════════════════════════════════════════════"

    if [[ ${TEST_ID_PREFIX:-} == compare && -n ${GITHUB_STEP_SUMMARY:-} ]]; then
        {
            printf '## bzr/python-bugzilla comparison summary\n\n'
            printf '| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n'
            printf '| --- | ---: | ---: | ---: | ---: |\n'
            printf '| %s | %d | %d | %d | %d |\n\n' \
                "$BZ_VERSION" "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_COUNT" "$GAP_COUNT"
        } >>"$GITHUB_STEP_SUMMARY"
    fi

    if [[ $FAIL_COUNT -gt 0 ]]; then
        return 1
    fi
    return 0
}

expect_gap() {
    local issue="${1:-}"

    if [[ $# -ne 1 || ! $issue =~ ^[1-9][0-9]*$ ]]; then
        printf 'expect_gap: expected one positive decimal issue number\n' >&2
        return 2
    fi
    if [[ $GAP_APPLIED -ne 0 ]]; then
        printf 'expect_gap: an expected gap was already applied to this test\n' >&2
        return 2
    fi
    if [[ $TEST_RESULT_PENDING -ne 1 ]]; then
        printf 'expect_gap: the current test has no pass or fail outcome\n' >&2
        return 2
    fi

    case "$LAST_TEST_RESULT" in
    FAIL)
        FAIL_COUNT=$((FAIL_COUNT - 1))
        GAP_COUNT=$((GAP_COUNT + 1))
        LAST_TEST_RESULT="GAP"
        LAST_TEST_REASON=""
        LAST_GAP_ISSUE="$issue"
        LAST_TEST_SHOW_CAPTURE=0
        GAP_APPLIED=1
        _render_test_result
        ;;
    PASS)
        PASS_COUNT=$((PASS_COUNT - 1))
        FAIL_COUNT=$((FAIL_COUNT + 1))
        LAST_TEST_RESULT="FAIL"
        LAST_TEST_REASON="expected gap issue #$issue appears resolved"
        LAST_TEST_SHOW_CAPTURE=0
        GAP_APPLIED=1
        _render_test_result
        ;;
    *)
        printf 'expect_gap: the current test has no pass or fail outcome\n' >&2
        return 2
        ;;
    esac
    return 0
}

# ── Core: run bzr ────────────────────────────────────────────────────

# Temp files for capturing output (created once per session).
BZR_STDOUT=$(mktemp /tmp/bzr-func-stdout.XXXXXX)
# Raw, unprojected stdout bytes — used by envelope assertions; $BZR_STDOUT holds
# the projected payload (see _project_envelope).
BZR_STDOUT_RAW=$(mktemp /tmp/bzr-func-stdout-raw.XXXXXX)
BZR_STDERR=$(mktemp /tmp/bzr-func-stderr.XXXXXX)
BZR_EXIT=0
# Read by comparison phases after observe_bzr_transport returns.
# shellcheck disable=SC2034
BZR_TRANSPORT=""
PYBZ_RUNTIME=""

# Match the outer record, with the default tracing timestamp or the fixtures' bare prefix.
# Spell out digit widths because older mawk does not support interval expressions.
BZR_TRACING_PREFIX_RE='^([0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T'
BZR_TRACING_PREFIX_RE+='[0-9][0-9]:[0-9][0-9]:[0-9][0-9][.]'
BZR_TRACING_PREFIX_RE+='[0-9][0-9][0-9][0-9][0-9][0-9]Z[[:space:]]+)?DEBUG[[:space:]]+'
BZR_REST_BOUNDARY_RE="${BZR_TRACING_PREFIX_RE}bzr::client::transport: "
BZR_REST_BOUNDARY_RE+='(strict )?API response([[:space:]]|$)'
BZR_XMLRPC_BOUNDARY_RE="${BZR_TRACING_PREFIX_RE}bzr::xmlrpc::protocol::client: "
BZR_XMLRPC_BOUNDARY_RE+='XML-RPC call([[:space:]]|$)'

# Assigns public state read by sourced comparison phases.
# shellcheck disable=SC2034
observe_bzr_transport() {
    local counts rest_count xmlrpc_count

    BZR_TRANSPORT=""
    if ! counts=$(awk -v rest_re="$BZR_REST_BOUNDARY_RE" \
        -v xmlrpc_re="$BZR_XMLRPC_BOUNDARY_RE" '
        $0 ~ rest_re { rest += 1 }
        $0 ~ xmlrpc_re { xmlrpc += 1 }
        END { print rest + 0, xmlrpc + 0 }
    ' "$BZR_STDERR"); then
        printf 'could not read bzr transport observations\n' >&2
        return 1
    fi
    read -r rest_count xmlrpc_count <<<"$counts"
    case "$((rest_count > 0)):$((xmlrpc_count > 0))" in
    1:0) BZR_TRANSPORT=REST ;;
    0:1) BZR_TRANSPORT=XMLRPC ;;
    0:0)
        printf 'bzr transport observation is missing\n' >&2
        return 1
        ;;
    *)
        printf 'bzr transport observation is ambiguous (REST and XML-RPC)\n' >&2
        return 1
        ;;
    esac
    return 0
}

_cleanup_tmpfiles() {
    rm -f "$BZR_STDOUT" "$BZR_STDOUT_RAW" "$BZR_STDERR"
    return 0
}
trap _cleanup_tmpfiles EXIT

# Pretty `--json` output is wrapped in the {schema_version, data} envelope (see
# docs/bzr-cli.md). Project `.data` into $BZR_STDOUT so existing payload
# assertions read it directly; non-enveloped output (schema documents,
# completion scripts, NDJSON, raw download bytes, table text) is copied through
# verbatim. $BZR_STDOUT_RAW always keeps the literal bytes for envelope checks.
_project_envelope() {
    if jq -e 'type=="object" and has("schema_version") and has("data")' \
        "$BZR_STDOUT_RAW" >/dev/null 2>&1; then
        jq '.data' "$BZR_STDOUT_RAW" >"$BZR_STDOUT"
    else
        cp "$BZR_STDOUT_RAW" "$BZR_STDOUT"
    fi
}

run_bzr() {
    set +e
    "$BZR_BIN" --json "$@" >"$BZR_STDOUT_RAW" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
    _project_envelope
    return 0
}

# Run bzr without --json (for table/quiet tests).
run_bzr_raw() {
    set +e
    "$BZR_BIN" "$@" >"$BZR_STDOUT_RAW" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
    _project_envelope
    return 0
}

pybz_image_name() {
    local checkout_id

    checkout_id=$(bugzilla_checkout_id) || return 1
    printf 'localhost/bzr-pybz-%s:3.3.0' "$checkout_id"
    return 0
}

pybz_sidecar_name() {
    local checkout_id

    checkout_id=$(bugzilla_checkout_id) || return 1
    printf 'bzr-pybz-%s-%s' "$BZ_VERSION" "$checkout_id"
    return 0
}

pybz_home_volume_name() {
    local checkout_id

    checkout_id=$(bugzilla_checkout_id) || return 1
    printf 'bzr-pybz-home-%s-%s' "$BZ_VERSION" "$checkout_id"
    return 0
}

pybz_sidecar_start() {
    if [[ $# -ne 2 ]]; then
        printf 'pybz_sidecar_start: expected runtime and Bugzilla container\n' >&2
        return 2
    fi
    if [[ ! -d ${FUNC_CONFIG_DIR:-} ]]; then
        printf 'pybz_sidecar_start: FUNC_CONFIG_DIR must name a directory\n' >&2
        return 2
    fi

    local runtime="$1"
    local bugzilla_container="$2"
    local image
    local sidecar
    local home_volume

    image=$(pybz_image_name) || return 1
    sidecar=$(pybz_sidecar_name) || return 1
    home_volume=$(pybz_home_volume_name) || return 1
    if ! "$runtime" container inspect "$bugzilla_container" >/dev/null 2>&1; then
        printf 'pybz_sidecar_start: Bugzilla container not found: %s\n' "$bugzilla_container" >&2
        return 1
    fi
    "$runtime" build -t "$image" -f "$SCRIPT_DIR/pybz/Containerfile" "$SCRIPT_DIR/pybz"
    if "$runtime" container inspect "$sidecar" >/dev/null 2>&1; then
        if [[ $("$runtime" container inspect --format '{{.State.Running}}' "$sidecar") == true ]]; then
            printf 'pybz_sidecar_start: sidecar is already running: %s; stop the active comparison first\n' \
                "$sidecar" >&2
            return 1
        fi
        "$runtime" rm "$sidecar" >/dev/null
    fi

    "$runtime" run -d \
        --name "$sidecar" \
        --network "container:${bugzilla_container}" \
        --volume "${FUNC_CONFIG_DIR}:/work:Z" \
        --volume "${home_volume}:/home/pybz" \
        --env HOME=/home/pybz \
        "$image" >/dev/null
    PYBZ_RUNTIME="$runtime"
    return 0
}

pybz_sidecar_stop() {
    if [[ $# -ne 1 ]]; then
        printf 'pybz_sidecar_stop: expected runtime\n' >&2
        return 2
    fi

    local runtime="$1"
    local sidecar

    sidecar=$(pybz_sidecar_name) || return 1
    if "$runtime" container inspect "$sidecar" >/dev/null 2>&1; then
        if ! "$runtime" rm -f "$sidecar" >/dev/null; then
            printf 'pybz_sidecar_stop: could not remove sidecar: %s\n' "$sidecar" >&2
            return 1
        fi
    fi
    PYBZ_RUNTIME=""
    return 0
}

pybz_stage_proxy() {
    if [[ $# -ne 2 || ! -f $1 || ! -r $1 || -L $1 || ! -d ${COMPARE_EXCHANGE_DIR:-} ]]; then
        printf 'pybz_stage_proxy: expected a readable repository file and exchange directory\n' >&2
        return 2
    fi
    local source="$1" destination="$2" source_dir staged
    source_dir=$(cd "$(dirname "$source")" && pwd -P) || return 1
    if [[ $source_dir != "$SCRIPT_DIR" || ! $destination =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
        printf 'pybz_stage_proxy: source or destination is outside the fixed proxy surface\n' >&2
        return 2
    fi
    staged="$COMPARE_EXCHANGE_DIR/$destination"
    cp "$source" "$staged" || return 1
    chmod 600 "$staged" || return 1
    printf '/work/compare/%s\n' "$destination"
}
_pybz_proxy_pid_alive() {
    local sidecar
    sidecar=$(pybz_sidecar_name) || return 1
    # shellcheck disable=SC2016 # $1 is expanded by the sidecar shell.
    "$PYBZ_RUNTIME" exec "$sidecar" sh -c '# pybz-proxy-alive
kill -0 "$1" 2>/dev/null' sh "$1"
}
pybz_proxy_start() {
    if [[ $# -lt 2 || $# -gt 3 || ! $2 =~ ^[0-9]+$ || ${#2} -gt 5 ||
        $2 -lt 1 || $2 -gt 65535 ||
        -z ${PYBZ_RUNTIME:-} || ! -d ${COMPARE_EXCHANGE_DIR:-} ]]; then
        printf 'pybz_proxy_start: expected kind, decimal port, and optional certificate directory\n' >&2
        return 2
    fi
    local kind="$1" port="$2" cert_dir="${3:-}" script log pid_file old_pid pid pid_temp
    case "$kind" in
    redhat)
        [[ $# -eq 2 ]] || return 2
        script=/work/compare/redhat-proxy.py
        ;;
    tls)
        if [[ $# -ne 3 || ! $cert_dir =~ ^[A-Za-z0-9][A-Za-z0-9._/-]*$ ||
            $cert_dir == *..* ]]; then
            printf 'pybz_proxy_start: invalid certificate directory\n' >&2
            return 2
        fi
        script=/work/compare/tls-proxy.py
        ;;
    *)
        printf 'pybz_proxy_start: kind must be tls or redhat\n' >&2
        return 2
        ;;
    esac
    local host_script="$COMPARE_EXCHANGE_DIR/${script##*/}"
    if [[ ! -f $host_script || ! -r $host_script || -L $host_script ]]; then
        printf 'pybz_proxy_start: staged proxy is missing or unreadable\n' >&2
        return 1
    fi
    if [[ $kind == tls &&
        (! -f ${FUNC_CONFIG_DIR:-}/$cert_dir/server.crt ||
            ! -f ${FUNC_CONFIG_DIR:-}/$cert_dir/server.key) ]]; then
        printf 'pybz_proxy_start: certificate material is missing\n' >&2
        return 1
    fi
    log="$COMPARE_EXCHANGE_DIR/${kind}.proxy.log"
    pid_file="$COMPARE_EXCHANGE_DIR/${kind}.proxy.pid"
    if [[ -e $pid_file ]]; then
        IFS= read -r old_pid <"$pid_file" || true
        if [[ ! $old_pid =~ ^[1-9][0-9]*$ ]]; then
            printf 'pybz_proxy_start: malformed prior PID for %s\n' "$kind" >&2
            return 1
        fi
        if _pybz_proxy_pid_alive "$old_pid"; then
            printf 'pybz_proxy_start: %s proxy is already running\n' "$kind" >&2
            return 1
        fi
        rm -f "$pid_file"
    fi
    local log_temp
    log_temp=$(mktemp "$COMPARE_EXCHANGE_DIR/.${kind}.proxy.log.XXXXXX") || return 1
    chmod 600 "$log_temp" || return 1
    mv "$log_temp" "$log" || return 1
    local sidecar
    sidecar=$(pybz_sidecar_name) || return 1
    # shellcheck disable=SC2016 # positional values expand only in the sidecar shell.
    pid=$("$PYBZ_RUNTIME" exec "$sidecar" sh -c '# pybz-proxy-start
set -eu
if [ "$1" = tls ]; then
  python "$2" "$3" 127.0.0.1 80 "/work/$4/server.crt" \
    "/work/$4/server.key" >"$5" 2>&1 &
else
  BZR_FUNC_REDHAT_MODE=bearer-auth python "$2" "$3" 80 >"$5" 2>&1 &
fi
printf "%s\n" "$!"' sh "$kind" "$script" "$port" "$cert_dir" \
        "/work/compare/${kind}.proxy.log") || return 1
    if [[ ! $pid =~ ^[1-9][0-9]*$ ]]; then
        printf 'pybz_proxy_start: proxy returned an invalid PID\n' >&2
        return 1
    fi
    pid_temp=$(mktemp "$COMPARE_EXCHANGE_DIR/.${kind}.proxy.pid.XXXXXX") || return 1
    printf '%s\n' "$pid" >"$pid_temp"
    chmod 600 "$pid_temp" || return 1
    mv "$pid_temp" "$pid_file" || return 1
    local attempt=0
    while [[ $attempt -lt 30 ]]; do
        if "$PYBZ_RUNTIME" exec "$sidecar" python -c \
            'import socket,sys; socket.create_connection(("127.0.0.1", int(sys.argv[1])), 1).close()' \
            "$port" >/dev/null 2>&1; then
            printf '%s\n' "$log"
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    local diagnostic='no startup detail'
    if grep -Fq 'Address already in use' "$log"; then
        diagnostic='port unavailable'
    elif [[ -s $log ]]; then
        diagnostic='proxy exited during startup'
    fi
    printf 'pybz_proxy_start: %s proxy was not ready after 30 attempts (%s)\n' \
        "$kind" "$diagnostic" >&2
    pybz_proxy_stop "$kind" || true
    return 1
}
pybz_proxy_stop() {
    if [[ $# -ne 1 || $1 != tls && $1 != redhat || -z ${PYBZ_RUNTIME:-} ||
        ! -d ${COMPARE_EXCHANGE_DIR:-} ]]; then
        printf 'pybz_proxy_stop: expected tls or redhat with an active sidecar\n' >&2
        return 2
    fi
    local kind="$1" pid_file="$COMPARE_EXCHANGE_DIR/${1}.proxy.pid" pid sidecar attempt
    [[ -e $pid_file ]] || return 0
    IFS= read -r pid <"$pid_file" || true
    if [[ ! $pid =~ ^[1-9][0-9]*$ ]]; then
        printf 'pybz_proxy_stop: malformed PID for %s\n' "$kind" >&2
        return 1
    fi
    sidecar=$(pybz_sidecar_name) || return 1
    if _pybz_proxy_pid_alive "$pid"; then
        # shellcheck disable=SC2016 # $1 is expanded by the sidecar shell.
        "$PYBZ_RUNTIME" exec "$sidecar" sh -c '# pybz-proxy-stop
kill -TERM "$1" 2>/dev/null || true' sh "$pid" || return 1
        attempt=0
        while _pybz_proxy_pid_alive "$pid" && [[ $attempt -lt 30 ]]; do
            sleep 0.1
            attempt=$((attempt + 1))
        done
        if _pybz_proxy_pid_alive "$pid"; then
            # shellcheck disable=SC2016 # $1 is expanded by the sidecar shell.
            "$PYBZ_RUNTIME" exec "$sidecar" sh -c 'kill -KILL "$1" 2>/dev/null || true' \
                sh "$pid" || return 1
        fi
    fi
    rm -f "$pid_file"
}
pybz_redhat_alias_install() {
    if [[ -z ${PYBZ_RUNTIME:-} ]]; then
        printf 'pybz_redhat_alias_install: expected an active sidecar\n' >&2
        return 2
    fi
    local sidecar
    sidecar=$(pybz_sidecar_name) || return 1
    "$PYBZ_RUNTIME" exec "$sidecar" sh -c '# pybz-redhat-alias
grep -Eq "^[[:space:]]*127\\.0\\.0\\.1[[:space:]]+bugzilla\\.redhat\\.com([[:space:]]|$)" \
  /etc/hosts || printf "127.0.0.1 bugzilla.redhat.com\\n" >>/etc/hosts'
}
_run_pybz_command() {
    local command="$1"
    local sidecar
    shift

    if [[ -z $PYBZ_RUNTIME ]]; then
        printf '_run_pybz_command: sidecar has not been started\n' >&2
        return 2
    fi
    sidecar=$(pybz_sidecar_name) || return 1
    set +e
    "$PYBZ_RUNTIME" exec "$sidecar" "$command" "$@" >"$BZR_STDOUT_RAW" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
    _project_envelope
    return 0
}

run_pybz() { _run_pybz_command bugzilla "$@"; }

run_pybz_adapter() {
    _run_pybz_command python /work/compare/python-bugzilla-adapter.py "$@"
}

pybz_write_api_key_identity_request() (
    if [[ $# -ne 4 || ! ${1:-} =~ ^[a-z0-9][a-z0-9-]*$ ||
        ! -d ${COMPARE_EXCHANGE_DIR:-} ]]; then
        printf 'pybz_write_api_key_identity_request: expected safe name, URL, API key, and username\n' >&2
        return 2
    fi
    local name="$1" url="$2" api_key="$3" username="$4"
    local output="$COMPARE_EXCHANGE_DIR/${name}.pybz.input.json"
    local url_source="$COMPARE_EXCHANGE_DIR/.api-key-identity.url.source" status=0
    local key_source="$COMPARE_EXCHANGE_DIR/.api-key-identity.key.source"
    local username_source="$COMPARE_EXCHANGE_DIR/.api-key-identity.username.source"
    umask 077
    trap 'rm -f -- "$url_source" "$key_source" "$username_source" || :' EXIT
    printf '%s' "$url" >"$url_source"
    printf '%s' "$api_key" >"$key_source"
    printf '%s' "$username" >"$username_source"
    jq -ecn --rawfile url "$url_source" --rawfile api_key "$key_source" \
        --rawfile username "$username_source" \
        '{url:$url, api_key:$api_key, username:$username}' >"$output" || status=$?
    if [[ $status -ne 0 ]]; then
        rm -f -- "$output"
        return "$status"
    fi
    chmod 600 "$output"
    printf '%s\n' "$output"
)
# Shared mechanics for resource comparison phases.
RESOURCE_GAP_ELIGIBLE=0
RESOURCE_GAP_FILE=""
RESOURCE_SERVER="compare-resource"
RESOURCE_MEMBERSHIPS=""

resource_init() {
    if [[ ! -d ${COMPARE_EXCHANGE_DIR:-} ]]; then
        printf 'resource_init: COMPARE_EXCHANGE_DIR must name a directory\n' >&2
        return 2
    fi
    run_bzr config set-server "$RESOURCE_SERVER" --url "$BZ_URL" \
        --api-key-env BZR_COMPARE_API_KEY --email "$COMPARE_ADMIN_EMAIL" \
        --auth-method query_param
    if [[ $BZR_EXIT -ne 0 ]]; then
        printf 'resource_init: could not configure matching query-parameter auth\n' >&2
        return 1
    fi
    RESOURCE_GAP_ELIGIBLE=0
    RESOURCE_GAP_FILE="$COMPARE_EXCHANGE_DIR/.resource-gap-eligible"
    RESOURCE_MEMBERSHIPS=""
}

resource_membership_record() {
    local user="$1" group="$2" entry

    entry="$user"$'\t'"$group"
    if [[ $'\n'${RESOURCE_MEMBERSHIPS}$'\n' != *$'\n'"$entry"$'\n'* ]]; then
        RESOURCE_MEMBERSHIPS="${RESOURCE_MEMBERSHIPS:+${RESOURCE_MEMBERSHIPS}$'\n'}$entry"
    fi
}

resource_membership_clear() {
    local user="$1" group="$2" entry candidate
    local remaining=""

    entry="$user"$'\t'"$group"
    while IFS= read -r candidate; do
        if [[ -n $candidate && $candidate != "$entry" ]]; then
            remaining="${remaining:+${remaining}$'\n'}$candidate"
        fi
    done <<<"$RESOURCE_MEMBERSHIPS"
    RESOURCE_MEMBERSHIPS="$remaining"
}

resource_membership_cleanup() {
    local entry user group status=0

    while IFS= read -r entry; do
        [[ -n $entry ]] || continue
        IFS=$'\t' read -r user group <<<"$entry"
        run_bzr --server "$RESOURCE_SERVER" group remove-user --group "$group" --user "$user"
        if [[ $BZR_EXIT -ne 0 ]]; then
            printf 'could not clean comparison membership for %s in %s\n' "$user" "$group" >&2
            status=1
        fi
    done <<<"$RESOURCE_MEMBERSHIPS"
    RESOURCE_MEMBERSHIPS=""
    return "$status"
}

resource_name_is_safe() {
    [[ $1 =~ ^[a-z0-9][a-z0-9-]*$ ]]
}

resource_capture_bzr() {
    local name="$1"

    cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/${name}.bzr.stdout.json"
    cp "$BZR_STDOUT_RAW" "$COMPARE_EXCHANGE_DIR/${name}.bzr.raw"
    cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/${name}.bzr.stderr"
    printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/${name}.bzr.exit"
}

resource_bzr() {
    local name="$1" api="$2" expected_transport="$3"
    shift 3

    if ! resource_name_is_safe "$name"; then
        test_fail "invalid resource capture name"
        return 1
    fi
    RUST_LOG=bzr=debug run_bzr --server "$RESOURCE_SERVER" --api "$api" "$@"
    resource_capture_bzr "$name"
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "bzr $name failed with exit $BZR_EXIT"
        return 1
    fi
    if ! jq -e . "$BZR_STDOUT" >/dev/null; then
        test_fail "bzr $name returned invalid JSON evidence"
        return 1
    fi
    if ! observe_bzr_transport || [[ $BZR_TRANSPORT != "$expected_transport" ]]; then
        test_fail "bzr $name did not prove $expected_transport transport"
        return 1
    fi
    printf '%s\n' "$BZR_TRANSPORT" >"$COMPARE_EXCHANGE_DIR/${name}.bzr.transport"
}

resource_pybz() {
    local name="$1" operation="$2" payload="$3" expected_transport="$4"
    local input output transport_filter

    if ! resource_name_is_safe "$name"; then
        test_fail "invalid resource capture name"
        return 1
    fi
    input="$COMPARE_EXCHANGE_DIR/${name}.pybz.input.json"
    output="$COMPARE_EXCHANGE_DIR/${name}.pybz.output.json"
    if ! jq -ecn --arg api_key "$BZR_COMPARE_API_KEY" --argjson payload "$payload" \
        '$payload | select(type == "object") | . + {api_key:$api_key}' >"$input"; then
        test_fail "python-bugzilla $name request is invalid"
        return 1
    fi
    chmod 600 "$input"
    run_pybz_adapter "$operation" "/work/compare/${input##*/}" \
        "/work/compare/${output##*/}"
    cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/${name}.pybz.stdout"
    cp "$BZR_STDOUT_RAW" "$COMPARE_EXCHANGE_DIR/${name}.pybz.raw"
    cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/${name}.pybz.stderr"
    printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.exit"
    if [[ $expected_transport == LOCAL ]]; then
        transport_filter='.transport == null'
    else
        transport_filter=".transport == \$expected"
    fi
    if [[ $BZR_EXIT -ne 0 || ! -r $output ]] ||
        ! jq -e --arg expected "$expected_transport" \
            "type == \"object\" and (keys | sort) == [\"result\",\"transport\"] and
             ($transport_filter)" "$output" >/dev/null; then
        test_fail "python-bugzilla $name failed to prove $expected_transport transport"
        return 1
    fi
    jq '.result' "$output" >"$COMPARE_EXCHANGE_DIR/${name}.pybz.result.json"
    jq -r '.transport // "LOCAL"' "$output" \
        >"$COMPARE_EXCHANGE_DIR/${name}.pybz.transport"
}

resource_positive_id() {
    local path="$1" expression="$2"

    jq -er "$expression | select(type == \"number\" and floor == . and . > 0)" "$path"
}

resource_require_positive_id() {
    local path="$1" expression="$2" label="$3"

    if ! resource_positive_id "$path" "$expression" >/dev/null; then
        test_fail "$label returned an invalid ID"
        return 1
    fi
}

resource_equal() {
    local name="$1" left="$2" right="$3"

    if ! diff -u "$left" "$right" >"$COMPARE_EXCHANGE_DIR/${name}.diff"; then
        test_fail "normalized $name differs"
        return 1
    fi
}

resource_gap_reset() {
    RESOURCE_GAP_ELIGIBLE=0
    rm -f "$RESOURCE_GAP_FILE"
}

resource_gap_allow() {
    RESOURCE_GAP_ELIGIBLE=1
    : >"$RESOURCE_GAP_FILE"
}

resource_expect_gap() {
    local issue="$1"

    if [[ $LAST_TEST_RESULT == PASS ]] ||
        [[ $RESOURCE_GAP_ELIGIBLE -eq 1 && -f $RESOURCE_GAP_FILE ]]; then
        expect_gap "$issue"
    fi
}

seed_comparison_attachment_flag_type() {
    local sql_file="$COMPARE_EXCHANGE_DIR/attachment-flag.sql"
    local result status=0

    printf '%s\n' \
        "INSERT INTO flagtypes (name, description, target_type, is_active," \
        "  is_requestable, is_requesteeble, is_multiplicable, sortkey)" \
        "SELECT 'bzr_compare_attachment_review'," \
        "  'bzr python-bugzilla comparison attachment flag', 'a', 1, 1, 1, 1, 30" \
        "WHERE NOT EXISTS (SELECT 1 FROM flagtypes" \
        "  WHERE name = 'bzr_compare_attachment_review' AND target_type = 'a');" \
        "INSERT INTO flaginclusions (type_id, product_id, component_id)" \
        "SELECT id, NULL, NULL FROM flagtypes" \
        "WHERE name = 'bzr_compare_attachment_review' AND target_type = 'a'" \
        "  AND NOT EXISTS (SELECT 1 FROM flaginclusions" \
        "    WHERE flaginclusions.type_id = flagtypes.id" \
        "      AND flaginclusions.product_id IS NULL" \
        "      AND flaginclusions.component_id IS NULL);" \
        "SELECT" \
        "  (SELECT COUNT(*) FROM flagtypes" \
        "    WHERE name = 'bzr_compare_attachment_review'" \
        "      AND target_type = 'a') AS flag_type_count," \
        "  (SELECT COUNT(*) FROM flaginclusions" \
        "    JOIN flagtypes ON flagtypes.id = flaginclusions.type_id" \
        "    WHERE flagtypes.name = 'bzr_compare_attachment_review'" \
        "      AND flagtypes.target_type = 'a'" \
        "      AND flaginclusions.product_id IS NULL" \
        "      AND flaginclusions.component_id IS NULL)" \
        "    AS unrestricted_inclusion_count;" >"$sql_file"
    chmod 600 "$sql_file"
    if ! result=$(run_bugzilla_sql_file "$sql_file"); then
        printf 'could not seed comparison attachment flag type\n' >&2
        status=1
    elif [[ ${result##*$'\n'} != $'1\t1' ]]; then
        printf 'comparison attachment flag type readback was not exactly one type and inclusion\n' >&2
        status=1
    fi
    rm -f "$sql_file"
    return "$status"
}

# ── Assertions ───────────────────────────────────────────────────────

assert_success() {
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "expected exit 0, got $BZR_EXIT"
        return 1
    fi
}

assert_failure() {
    if [[ $BZR_EXIT -eq 0 ]]; then
        test_fail "expected non-zero exit, got 0"
        return 1
    fi
}

assert_exit_code() {
    local expected="$1"
    if [[ $BZR_EXIT -ne $expected ]]; then
        test_fail "expected exit $expected, got $BZR_EXIT"
        return 1
    fi
}

# assert_json <jq-expr> <expected-value>
assert_json() {
    local expr="$1"
    local expected="$2"
    local actual
    actual=$(jq -r "$expr" "$BZR_STDOUT" 2>/dev/null)
    if [[ "$actual" != "$expected" ]]; then
        test_fail "jq '$expr' = '$actual', expected '$expected'"
        return 1
    fi
}

# assert_raw_json <jq-expr> <expected-value> — like assert_json but against the
# raw, unprojected stdout (the {schema_version, data} envelope), for asserting
# the envelope itself rather than the projected payload.
assert_raw_json() {
    local expr="$1"
    local expected="$2"
    local actual
    actual=$(jq -r "$expr" "$BZR_STDOUT_RAW" 2>/dev/null)
    if [[ "$actual" != "$expected" ]]; then
        test_fail "raw jq '$expr' = '$actual', expected '$expected'"
        return 1
    fi
}

# assert_stderr_json <jq-expr> <expected-value> — jq against the captured
# stderr, where the structured `error` object lands under --json on failure.
#
# stderr is a mixed stream: tracing diagnostics share it with the structured
# error object. Some are one-shot per server (the "URL is not HTTPS — API key
# will be sent in plaintext" warning fires on a credentialed server's first
# connect), so parsing the whole file makes an assertion pass or fail on
# whether an unrelated earlier test happened to warm that server. Select the
# last JSON object line instead — the error object is always emitted compact,
# one line, and last.
assert_stderr_json() {
    local expr="$1"
    local expected="$2"
    local actual
    actual=$(grep '^{' "$BZR_STDERR" 2>/dev/null | tail -1 | jq -r "$expr" 2>/dev/null)
    if [[ "$actual" != "$expected" ]]; then
        test_fail "stderr jq '$expr' = '$actual', expected '$expected'"
        return 1
    fi
}

# assert_json_contains <jq-expr> <substring>
assert_json_contains() {
    local expr="$1"
    local substring="$2"
    local actual
    actual=$(jq -r "$expr" "$BZR_STDOUT" 2>/dev/null)
    if [[ "$actual" != *"$substring"* ]]; then
        test_fail "jq '$expr' = '$actual', does not contain '$substring'"
        return 1
    fi
}

# assert_json_array_min_length <jq-expr> <min-length>
assert_json_array_min_length() {
    local expr="$1"
    local min_len="$2"
    local actual_len
    actual_len=$(jq "$expr | length" "$BZR_STDOUT" 2>/dev/null)
    if [[ -z "$actual_len" ]] || [[ "$actual_len" -lt "$min_len" ]]; then
        test_fail "jq '$expr' length = ${actual_len:-null}, expected >= $min_len"
        return 1
    fi
}

# assert_json_array_length <jq-expr> <exact-length>
assert_json_array_length() {
    local expr="$1"
    local expected_len="$2"
    local actual_len
    actual_len=$(jq "$expr | length" "$BZR_STDOUT" 2>/dev/null)
    if [[ "$actual_len" != "$expected_len" ]]; then
        test_fail "jq '$expr' length = ${actual_len:-null}, expected $expected_len"
        return 1
    fi
}

# assert_stdout_contains <substring>
assert_stdout_contains() {
    local substring="$1"
    if ! grep -q "$substring" "$BZR_STDOUT" 2>/dev/null; then
        test_fail "stdout does not contain '$substring'"
        return 1
    fi
}

# assert_stdout_not_contains <substring>
assert_stdout_not_contains() {
    local substring="$1"
    if grep -q "$substring" "$BZR_STDOUT" 2>/dev/null; then
        test_fail "stdout unexpectedly contains '$substring'"
        return 1
    fi
}

# assert_stdout_empty
assert_stdout_empty() {
    if [[ -s "$BZR_STDOUT" ]]; then
        test_fail "expected empty stdout"
        return 1
    fi
}

# assert_stderr_empty
assert_stderr_empty() {
    if [[ -s "$BZR_STDERR" ]]; then
        test_fail "expected empty stderr"
        return 1
    fi
}

# assert_file_contains <path> <string>
assert_file_contains() {
    local path="$1"
    local string="$2"
    if ! grep -q "$string" "$path" 2>/dev/null; then
        test_fail "file '$path' does not contain '$string'"
        return 1
    fi
}

# assert_json_exists <jq-expr> — value is not null/empty
assert_json_exists() {
    local expr="$1"
    local actual
    actual=$(jq -r "$expr" "$BZR_STDOUT" 2>/dev/null)
    if [[ -z "$actual" ]] || [[ "$actual" == "null" ]]; then
        test_fail "jq '$expr' is null or empty"
        return 1
    fi
}

# assert_json_not_contains <jq-expr> <substring> — substring is ABSENT from the
# jq result. Backs the discriminating-fixture exclusion checks (assert a
# non-matching fixture id is not in a filtered result).
assert_json_not_contains() {
    local expr="$1"
    local substring="$2"
    local actual
    actual=$(jq -r "$expr" "$BZR_STDOUT" 2>/dev/null)
    if [[ "$actual" == *"$substring"* ]]; then
        test_fail "jq '$expr' = '$actual', unexpectedly contains '$substring'"
        return 1
    fi
}

# assert_json_valid — stdout parses as JSON (schema / structured-output checks).
assert_json_valid() {
    if ! jq -e . "$BZR_STDOUT" >/dev/null 2>&1; then
        test_fail "stdout is not valid JSON"
        return 1
    fi
}

# assert_count <n> — assert the {"count": n} shape emitted by --count. Use only
# on per-run-unique marker-isolated fixtures, never the shared corpus.
assert_count() {
    local expected="$1"
    local actual
    actual=$(jq -r '.count' "$BZR_STDOUT" 2>/dev/null)
    if [[ "$actual" != "$expected" ]]; then
        test_fail "count = ${actual:-null}, expected $expected"
        return 1
    fi
}

# assert_ndjson_line_count <n> — number of non-empty NDJSON lines on stdout.
assert_ndjson_line_count() {
    local expected="$1"
    local actual
    actual=$(grep -c '[^[:space:]]' "$BZR_STDOUT" 2>/dev/null || true)
    if [[ "$actual" != "$expected" ]]; then
        test_fail "ndjson line count = ${actual:-0}, expected $expected"
        return 1
    fi
}

# assert_stderr_contains <substring> — grep stderr (conflict, dry-run, and
# truncation notices route there). Pairs with exit-code checks so a conflict
# test proves the reason, not just that exit 2 fired.
assert_stderr_contains() {
    local substring="$1"
    if ! grep -q "$substring" "$BZR_STDERR" 2>/dev/null; then
        test_fail "stderr does not contain '$substring'"
        return 1
    fi
}

# assert_stderr_not_contains <substring> — assert a substring is ABSENT from
# stderr. Pairs with progress-stream checks (e.g. no `done` event on failure).
assert_stderr_not_contains() {
    local substring="$1"
    if grep -q "$substring" "$BZR_STDERR" 2>/dev/null; then
        test_fail "stderr unexpectedly contains '$substring'"
        return 1
    fi
}

# ── Fixtures ─────────────────────────────────────────────────────────

# make_bug [--marker <tag>] <bzr bug create args...> — create a bug and echo its
# id. A marker stamps a whiteboard tag (caller passes a per-run-unique value) so
# filter/paging/count tests isolate their own fixtures from the shared, growing
# corpus.
#
# On failure it logs a diagnostic to stderr (visible in the run log even from a
# `$(...)` capture) and echoes an empty id, but ALWAYS returns 0. This is
# deliberate: callers use `id=$(make_bug ...)`, and under the suite's
# `set -euo pipefail` a non-zero return from a command substitution would abort
# the whole run. Returning 0 keeps the run alive — the caller sees an empty id
# and the dependent test fails on its own assertion, so one bad create is one
# failed test, not a silent total-suite abort.
make_bug() {
    local marker=""
    if [[ "${1:-}" == "--marker" ]]; then
        marker="$2"
        shift 2
    fi
    if [[ -n "$marker" ]]; then
        run_bzr bug create --whiteboard "$marker" "$@"
    else
        run_bzr bug create "$@"
    fi
    if [[ $BZR_EXIT -ne 0 ]]; then
        echo "make_bug: bug create failed (exit $BZR_EXIT): $(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 0
    fi
    jq -r '.id' "$BZR_STDOUT" 2>/dev/null || true
}

# wait_for_changed <bug_id> <prev_last_change_time> — poll `bug view` until the
# bug's last_change_time is strictly greater than the given value, so the forced
# mid-air-collision test is deterministic (last_change_time is second-granular).
# ISO-8601 timestamps compare lexically in chronological order. Returns non-zero
# if the timestamp has not advanced within the retry budget.
wait_for_changed() {
    local bug_id="$1"
    local prev="$2"
    local attempt=0
    local current
    while [[ $attempt -lt 30 ]]; do
        run_bzr bug view "$bug_id"
        current=$(jq -r '.last_change_time // empty' "$BZR_STDOUT" 2>/dev/null)
        if [[ -n "$current" ]] && [[ "$current" > "$prev" ]]; then
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    return 1
}

# unique_name <prefix> — per-run fixture id safe for Bugzilla names.
unique_name() {
    local prefix="$1"
    printf '%s-%s-%s' "$prefix" "$$" "$RANDOM"
    return 0
}

# write_json_fixture <path> <json> — writes compact JSON without a trailing
# shell-expanded newline surprise.
write_json_fixture() {
    local path="$1"
    local json="$2"
    printf '%s' "$json" >"$path"
    return 0
}

# assert_stdout_equals_file <path> — raw stdout exactly matches file bytes.
assert_stdout_equals_file() {
    local path="$1"
    if ! cmp -s "$BZR_STDOUT" "$path"; then
        test_fail "stdout does not exactly match '$path'"
        return 1
    fi
}

# assert_schema_list_contains <name> — schema list stdout contains a schema name.
assert_schema_list_contains() {
    local name="$1"
    assert_json_exists "index(\"$name\")"
}

# run_bugzilla_sql_file <path> — execute SQL inside the running Bugzilla
# container. Use this only for fixture capabilities that Bugzilla's public API
# cannot create, such as flag types and product group controls.
run_bugzilla_sql_file() {
    local sql_file="$1"
    local runtime
    local container
    runtime=$(container_runtime)
    container=$(bugzilla_container_name)
    "$runtime" exec -i "$container" mysql -u root bugs <"$sql_file"
}

# ── Group non-member fixture (issue #617) ────────────────────────────
# A second, *enabled* user who is not a member of the functional test group.
# Asserting that `group list-users --group <g>` honors its filter needs one: an
# added member appears in the listing whether or not the filter is applied, and
# the absence half proves nothing against a user the server would have hidden
# anyway. The login deliberately shares no substring with `testuser`, so the
# existing `user search testuser` and `assert_stdout_not_contains
# "testuser@test.bzr"` assertions cannot match it.
NONMEMBER_EMAIL="functest-nonmember@test.bzr"

# ensure_enabled_nonmember_user — create $NONMEMBER_EMAIL if absent and make
# sure it can log in. Idempotent in both halves: an "already exists" create is
# success, and the enable runs unconditionally so a prior run that disabled the
# user is repaired. The password is explicit because omitting it makes the
# server generate one and mail it, and this harness configures no mail path.
# Overwrites the BZR_* capture globals. Returns non-zero with a diagnostic on
# stderr when either half fails.
#
# This establishes *exists* and *enabled*, not *not a member* — that half holds
# only because nothing adds this login to a group, and containers are reused
# between runs (setup-bugzilla.sh reuses one per checkout+version), so group
# membership survives. A phase that adds $NONMEMBER_EMAIL to a group must
# remove it, or it breaks the next run's non-membership assertion.
ensure_enabled_nonmember_user() {
    run_bzr user create --email "$NONMEMBER_EMAIL" \
        --full-name "Enabled Non-Member" --password "TestPass1!"
    if [[ $BZR_EXIT -ne 0 ]] && ! grep -q "already" "$BZR_STDERR" 2>/dev/null; then
        echo "ensure_enabled_nonmember_user: create failed (exit $BZR_EXIT):" \
            "$(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 1
    fi
    run_bzr user update "$NONMEMBER_EMAIL" --disable-login false --login-denied-text ""
    if [[ $BZR_EXIT -ne 0 ]]; then
        echo "ensure_enabled_nonmember_user: enable failed (exit $BZR_EXIT):" \
            "$(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 1
    fi
    return 0
}

# assert_user_login_enabled <login> — fail the current test unless the server
# reports can_login true for exactly <login>. This is the half a dependent
# assertion rests on: a fixture that silently degraded to disabled would make an
# absence assertion pass for the same wrong reason the current one does.
# Overwrites the BZR_* capture globals, so call it before capturing output you
# still need.
assert_user_login_enabled() {
    local login="$1"
    run_bzr user search "$login" --details
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "user search '$login' --details exited $BZR_EXIT"
        return 1
    fi
    assert_json "[.[] | select(.name == \"$login\")][0].can_login" "true"
}

# assert_user_group_membership <login> <group> <in|out> — assert that <login>'s
# own membership set does or does not contain <group>, read from the `groups`
# field `user search --details` already returns (USER_FIELDS_DETAILED,
# src/client/mod.rs:22). This reads the *user* resource, not
# `group list-users`, so it is independent of the group filter #625 owns.
#
# Two ways an `out` assertion could pass for the wrong reason, both closed
# below: the login might be missing from the result set entirely, which the
# presence assertion inside this helper rejects; or `groups` might be empty for
# every user because the credential cannot see membership, which callers reject
# by pairing this with an `in` assertion on a user known to be a member.
# Overwrites the BZR_* capture globals.
assert_user_group_membership() {
    local login="$1"
    local group="$2"
    local want="$3"
    local expected
    case "$want" in
    in) expected=1 ;;
    out) expected=0 ;;
    *)
        test_fail "assert_user_group_membership: want must be in|out, got '$want'"
        return 1
        ;;
    esac
    run_bzr user search "$login" --details
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "user search '$login' --details exited $BZR_EXIT"
        return 1
    fi
    # Presence first. The membership filter below yields 0 both when <login>
    # holds no <group> and when <login> is absent from the result set entirely,
    # so without this an `out` assertion passes against a fixture that was never
    # created — the same pass-for-the-wrong-reason this fixture exists to
    # remove. The absence half has to prove the row is there before it can mean
    # anything. The `in` control on a different login cannot cover this: it
    # proves the credential can see membership, not that *this* row exists.
    assert_json "[.[] | select(.name == \"$login\")] | length" "1" || return 1
    assert_json \
        "[[.[] | select(.name == \"$login\")][0].groups[]? | select(.name == \"$group\")] | length" \
        "$expected"
}

# ── TLS fixture (issue #406) ─────────────────────────────────────────
# Front the HTTP-only Bugzilla container with HTTPS via a python3
# TLS-terminating reverse proxy so the ad-hoc --server-tls-* trust flags can be
# exercised end-to-end. State is published through globals consumed by
# phases/02c-tls-inline.sh: TLS_PORT, TLS_CA_CERT, TLS_GOOD_PIN, TLS_FIXTURE_DIR,
# TLS_FIXTURE_PID. See docs/superpowers/specs/2026-06-23-issue-406-*.md.
TLS_FIXTURE_DIR=""
TLS_FIXTURE_PID=""
TLS_PORT=""
TLS_CA_CERT=""
TLS_GOOD_PIN=""

functional_proxy_default_port() {
    local backend_port="$1"
    local offset="$2"
    if [[ $((backend_port + offset)) -le 65535 ]]; then
        printf '%d\n' "$((backend_port + offset))"
    else
        printf '%d\n' "$((backend_port - offset))"
    fi
}

# tls_tools_available — returns 0 iff the host tooling for the HTTPS fixture is
# present. When it is not, the TLS phase skips cleanly so run-all-versions stays
# predictable on hosts without TLS tooling.
tls_tools_available() {
    command -v python3 >/dev/null 2>&1 &&
        command -v openssl >/dev/null 2>&1 &&
        command -v curl >/dev/null 2>&1
}

# tls_fixture_start <backend_port> — generate a CA + leaf cert, compute the leaf
# pin, launch the TLS reverse proxy in front of 127.0.0.1:<backend_port>, and
# wait for it to accept TLS. Returns non-zero (after cleaning up) if the proxy
# does not become ready, so the caller can fail the phase without hanging.
tls_fixture_start() {
    local backend_port="$1"
    TLS_PORT="${BZR_FUNC_TLS_PORT:-$(functional_proxy_default_port "$backend_port" 1000)}"
    if [[ -d ${FUNC_CONFIG_DIR:-} ]]; then
        TLS_FIXTURE_DIR=$(mktemp -d "$FUNC_CONFIG_DIR/tls.XXXXXX")
    else
        TLS_FIXTURE_DIR=$(mktemp -d /tmp/bzr-func-tls.XXXXXX)
    fi
    chmod 700 "$TLS_FIXTURE_DIR"
    TLS_CA_CERT="$TLS_FIXTURE_DIR/ca.pem"

    local ca_key="$TLS_FIXTURE_DIR/ca.key"
    local key="$TLS_FIXTURE_DIR/server.key"
    local crt="$TLS_FIXTURE_DIR/server.crt"
    local csr="$TLS_FIXTURE_DIR/server.csr"
    local ext="$TLS_FIXTURE_DIR/ext.cnf"

    # Self-signed CA (trust anchor for the --server-tls-ca-cert case).
    openssl req -x509 -newkey rsa:2048 -nodes \
        -keyout "$ca_key" -out "$TLS_CA_CERT" \
        -subj "/CN=bzr-func-test-ca" -days 2 \
        -addext "basicConstraints=critical,CA:TRUE" \
        -addext "keyUsage=critical,keyCertSign,cRLSign" >/dev/null 2>&1

    # Leaf signed by the CA. The IP SAN lets --server-tls-ca-cert pass full
    # hostname verification when connecting to https://127.0.0.1; serverAuth EKU
    # is required by rustls.
    openssl req -newkey rsa:2048 -nodes \
        -keyout "$key" -out "$csr" -subj "/CN=127.0.0.1" >/dev/null 2>&1
    cat >"$ext" <<'EOF'
subjectAltName=IP:127.0.0.1,DNS:localhost
basicConstraints=CA:FALSE
keyUsage=digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
EOF
    openssl x509 -req -in "$csr" -CA "$TLS_CA_CERT" -CAkey "$ca_key" \
        -CAcreateserial -out "$crt" -days 2 -extfile "$ext" >/dev/null 2>&1
    chmod 600 "$ca_key" "$TLS_CA_CERT" "$key" "$crt" "$csr" "$ext"

    # Pin = sha256// + base64(SHA-256(leaf DER)), matching
    # src/tls/fingerprint.rs::compute_fingerprint.
    # shellcheck disable=SC2034 # consumed by phases/02c-tls-inline.sh
    TLS_GOOD_PIN="sha256//$(openssl x509 -in "$crt" -outform DER |
        openssl dgst -sha256 -binary | openssl base64 -A)"

    python3 "$SCRIPT_DIR/tls-proxy.py" "$TLS_PORT" 127.0.0.1 "$backend_port" \
        "$crt" "$key" >"$TLS_FIXTURE_DIR/proxy.log" 2>&1 &
    TLS_FIXTURE_PID=$!
    # Drop the proxy from job control so killing it in _tls_cleanup does not emit
    # a "Terminated" job notice into the test output.
    disown "$TLS_FIXTURE_PID" 2>/dev/null || true

    local attempt=0
    while [[ $attempt -lt 30 ]]; do
        if curl -sk "https://127.0.0.1:${TLS_PORT}/rest/version" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done

    echo "tls_fixture_start: proxy not ready on port ${TLS_PORT} after 30s" >&2
    tail -5 "$TLS_FIXTURE_DIR/proxy.log" >&2 2>/dev/null || true
    _tls_cleanup
    return 1
}

# _tls_cleanup — stop the proxy and remove the temp certs. Idempotent and
# set -u-safe: every variable is guarded, an absent/dead PID and an
# already-removed dir are tolerated, and it is safe to call before
# tls_fixture_start or more than once (it composes onto the EXIT trap).
_tls_cleanup() {
    if [[ -n "${TLS_FIXTURE_PID:-}" ]]; then
        kill "$TLS_FIXTURE_PID" >/dev/null 2>&1 || true
        TLS_FIXTURE_PID=""
    fi
    if [[ -n "${TLS_FIXTURE_DIR:-}" ]] && [[ -d "$TLS_FIXTURE_DIR" ]]; then
        rm -rf "$TLS_FIXTURE_DIR"
        TLS_FIXTURE_DIR=""
    fi
    return 0
}

# ── Red Hat response-shape fixture (issue #589) ─────────────────────
REDHAT_SHAPE_PORT=""
REDHAT_SHAPE_PID=""
REDHAT_SHAPE_LOG=""

redhat_shape_start() {
    local backend_port="$1"
    REDHAT_SHAPE_PORT="${BZR_FUNC_REDHAT_PORT:-$(functional_proxy_default_port "$backend_port" 2000)}"
    REDHAT_SHAPE_LOG="$FUNC_CONFIG_DIR/redhat-shape-proxy.log"
    python3 "$SCRIPT_DIR/redhat-shape-proxy.py" "$REDHAT_SHAPE_PORT" \
        "$backend_port" >"$REDHAT_SHAPE_LOG" 2>&1 &
    REDHAT_SHAPE_PID=$!

    local attempt=0
    while [[ $attempt -lt 30 ]]; do
        if curl -sf "http://127.0.0.1:${REDHAT_SHAPE_PORT}/_bzr_ready" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    echo "redhat_shape_start: proxy not ready; log: $REDHAT_SHAPE_LOG" >&2
    tail -5 "$REDHAT_SHAPE_LOG" >&2 2>/dev/null || true
    redhat_shape_stop
    return 1
}

redhat_shape_stop() {
    if [[ -n "${REDHAT_SHAPE_PID:-}" ]]; then
        local pid="$REDHAT_SHAPE_PID"
        kill "$pid" >/dev/null 2>&1 || true
        local attempt=0
        while kill -0 "$pid" >/dev/null 2>&1 && [[ $attempt -lt 30 ]]; do
            sleep 0.1
            attempt=$((attempt + 1))
        done
        if kill -0 "$pid" >/dev/null 2>&1; then
            kill -9 "$pid" >/dev/null 2>&1 || true
        fi
        wait "$pid" 2>/dev/null || true
        REDHAT_SHAPE_PID=""
        if kill -0 "$pid" >/dev/null 2>&1; then
            echo "redhat_shape_stop: proxy process $pid is still running" >&2
            return 1
        fi
    fi
    return 0
}
