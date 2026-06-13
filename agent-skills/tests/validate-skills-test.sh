#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"

VALIDATE="$HERE/validate-skills.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

mkgood() {
  d="$WORK/skills/$1"
  mkdir -p "$d"
  printf -- '---\nname: %s\ndescription: %s\n---\n\nbody\n' "$1" "$2" >"$d/SKILL.md"
}

# valid skill passes
mkdir -p "$WORK/skills"
mkgood "bzr-reference" "Use bzr to work with Bugzilla bugs."
out=$("$VALIDATE" "$WORK/skills" 2>&1) && rc=0 || rc=$?
assert_eq "valid skill exits 0" "0" "$rc"

# name mismatch fails
rm -rf "$WORK/skills"
mkdir -p "$WORK/skills/bzr-x"
printf -- '---\nname: wrong\ndescription: x\n---\n' >"$WORK/skills/bzr-x/SKILL.md"
out=$("$VALIDATE" "$WORK/skills" 2>&1) && rc=0 || rc=$?
assert_eq "name mismatch exits non-zero" "1" "$rc"
assert_contains "name mismatch message" "$out" "name"

# overlong description fails (501 chars)
rm -rf "$WORK/skills"
mkdir -p "$WORK/skills/bzr-y"
long=$(printf 'a%.0s' $(seq 1 501))
printf -- '---\nname: bzr-y\ndescription: %s\n---\n' "$long" >"$WORK/skills/bzr-y/SKILL.md"
out=$("$VALIDATE" "$WORK/skills" 2>&1) && rc=0 || rc=$?
assert_eq "overlong description exits non-zero" "1" "$rc"
assert_contains "overlong message" "$out" "500"

# empty description fails
rm -rf "$WORK/skills"
mkdir -p "$WORK/skills/bzr-z"
printf -- '---\nname: bzr-z\ndescription:\n---\n' >"$WORK/skills/bzr-z/SKILL.md"
out=$("$VALIDATE" "$WORK/skills" 2>&1) && rc=0 || rc=$?
assert_eq "empty description exits non-zero" "1" "$rc"

# broken reference link fails
rm -rf "$WORK/skills"
mkdir -p "$WORK/skills/bzr-w"
printf -- '---\nname: bzr-w\ndescription: ok\n---\nSee [cmds](reference/missing.md)\n' >"$WORK/skills/bzr-w/SKILL.md"
out=$("$VALIDATE" "$WORK/skills" 2>&1) && rc=0 || rc=$?
assert_eq "broken link exits non-zero" "1" "$rc"
assert_contains "broken link message" "$out" "missing.md"

report "validate-skills-test"
