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

if ! awk -v maximum="$MAX_LINE_BYTES" 'length($0) > maximum { exit 1 }' "$notes_file"; then
	fail "release-note lines must not exceed 4,096 bytes"
fi

awk \
	-v no_vulnerabilities="$NO_VULNERABILITIES" \
	-v qualifying_vulnerabilities="$QUALIFYING_VULNERABILITIES" '
  function fail(requirement) {
    print "ERROR: release-note vulnerability assessment violates " requirement "." > "/dev/stderr"
    print "Update the release CHANGELOG.md section." > "/dev/stderr"
    invalid = 1
    exit 1
  }

  function trim(value) {
    sub(/^[[:space:]]+/, "", value)
    sub(/[[:space:]]+$/, "", value)
    return value
  }

  function complete_entry(  field_index) {
    if (!in_entry) {
      return
    }

    for (field_index = 1; field_index <= field_count; field_index++) {
      if (seen[field_index] != 1) {
        fail("each vulnerability entry must contain every required field exactly once")
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
      identifier = trim(substr($0, length(vulnerability_heading) + 1))
      if (identifier == "") {
        fail("each vulnerability heading must include a public identifier")
      }

      entry_count++
      in_entry = 1
      for (field = 1; field <= field_count; field++) {
        seen[field] = 0
      }
      next
    }

    if (assessment == "qualifying" && in_block && $0 ~ /^### /) {
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
            fail("vulnerability fields must have non-empty same-line values")
          }
          if (++seen[field] != 1) {
            fail("each vulnerability field must appear exactly once per entry")
          }
          if (field == 4 && index(value, "https://") != 1) {
            fail("each advisory value must begin with https://")
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
' "$notes_file"
