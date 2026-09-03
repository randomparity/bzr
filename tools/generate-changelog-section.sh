#!/usr/bin/env bash
# Generate one release section from conventional commits.
#
# Runs git-cliff with the repo cliff.toml over PREV_TAG..TAG and inserts the
# standard security-assessment line so tools/check-release-security-notes.sh
# accepts the result. Output starts with the "## [version] - date" heading:
# CHANGELOG.md splicing keeps it, while release-note consumers strip line 1
# because GitHub notes carry no heading. If the range contains
# security-relevant commits, generation fails instead of guessing: a
# vulnerability disclosure is written by hand, never synthesized.
# Usage: generate-changelog-section.sh TAG PREV_TAG
set -euo pipefail

export LC_ALL=C

readonly NO_VULNERABILITIES='Security assessment: No publicly identified runtime vulnerabilities in bzr were fixed in this release.'
readonly GIT_CLIFF_VERSION='2.14.1'

if (($# != 2)); then
  echo "ERROR: usage: generate-changelog-section.sh TAG PREV_TAG" >&2
  exit 1
fi

tag=$1
prev_tag=$2

for tool in git-cliff awk grep; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "ERROR: $tool is required but not installed." >&2
    exit 1
  fi
done

actual_git_cliff_version=$(git-cliff --version)
if [[ $actual_git_cliff_version != "git-cliff $GIT_CLIFF_VERSION" ]]; then
  echo "ERROR: git-cliff $GIT_CLIFF_VERSION is required; found $actual_git_cliff_version." >&2
  exit 1
fi

range="${prev_tag}..${tag}"

# Security-relevant work must be disclosed by hand. Fail loudly rather than
# emitting an assessment that says nothing was fixed.
if git log "$range" --format='%s' |
  grep -Eq '^(fix|feat)\(security\)|RUSTSEC-|CVE-[0-9]+'; then
  echo "ERROR: $range contains security-relevant commits; write this release's" >&2
  echo "security assessment and any vulnerability entries by hand before tagging." >&2
  exit 1
fi

section=$(git cliff --config "$(dirname "$0")/../cliff.toml" \
  --tag "$tag" "$range")
if [ -z "$section" ]; then
  echo "ERROR: git-cliff produced no output for $range." >&2
  exit 1
fi

# Drop the configured header block: output starts at the first section heading.
notes=$(printf '%s\n' "$section" | awk '/^## \[/{found=1} found')
if ! printf '%s\n' "$notes" | head -n 1 | grep -q '^## \['; then
  echo "ERROR: generated text for $range has no release heading." >&2
  exit 1
fi

awk \
  -v assessment="$NO_VULNERABILITIES" '
  /^## \[/ {
    print
    print ""
    print "### Security"
    print ""
    print assessment
    printed_security = 1
    next
  }
  printed_security && /^### Security$/ { next }
  { print }
' <<<"$notes"
