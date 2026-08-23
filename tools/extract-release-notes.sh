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
  function strip_fence_indent(line) {
    if (substr(line, 1, 3) == "   ") {
      line = substr(line, 4)
    } else if (substr(line, 1, 2) == "  ") {
      line = substr(line, 3)
    } else if (substr(line, 1, 1) == " ") {
      line = substr(line, 2)
    }
    return line
  }

  function opening_fence(line, fence_line, character, run_length) {
    fence_line = strip_fence_indent(line)
    character = substr(fence_line, 1, 1)
    if (character != "`" && character != "~") {
      return ""
    }
    run_length = 0
    while (substr(fence_line, run_length + 1, 1) == character) {
      run_length++
    }
    if (run_length < 3) {
      return ""
    }
    return substr(fence_line, 1, run_length)
  }

  function closes_fence(line, character, minimum_length, fence_line, run_length) {
    fence_line = strip_fence_indent(line)
    if (substr(fence_line, 1, 1) != character) {
      return 0
    }
    run_length = 0
    while (substr(fence_line, run_length + 1, 1) == character) {
      run_length++
    }
    return run_length >= minimum_length && substr(fence_line, run_length + 1) ~ /^ *$/
  }

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

  {
    if (fence_character != "") {
      if (capture) {
        print
      }
      if (closes_fence($0, fence_character, fence_length)) {
        fence_character = ""
        fence_length = 0
      }
      next
    }
    if (in_html_comment) {
      if (capture) {
        print
      }
      if (index($0, "-->") != 0) {
        in_html_comment = 0
      }
      next
    }

    current_fence = opening_fence($0)
    if (current_fence != "") {
      if (capture) {
        print
      }
      fence_character = substr(current_fence, 1, 1)
      fence_length = length(current_fence)
      next
    }

    if (index($0, "<!--") != 0) {
      if (capture) {
        print
      }
      if (index(substr($0, index($0, "<!--") + 4), "-->") == 0) {
        in_html_comment = 1
      }
      next
    }
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
