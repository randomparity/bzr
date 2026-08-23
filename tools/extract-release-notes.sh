#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

if (($# != 2)); then
	echo "ERROR: release-note extraction requires a changelog file and version." >&2
	exit 1
fi

changelog_file=$1
version=$2
if [[ ! -f $changelog_file || ! -r $changelog_file ]]; then
	echo "ERROR: release-note extraction requires a readable regular changelog file." >&2
	exit 1
fi
if [[ -z $version ]]; then
	echo "ERROR: release-note extraction requires a non-empty version." >&2
	exit 1
fi

awk -v version="$version" '
  function is_candidate_heading(line, prefix, date) {
    prefix = "## [" version "]"
    if (version == "Unreleased") {
      return line == prefix
    }
    prefix = prefix " - "
    if (index(line, prefix) != 1 || length(line) != length(prefix) + 10) {
      return 0
    }
    date = substr(line, length(prefix) + 1)
    return date ~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/
  }

  is_candidate_heading($0) {
    candidate_count++
    capture = candidate_count == 1
    next
  }
  capture && index($0, "## [") == 1 {
    capture = 0
  }
  capture {
    print
  }
  END {
    if (candidate_count != 1) {
      print "ERROR: CHANGELOG.md must contain exactly one literal candidate release heading." > "/dev/stderr"
      exit 1
    }
  }
' <"$changelog_file"
