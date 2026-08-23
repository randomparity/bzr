#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

readonly MAX_FILE_BYTES=1048576
readonly MAX_LINE_BYTES=4096

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

notes_file=$(mktemp)
trap 'rm -f "$notes_file"' EXIT

if ! awk -v version="$version" '
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
' <"$changelog_file" >"$notes_file"; then
  exit 1
fi

file_bytes=$(wc -c <"$notes_file")
if ((file_bytes > MAX_FILE_BYTES)); then
  echo "ERROR: candidate release notes must not exceed 1 MiB." >&2
  exit 1
fi

if ! awk -v maximum="$MAX_LINE_BYTES" 'length($0) > maximum { exit 1 }' <"$notes_file"; then
  echo "ERROR: candidate release-note lines must not exceed 4,096 bytes." >&2
  exit 1
fi

if ! awk '
  function strip_marker_indent(line) {
    sub(/^   /, "", line)
    sub(/^  /, "", line)
    sub(/^ /, "", line)
    return line
  }

  function is_tilde_fence(line) {
    line = strip_marker_indent(line)
    return line ~ /^~~~/
  }

  is_tilde_fence($0) || index($0, "<") != 0 || index($0, ">") != 0 || \
    index($0, "[") != 0 || index($0, "]") != 0 || index($0, "`") != 0 || \
    index($0, "&") != 0 { exit 1 }
' <"$notes_file"; then
  echo "ERROR: candidate release notes must use the bounded plain-text grammar." >&2
  exit 1
fi

cat "$notes_file"
