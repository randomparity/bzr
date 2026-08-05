#!/bin/sh
# Version-contract check for the agent-skills payload.
#
# agent-skills/README.md states the contract: `agent-skills/VERSION` matches the
# crate version in Cargo.toml. Four other places assert something stronger --
# that the skills were *authored against* that CLI surface. Issue #507 found all
# five stale at 0.6.1-dev against a 0.8.1-dev crate, having shipped through two
# releases, because no check read a version: drift-check.sh compares verbs, and
# flag-drift-check.sh and skills-flag-check.sh compare flags.
#
# So this checks all five, not just VERSION. A VERSION-only check leaves the
# four prose claims free to drift on their own, which is the drift #507 reported.
#
# The prose claims are found by scanning rather than from a hardcoded path list,
# so a claim written into a new skill is covered the day it is written. At least
# one must be found: a check that passes on zero claims cannot tell "all five
# agree" from "the files moved and I read nothing".
#
# The scan joins each file's lines before matching, because the README wraps its
# claim across a line break ("...is authored\nagainst `bzr` 0.8.1-dev.").
#
# Usage: version-check.sh [CARGO_TOML] [AGENT_SKILLS_DIR]
#   CARGO_TOML        defaults to the repo-root Cargo.toml.
#   AGENT_SKILLS_DIR  defaults to agent-skills; holds VERSION, README.md, skills.
#
# Exit: 1 on any disagreement, an unreadable Cargo.toml version, a missing
#       VERSION file, or zero authored-against claims. 0 when all five agree.
set -eu

# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CARGO_TOML="${1:-$HERE/../../Cargo.toml}"
SKILLS_ROOT="${2:-$HERE/..}"
fail=0

err() {
  printf 'version-check: ERROR %s\n' "$1" >&2
  fail=1
}

if [ ! -f "$CARGO_TOML" ]; then
  err "Cargo.toml not found: $CARGO_TOML"
  exit 1
fi

# The crate version is the one in [package]; dependency tables carry their own
# `version =` lines, so the section must bound the match.
CRATE_VERSION=$(awk '
  /^\[package\]/ { pkg = 1; next }
  /^\[/          { pkg = 0 }
  pkg && /^version[[:space:]]*=/ {
    if (match($0, /"[^"]*"/)) { print substr($0, RSTART + 1, RLENGTH - 2); exit }
  }
' "$CARGO_TOML")

if [ -z "$CRATE_VERSION" ]; then
  err "no [package] version in $CARGO_TOML"
  exit 1
fi

VERSION_FILE="$SKILLS_ROOT/VERSION"
if [ ! -f "$VERSION_FILE" ]; then
  err "VERSION file not found: $VERSION_FILE"
else
  skill_version=$(cat "$VERSION_FILE")
  if [ "$skill_version" != "$CRATE_VERSION" ]; then
    err "$VERSION_FILE says $skill_version but Cargo.toml says $CRATE_VERSION"
  fi
fi

# A semver-ish literal whose optional pre-release part cannot end in punctuation,
# so a claim terminated by a period ("...bzr 0.8.1-dev.") yields "0.8.1-dev".
VERSION_RE='[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*(-[0-9A-Za-z.-]*[0-9A-Za-z])?'

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
: >"$WORK/claims"

# The documentation payload: the README plus every file under skills/. The tests
# directory is deliberately out of scope -- its fixtures name stale versions on
# purpose.
scan_targets() {
  [ -f "$SKILLS_ROOT/README.md" ] && printf '%s\n' "$SKILLS_ROOT/README.md"
  [ -d "$SKILLS_ROOT/skills" ] && find "$SKILLS_ROOT/skills" -type f | sort
  return 0
}

scan_targets | while IFS= read -r f; do
  tr '\n' ' ' <"$f" |
    grep -oE "authored[[:space:]]+against[^0-9]*$VERSION_RE" |
    grep -oE "$VERSION_RE\$" |
    awk -v file="$f" '{ print file "\t" $0 }' >>"$WORK/claims"
done

claims=0
while IFS="$(printf '\t')" read -r file claimed; do
  claims=$((claims + 1))
  if [ "$claimed" != "$CRATE_VERSION" ]; then
    err "$file: authored-against claim says $claimed but Cargo.toml says $CRATE_VERSION"
  fi
done <"$WORK/claims"

if [ "$claims" -eq 0 ]; then
  err "no 'authored against <version>' claim found under $SKILLS_ROOT"
fi

exit "$fail"
