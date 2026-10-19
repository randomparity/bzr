#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
FIXTURES=$(mktemp -d)
trap 'rm -rf "$FIXTURES"' EXIT
mkdir -p "$FIXTURES/bin" "$FIXTURES/invocation tree/subdir"
for tool in bash jq git make; do
  ln -s "$(command -v "$tool")" "$FIXTURES/bin/$tool"
done
# A distinct checkout makes accidental use of CLAUDE_PROJECT_DIR observable.
git init -q "$FIXTURES/invocation tree"
cp "$ROOT/Makefile" "$FIXTURES/invocation tree/Makefile"
cat >"$FIXTURES/bin/cargo" <<'CARGO'
#!/bin/sh
printf '%s\n' "$PWD|$*" >>"$HOOK_TEST_LOG"
printf 'check output\n'
case "$*" in
  "$HOOK_TEST_FAIL"*) exit 101 ;;
esac
CARGO
chmod +x "$FIXTURES/bin/cargo"
HOOK=$(jq -r '.hooks.PreToolUse[] | select(.matcher == "Bash") | .hooks[].command' \
  "$ROOT/.claude/settings.json")
export CLAUDE_PROJECT_DIR="$ROOT" HOOK_TEST_LOG="$FIXTURES/calls" HOOK_TEST_FAIL=never
unset CLAUDE_TOOL_INPUT
PAYLOAD=$(jq -nc --arg cwd "$FIXTURES/invocation tree/subdir" \
  '{cwd:$cwd,tool_input:{command:"git commit -m wip"}}')

expect() {
  local name=$1 input=$2 expected_status=$3 expected_calls=$4 status=0
  : >"$HOOK_TEST_LOG"
  CARGO=cargo VERBOSE=0 PATH="$FIXTURES/bin" /bin/bash -c "$HOOK" <<<"$input" \
    >"$FIXTURES/stdout" 2>"$FIXTURES/stderr" || status=$?
  if [[ $status != "$expected_status" || $(wc -l <"$HOOK_TEST_LOG" | tr -d ' ') != "$expected_calls" || -s "$FIXTURES/stdout" ]]; then
    printf 'FAIL: %s (exit %s, expected %s; expected %s checks)\n' \
      "$name" "$status" "$expected_status" "$expected_calls" >&2
    cat "$HOOK_TEST_LOG" "$FIXTURES/stderr" >&2
    exit 1
  fi
  if [[ $expected_status == 2 && ! -s "$FIXTURES/stderr" ]]; then
    printf 'FAIL: %s lacks failure diagnostics\n' "$name" >&2
    exit 1
  fi
}

expect success "$PAYLOAD" 0 3
expected_root=$(cd "$FIXTURES/invocation tree" && pwd -P)
printf '%s\n' "$expected_root|fmt -- --check" \
  "$expected_root|clippy --all-targets --features test-helpers -- -D warnings" \
  "$expected_root|test --quiet" >"$FIXTURES/expected"
diff -u "$FIXTURES/expected" "$HOOK_TEST_LOG"
for check in fmt clippy test; do
  HOOK_TEST_FAIL=$check
  case $check in fmt) count=1 ;; clippy) count=2 ;; test) count=3 ;; esac
  expect "failed $check" "$PAYLOAD" 2 "$count"
done
HOOK_TEST_FAIL=never
for command in 'ls -la' '' null; do
  input=$(jq -nc --arg command "$command" \
    '{tool_input:{command:(if $command == "null" then null else $command end)}}')
  expect "unrelated $command" "$input" 0 0
done
expect missing-command '{}' 0 0
for input in '' '{' '[]' 'null' '{"tool_input":{"command":42}}' '{"tool_input":{"command":false}}'; do
  expect malformed "$input" 2 0
done
expect missing-cwd '{"tool_input":{"command":"git commit"}}' 2 0
input=$(jq -c --arg cwd "$FIXTURES/absent" '.cwd=$cwd' <<<"$PAYLOAD")
expect unavailable-cwd "$input" 2 0
for tool in jq git make; do
  mv "$FIXTURES/bin/$tool" "$FIXTURES/$tool"
  expect "missing $tool" "$PAYLOAD" 2 0
  mv "$FIXTURES/$tool" "$FIXTURES/bin/$tool"
done
printf 'Claude hook regression tests passed\n'
