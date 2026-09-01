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

    test_id="$CURRENT_TEST_GROUP/$slug"
    case $SEEN_TEST_IDS in
    *$'\n'"$test_id"$'\n'*)
        printf "test_begin: duplicate functional test ID '%s'\n" "$test_id" >&2
        return 2
        ;;
    esac

    SEEN_TEST_IDS="${SEEN_TEST_IDS}${test_id}"$'\n'
    CURRENT_TEST="$description"
    printf "  ${CYAN}TEST${RESET}  [%s] %s ... " "$test_id" "$CURRENT_TEST"
    return 0
}

test_pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    printf '%bPASS%b\n' "$GREEN" "$RESET"
    return 0
}

test_fail() {
    local reason="${1:-}"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    printf '%bFAIL%b' "$RED" "$RESET"
    if [[ -n "$reason" ]]; then
        printf "  (%s)" "$reason"
    fi
    printf "\n"
    # Print captured stdout/stderr for debugging
    if [[ -f "$BZR_STDOUT" ]]; then
        echo "    stdout: $(head -5 "$BZR_STDOUT")"
    fi
    if [[ -f "$BZR_STDERR" ]]; then
        echo "    stderr: $(head -5 "$BZR_STDERR")"
    fi
    return 0
}

test_skip() {
    local reason="${1:-}"
    SKIP_COUNT=$((SKIP_COUNT + 1))
    printf '%bSKIP%b' "$YELLOW" "$RESET"
    if [[ -n "$reason" ]]; then
        printf "  (%s)" "$reason"
    fi
    printf "\n"
    return 0
}

test_summary() {
    echo ""
    echo "════════════════════════════════════════════════════════════"
    printf "  ${GREEN}PASSED: %d${RESET}  " "$PASS_COUNT"
    printf "${RED}FAILED: %d${RESET}  " "$FAIL_COUNT"
    printf "${YELLOW}SKIPPED: %d${RESET}\n" "$SKIP_COUNT"
    echo "  TOTAL:  $((PASS_COUNT + FAIL_COUNT + SKIP_COUNT))"
    echo "════════════════════════════════════════════════════════════"

    if [[ $FAIL_COUNT -gt 0 ]]; then
        return 1
    fi
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
    TLS_PORT="${BZR_FUNC_TLS_PORT:-$((backend_port + 1000))}"
    TLS_FIXTURE_DIR=$(mktemp -d /tmp/bzr-func-tls.XXXXXX)
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
    REDHAT_SHAPE_PORT="${BZR_FUNC_REDHAT_PORT:-$((backend_port + 2000))}"
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
