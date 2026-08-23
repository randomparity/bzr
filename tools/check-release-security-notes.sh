#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

readonly MAX_FILE_BYTES=1048576
readonly MAX_LINE_BYTES=4096
readonly NO_VULNERABILITIES='Security assessment: No publicly identified runtime vulnerabilities in bzr were fixed in this release.'
readonly QUALIFYING_VULNERABILITIES='Security assessment: Publicly identified runtime vulnerabilities in bzr were fixed in this release.'

fail() {
	local requirement=$1
	echo "ERROR: release-note vulnerability assessment violates $requirement." >&2
	echo "Update the release's CHANGELOG.md section." >&2
	exit 1
}

if (($# != 1)); then
	fail "the validator requires exactly one release-notes file"
fi

notes_file=$1
if [[ ! -f $notes_file || ! -r $notes_file ]]; then
	fail "the release-notes input must be a readable regular file"
fi

file_bytes=$(wc -c <"$notes_file")
if ((file_bytes > MAX_FILE_BYTES)); then
	fail "the release-notes file must not exceed 1 MiB"
fi

if ! awk -v maximum="$MAX_LINE_BYTES" 'length($0) > maximum { exit 1 }' <"$notes_file"; then
	fail "release-note lines must not exceed 4,096 bytes"
fi

awk \
	-v no_vulnerabilities="$NO_VULNERABILITIES" \
	-v qualifying_vulnerabilities="$QUALIFYING_VULNERABILITIES" '
  function fail(requirement, entry_number, message) {
    message = "ERROR: release-note vulnerability assessment"
    if (entry_number != "") {
      message = message " entry " entry_number
    }
    print message " violates " requirement "." > "/dev/stderr"
    print "Update the release CHANGELOG.md section." > "/dev/stderr"
    invalid = 1
    exit 1
  }

  function trim(value) {
    sub(/^[[:space:]]+/, "", value)
    sub(/[[:space:]]+$/, "", value)
    return value
  }

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

  function complete_entry(  field_index) {
    if (!in_entry) {
      return
    }

    for (field_index = 1; field_index <= field_count; field_index++) {
      if (seen[field_index] != 1) {
        fail("each vulnerability entry must contain every required field exactly once", entry_count)
      }
    }

    in_entry = 0
  }

  BEGIN {
    field_count = 5
    fields[1] = "- Affected bzr versions:"
    fields[2] = "- First fixed version:"
    fields[3] = "- Runtime impact:"
    fields[4] = "- Advisory:"
    fields[5] = "- Upgrade guidance:"
    vulnerability_heading = "#### Vulnerability: "
  }

  {
    if (fence_character != "") {
      if (closes_fence($0, fence_character, fence_length)) {
        fence_character = ""
        fence_length = 0
      }
      next
    }
    current_fence = opening_fence($0)
    if (current_fence != "") {
      if (assessment == "qualifying" && in_block && in_entry) {
        fail("vulnerability entries must not contain fenced code", entry_count)
      }
      fence_character = substr(current_fence, 1, 1)
      fence_length = length(current_fence)
      next
    }

    if (in_html_comment) {
      if (index($0, "-->") != 0) {
        in_html_comment = 0
      }
      next
    }
    if (index($0, "<!--") != 0) {
      if (assessment == "qualifying" && in_block && in_entry) {
        fail("vulnerability entries must not contain HTML comments", entry_count)
      }
      if (index(substr($0, index($0, "<!--") + 4), "-->") == 0) {
        in_html_comment = 1
      }
      next
    }
  }

  $0 == no_vulnerabilities {
    assessment_count++
    assessment = "no"
    in_block = 0
    next
  }

  $0 == qualifying_vulnerabilities {
    assessment_count++
    assessment = "qualifying"
    in_block = 1
    next
  }

  {
    if (index($0, vulnerability_heading) == 1) {
      vulnerability_headings++

      if (assessment != "qualifying" || !in_block) {
        outside_heading = 1
        next
      }

      complete_entry()
      entry_count++
      in_entry = 1
      for (field = 1; field <= field_count; field++) {
        seen[field] = 0
      }

      identifier = trim(substr($0, length(vulnerability_heading) + 1))
      if (identifier == "") {
        fail("each vulnerability heading must include a public identifier", entry_count)
      }
      next
    }

    if (assessment == "qualifying" && in_block && $0 ~ /^(   |  | )?###([[:space:]]|$)/) {
      complete_entry()
      in_block = 0
      next
    }

    if (assessment == "qualifying" && in_block && !in_entry && $0 !~ /^[[:space:]]*$/) {
      fail("the qualifying assessment must be followed by vulnerability entries")
    }

    if (assessment == "qualifying" && in_block && in_entry) {
      for (field = 1; field <= field_count; field++) {
        if (index($0, fields[field]) == 1) {
          value = trim(substr($0, length(fields[field]) + 1))
          if (value == "") {
            fail("vulnerability fields must have non-empty same-line values", entry_count)
          }
          if (++seen[field] != 1) {
            fail("each vulnerability field must appear exactly once per entry", entry_count)
          }
          if (field == 4 && index(value, "https://") != 1) {
            fail("each advisory value must begin with https://", entry_count)
          }
          next
        }
      }
    }
  }

  END {
    if (invalid) {
      exit 1
    }
    if (assessment_count != 1) {
      fail("exactly one whole-line security assessment marker is required")
    }
    if (assessment == "no") {
      if (vulnerability_headings != 0) {
        fail("the no-vulnerability assessment must not include vulnerability entries")
      }
      exit 0
    }
    if (outside_heading || vulnerability_headings != entry_count) {
      fail("vulnerability entries must follow the qualifying assessment before the next level-three heading")
    }
    complete_entry()
    if (entry_count == 0) {
      fail("the qualifying assessment requires at least one complete vulnerability entry")
    }
  }
' <"$notes_file"
