#!/usr/bin/env bash
set -euo pipefail

block() {
  printf 'Pre-commit checks failed: %s\n' "$1" >&2
  exit 2
}

input=$(jq -cse '
  if length == 1 and (.[0] | type == "object") then
    .[0] | (.tool_input.command | type) as $type |
    if $type == "null" or $type == "string" then .
    else error("command must be a string or null") end
  else error("expected one hook input object") end
') || block 'read valid hook JSON on stdin; ensure jq is installed.'
command=$(jq -r '.tool_input.command // ""' <<<"$input")
case $command in
*'git commit'*) ;;
*) exit 0 ;;
esac

cwd=$(jq -er '.cwd | select(type == "string" and length > 0)' <<<"$input") ||
  block 'hook input must include the invocation cwd.'
root=$(git -C "$cwd" rev-parse --show-toplevel) ||
  block 'cannot resolve the invocation checkout; verify cwd and Git installation.'
make -C "$root" check-commit >&2 ||
  block 'fix the reported checks before committing; ensure make is installed.'
