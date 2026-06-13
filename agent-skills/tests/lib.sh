#!/bin/sh
# Shared assert helpers for bzr-skill tests. Source this; do not execute.
# Each assert prints PASS/FAIL and increments counters in the sourcing shell.
set -eu

TESTS_RUN=0
TESTS_FAILED=0

_pass() {
  TESTS_RUN=$((TESTS_RUN + 1))
  printf '  ok   - %s\n' "$1"
}

_fail() {
  TESTS_RUN=$((TESTS_RUN + 1))
  TESTS_FAILED=$((TESTS_FAILED + 1))
  printf '  FAIL - %s\n' "$1"
  [ -n "${2:-}" ] && printf '         %s\n' "$2"
}

# assert_eq <name> <expected> <actual>
assert_eq() {
  if [ "$2" = "$3" ]; then _pass "$1"; else _fail "$1" "expected [$2] got [$3]"; fi
}

# assert_contains <name> <haystack> <needle>
assert_contains() {
  case "$2" in
  *"$3"*) _pass "$1" ;;
  *) _fail "$1" "[$2] does not contain [$3]" ;;
  esac
}

# assert_not_contains <name> <haystack> <needle>
assert_not_contains() {
  case "$2" in
  *"$3"*) _fail "$1" "[$2] unexpectedly contains [$3]" ;;
  *) _pass "$1" ;;
  esac
}

# assert_file <name> <path>  -- passes if a regular file exists
assert_file() {
  if [ -f "$2" ]; then _pass "$1"; else _fail "$1" "no file at $2"; fi
}

# assert_no_path <name> <path> -- passes if nothing exists at path
assert_no_path() {
  if [ -e "$2" ] || [ -L "$2" ]; then _fail "$1" "unexpected path $2"; else _pass "$1"; fi
}

# report; call at end of a test file. Exits non-zero if any failed.
report() {
  printf '%s: %d run, %d failed\n' "${1:-tests}" "$TESTS_RUN" "$TESTS_FAILED"
  [ "$TESTS_FAILED" -eq 0 ]
}
