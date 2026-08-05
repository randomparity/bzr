#!/bin/sh
# Verify that every canonical skill file is included in Cargo's package file list.
# Usage: package-content-check.sh [PACKAGE_LIST] [SKILLS_DIR]
set -eu

# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
ROOT=$(CDPATH= cd -- "$HERE/../.." && pwd)
SKILLS_DIR=${2:-"$ROOT/content/skills"}
PACKAGE_LIST=${1:-}
generated_list=

if [ -z "$PACKAGE_LIST" ]; then
  generated_list=$(mktemp)
  PACKAGE_LIST=$generated_list
  "${CARGO:-cargo}" package --list --allow-dirty >"$PACKAGE_LIST"
fi

skill_files=$(mktemp)
missing_files=$(mktemp)
trap 'rm -f "$generated_list" "$skill_files" "$missing_files"' EXIT
find "$SKILLS_DIR" -type f -print >"$skill_files"

checked=0
while IFS= read -r file; do
  relative=${file#"$SKILLS_DIR"/}
  packaged="content/skills/$relative"
  checked=$((checked + 1))
  if ! grep -Fqx "$packaged" "$PACKAGE_LIST"; then
    printf '%s\n' "$packaged" >>"$missing_files"
  fi
done <"$skill_files"

if [ -s "$missing_files" ]; then
  printf 'package-content-check: ERROR canonical skill files missing from Cargo package:\n' >&2
  sed 's/^/  /' "$missing_files" >&2
  exit 1
fi

printf 'package-content-check: checked %s canonical skill files\n' "$checked"
