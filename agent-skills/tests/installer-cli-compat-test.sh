#!/bin/sh
set -eu

# shellcheck disable=SC1007
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091
. "$HERE/lib.sh"

INSTALL_SH="$HERE/../install.sh"
INSTALL_PS1="$HERE/../install.ps1"
BZR_BIN=${BZR_BIN:?installer CLI compatibility test requires BZR_BIN}
SENTINEL=.bzr-skill-managed
TEST_ROOT=$(mktemp -d)

cleanup() {
  rm -rf "$TEST_ROOT"
}
trap cleanup EXIT HUP INT TERM

assert_current_binary_sentinel() {
  path=$1
  assert_contains "binary sentinel manager" "$(cat "$path")" "managed-by: bzr-skill"
  assert_contains "binary sentinel version" "$(cat "$path")" "source-version:"
  assert_contains "binary sentinel commit" "$(cat "$path")" "source-commit:"
}

# A real POSIX standalone install is accepted and replaced by the binary.
root="$TEST_ROOT/posix"
mkdir "$root"
BZR_SKILL_DEST_ROOT="$root" "$INSTALL_SH" --agent standard >/dev/null 2>&1
printf 'standalone marker\n' >"$root/.agents/skills/bzr-reference/standalone-only.txt"
"$BZR_BIN" skills install --agent standard --project "$root" >/dev/null
assert_no_path "binary replaced POSIX standalone payload" \
  "$root/.agents/skills/bzr-reference/standalone-only.txt"
assert_file "binary replacement kept nested content" \
  "$root/.agents/skills/bzr-reference/reference/commands.md"
assert_current_binary_sentinel \
  "$root/.agents/skills/bzr-reference/$SENTINEL"

# Always exercise the exact PowerShell byte grammar, even without pwsh.
printf '%b%b%b%b' \
  '\357\273\277managed-by: bzr-skill\r\n' \
  'installed-skill: bzr-reference\r\n' \
  'source-version: fixture\r\n' \
  'source-commit: fixture\r\n' \
  >"$root/.agents/skills/bzr-reference/$SENTINEL"
"$BZR_BIN" skills install --agent standard --project "$root" >/dev/null
assert_current_binary_sentinel \
  "$root/.agents/skills/bzr-reference/$SENTINEL"

# When PowerShell is available, exercise its real writer too.
if command -v pwsh >/dev/null 2>&1; then
  root="$TEST_ROOT/powershell"
  mkdir "$root"
  BZR_SKILL_DEST_ROOT="$root" pwsh -NoProfile -File "$INSTALL_PS1" -Agent standard \
    >/dev/null
  printf 'powershell marker\n' >"$root/.agents/skills/bzr-reference/powershell-only.txt"
  "$BZR_BIN" skills install --agent standard --project "$root" >/dev/null
  assert_no_path "binary replaced PowerShell standalone payload" \
    "$root/.agents/skills/bzr-reference/powershell-only.txt"
  assert_current_binary_sentinel \
    "$root/.agents/skills/bzr-reference/$SENTINEL"
else
  printf 'skip: pwsh unavailable; checked BOM+CRLF fixture covered\n'
fi
