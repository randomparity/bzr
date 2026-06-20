#!/bin/bash
# Test helper library for bzr functional tests.
# Source this file; do not execute directly.

# ── Version ──────────────────────────────────────────────────────────
BZ_VERSION="${BZR_BZ_VERSION:-bz50}"

bz_version_num() {
    case "$BZ_VERSION" in
        bz50) echo 500 ;;
        bz52) echo 520 ;;
        bz53) echo 530 ;;
        *)    echo 0 ;;
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
    CURRENT_TEST="$1"
    printf "  ${CYAN}TEST${RESET}  %s ... " "$CURRENT_TEST"
    return 0
}

test_pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    printf "${GREEN}PASS${RESET}\n"
    return 0
}

test_fail() {
    local reason="${1:-}"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    printf "${RED}FAIL${RESET}"
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
    printf "${YELLOW}SKIP${RESET}"
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
BZR_STDERR=$(mktemp /tmp/bzr-func-stderr.XXXXXX)
BZR_EXIT=0

_cleanup_tmpfiles() {
    rm -f "$BZR_STDOUT" "$BZR_STDERR"
    return 0
}
trap _cleanup_tmpfiles EXIT

run_bzr() {
    set +e
    "$BZR_BIN" --json "$@" >"$BZR_STDOUT" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
    return 0
}

# Run bzr without --json (for table/quiet tests).
run_bzr_raw() {
    set +e
    "$BZR_BIN" "$@" >"$BZR_STDOUT" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
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
