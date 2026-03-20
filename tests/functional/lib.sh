#!/bin/bash
# Test helper library for bzr functional tests.
# Source this file; do not execute directly.

# ── Version ──────────────────────────────────────────────────────────
BZ_VERSION="${BZR_BZ_VERSION:-bz50}"

bz_version_num() {
    case "$BZ_VERSION" in
        bz50) echo 500 ;;
        bz52) echo 520 ;;
        *)    echo 0 ;;
    esac
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
}

test_pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    printf "${GREEN}PASS${RESET}\n"
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
}

test_skip() {
    local reason="${1:-}"
    SKIP_COUNT=$((SKIP_COUNT + 1))
    printf "${YELLOW}SKIP${RESET}"
    if [[ -n "$reason" ]]; then
        printf "  (%s)" "$reason"
    fi
    printf "\n"
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
}
trap _cleanup_tmpfiles EXIT

run_bzr() {
    set +e
    "$BZR_BIN" --json "$@" >"$BZR_STDOUT" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
}

# Run bzr without --json (for table/quiet tests).
run_bzr_raw() {
    set +e
    "$BZR_BIN" "$@" >"$BZR_STDOUT" 2>"$BZR_STDERR"
    BZR_EXIT=$?
    set -e
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
