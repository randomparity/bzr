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

if ! awk '
  function strip_marker_indent(line) {
    if (substr(line, 1, 3) == "   ") {
      return substr(line, 4)
    }
    if (substr(line, 1, 2) == "  ") {
      return substr(line, 3)
    }
    if (substr(line, 1, 1) == " ") {
      return substr(line, 2)
    }
    return line
  }

  function is_fence_marker(line, marker, character, run_length) {
    marker = strip_marker_indent(line)
    character = substr(marker, 1, 1)
    if (character != "`" && character != "~") {
      return 0
    }
    run_length = 0
    while (substr(marker, run_length + 1, 1) == character) {
      run_length++
    }
    return run_length >= 3
  }

  is_fence_marker($0) || index($0, "<!--") != 0 || index($0, "-->") != 0 { exit 1 }
' <"$changelog_file"; then
	echo "ERROR: CHANGELOG.md must not contain fenced-code markers or HTML-comment delimiters." >&2
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

  function is_release_heading(line, separator, release_version, date) {
    if (line == "## [Unreleased]") {
      return 1
    }
    if (index(line, "## [") != 1) {
      return 0
    }
    separator = index(line, "] - ")
    if (separator <= 5) {
      return 0
    }
    release_version = substr(line, 5, separator - 5)
    date = substr(line, separator + 4)
    return release_version != "" && length(date) == 10 && \
      date ~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/
  }

  is_candidate_heading($0) {
    candidate_count++
    capture = candidate_count == 1
    next
  }
  capture && is_release_heading($0) {
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
