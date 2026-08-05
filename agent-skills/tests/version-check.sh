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
# Claims are checked from both ends. The four sites the contract names are
# REQUIRED to carry one, so rewording a heading cannot silently drop a version
# stamp -- a check whose only floor is "at least one claim somewhere" reports a
# clean tree after three of the four are deleted. Everything else under skills/
# is scanned too, so a claim written into a new skill is covered the day it is
# written without editing this file.
#
# The scan joins each file's lines before matching, because the README wraps its
# claim across a line break ("...is authored\nagainst `bzr` 0.8.1-dev."). The
# gaps around `bzr` are bounded: an unbounded gap latches "authored against the
# current CLI surface" onto an unrelated version literal hundreds of characters
# later.
#
# Usage: version-check.sh [CARGO_TOML] [AGENT_SKILLS_DIR]
#   CARGO_TOML        defaults to the repo-root Cargo.toml.
#   AGENT_SKILLS_DIR  defaults to agent-skills; holds VERSION, README.md, skills.
#
# Exit: 1 on any disagreement, an unreadable Cargo.toml version, a missing
#       VERSION file, or a required site with no claim. 0 when all five agree.
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
# Matched case-insensitively: the same claim at the start of a sentence is the
# same claim. The gaps are wide enough for the phrasings an author reaches for
# ("against the current `bzr` X", "against bzr version X") and no wider -- an
# unbounded gap matches this repo's own README, which names the claim shape as
# documentation.
CLAIM_RE="authored[[:space:]]+against[^0-9]{0,20}bzr[^0-9]{0,12}$VERSION_RE"

# The sites the README's contract names. Each must carry a claim.
REQUIRED_SITES='README.md
skills/bzr-reference/SKILL.md
skills/bzr-reference/reference/commands.md
skills/bzr-reference/reference/commands.yml'

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
    grep -oiE "$CLAIM_RE" |
    grep -oE "$VERSION_RE\$" |
    awk -v file="$f" '{ print file "\t" $0 }' >>"$WORK/claims"
done

while IFS="$(printf '\t')" read -r file claimed; do
  if [ "$claimed" != "$CRATE_VERSION" ]; then
    err "$file: authored-against claim says $claimed but Cargo.toml says $CRATE_VERSION"
  fi
done <"$WORK/claims"

cut -f1 "$WORK/claims" | sort -u >"$WORK/claimed-files"

printf '%s\n' "$REQUIRED_SITES" | while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  if ! grep -qxF "$SKILLS_ROOT/$rel" "$WORK/claimed-files"; then
    printf 'version-check: ERROR %s: no "authored against bzr <version>" claim\n' \
      "$SKILLS_ROOT/$rel" >&2
    printf 'missing\n' >>"$WORK/missing"
  fi
done

if [ -f "$WORK/missing" ]; then
  fail=1
fi

exit "$fail"
