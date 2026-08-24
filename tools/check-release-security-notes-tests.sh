#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
VALIDATOR="$SCRIPT_DIR/check-release-security-notes.sh"
FIXTURES=$(mktemp -d)
SIDE_EFFECT="$FIXTURES/side-effect"
trap 'rm -r "$FIXTURES"' EXIT

NO_VULNERABILITIES='Security assessment: No publicly identified runtime vulnerabilities in bzr were fixed in this release.'
QUALIFYING_VULNERABILITIES='Security assessment: Publicly identified runtime vulnerabilities in bzr were fixed in this release.'
ZERO_WIDTH_SPACE=$(printf '\342\200\213')

write_fixture() {
  local name=$1
  {
    printf '%s\n\n' '### Security'
    cat
  } >"$FIXTURES/$name"
  printf '%s\n' "$FIXTURES/$name"
}

expect_allowed() {
  local name=$1
  local fixture=$2
  if ! bash "$VALIDATOR" "$fixture" >/dev/null 2>&1; then
    echo "expected release-security validator to allow $name" >&2
    return 1
  fi
}

expect_rejected() {
  local name=$1
  local fixture=$2
  if bash "$VALIDATOR" "$fixture" >/dev/null 2>&1; then
    echo "expected release-security validator to reject $name" >&2
    return 1
  fi
}

expect_rejected_with_stderr() {
  local name=$1
  local fixture=$2
  local expected=$3
  local stderr_file="$FIXTURES/$name.stderr"
  if bash "$VALIDATOR" "$fixture" >/dev/null 2>"$stderr_file"; then
    echo "expected release-security validator to reject $name" >&2
    return 1
  fi
  if ! grep -Fqx "$expected" "$stderr_file"; then
    echo "expected release-security validator stderr for $name to contain: $expected" >&2
    return 1
  fi
}

expect_allowed_from_directory() {
  local name=$1
  local directory=$2
  local fixture_name=$3
  if ! (cd "$directory" && bash "$VALIDATOR" "$fixture_name") >/dev/null 2>&1; then
    echo "expected release-security validator to allow $name" >&2
    return 1
  fi
}

expect_indented_level_three_heading_rejected() {
  local name=$1
  local indentation=$2
  local fixture="$FIXTURES/$name"
  {
    printf '%s\n' "$QUALIFYING_VULNERABILITIES"
    printf '%s\n' "#### Vulnerability: GHSA-$name"
    printf '%s\n' '- Affected bzr versions: before 1.2.3'
    printf '%s\n' '- First fixed version: 1.2.3'
    printf '%s### Dependency security\n' "$indentation"
    printf '%s\n' '- Runtime impact: A remote server could cause a denial of service.'
    printf '%s\n' "- Advisory: https://example.com/GHSA-$name"
    printf '%s\n' '- Upgrade guidance: Upgrade to bzr 1.2.3 or later.'
  } >"$fixture"
  expect_rejected "$name ends an entry" "$fixture"
}

expect_indented_vulnerability_heading() {
  local name=$1
  local indentation=$2
  local assessment=$3
  local expectation=$4
  local fixture="$FIXTURES/$name"
  {
    printf '%s\n' "$assessment"
    printf '%s%s\n' "$indentation" "#### Vulnerability: GHSA-$name"
    printf '%s\n' '- Affected bzr versions: before 1.2.3'
    printf '%s\n' '- First fixed version: 1.2.3'
    printf '%s\n' '- Runtime impact: A remote server could cause a denial of service.'
    printf '%s\n' "- Advisory: https://example.com/GHSA-$name"
    printf '%s\n' '- Upgrade guidance: Upgrade to bzr 1.2.3 or later.'
  } >"$fixture"
  if [[ $expectation == allowed ]]; then
    expect_allowed "$name" "$fixture"
  else
    expect_rejected "$name" "$fixture"
  fi
}

no_qualifying=$(
  write_fixture no-qualifying <<EOF
$NO_VULNERABILITIES
EOF
)
expect_allowed "exact no-qualifying assessment" "$no_qualifying"

dependency_note=$(
  write_fixture dependency-note <<EOF
$NO_VULNERABILITIES

### Dependency security

- Updated a dependency to address CVE-2026-12345.
EOF
)
expect_allowed "dependency CVE after no-qualifying assessment" "$dependency_note"

unidentified_issue=$(
  write_fixture unidentified-issue <<EOF
$NO_VULNERABILITIES

- Fixed a runtime issue that has no public identifier.
EOF
)
expect_allowed "fixed issue without public identifier" "$unidentified_issue"

complete_entry=$(
  write_fixture complete-entry <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-1234-5678-9abc
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://github.com/randomparity/bzr/security/advisories/GHSA-1234-5678-9abc
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_allowed "complete qualifying entry" "$complete_entry"

image_wrapped_assessment=$(
  write_fixture image-wrapped-assessment <<EOF
![
$NO_VULNERABILITIES
](https://example.com/image.png)
EOF
)
expect_rejected "assessment in a multiline image label" "$image_wrapped_assessment"

html_wrapped_assessment=$(
  write_fixture html-wrapped-assessment <<EOF
<details>
$NO_VULNERABILITIES
</details>
EOF
)
expect_rejected "assessment in a raw HTML container" "$html_wrapped_assessment"

late_assessment=$(
  write_fixture late-assessment <<EOF
Release overview.

$NO_VULNERABILITIES
EOF
)
expect_rejected "assessment outside the first Security paragraph" "$late_assessment"

render_empty_identifier=$(
  write_fixture render-empty-identifier <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: []()
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-render-empty
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "render-empty vulnerability identifier" "$render_empty_identifier"

for public_identifier in CVE-2026-12345 GHSA-1234-5678-9abc RUSTSEC-2026-0001; do
  visible_identifier=$(
    write_fixture "visible-$public_identifier" <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: $public_identifier
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/$public_identifier
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
  )
  expect_allowed "visible $public_identifier token" "$visible_identifier"
done

empty_rendered_identifier=$(
  write_fixture empty-rendered-identifier <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: ###
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-empty-rendered-identifier
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "ATX closing hashes with no rendered identifier" "$empty_rendered_identifier"

identifier_with_closing_hashes=$(
  write_fixture identifier-with-closing-hashes <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-visible-identifier ###
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-visible-identifier
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "identifier with ATX closing hashes" "$identifier_with_closing_hashes"

expect_indented_vulnerability_heading \
  "no-outcome-one-space-vulnerability-heading" \
  ' ' \
  "$NO_VULNERABILITIES" \
  rejected
expect_indented_vulnerability_heading \
  "no-outcome-two-space-vulnerability-heading" \
  '  ' \
  "$NO_VULNERABILITIES" \
  rejected
expect_indented_vulnerability_heading \
  "no-outcome-three-space-vulnerability-heading" \
  '   ' \
  "$NO_VULNERABILITIES" \
  rejected
expect_indented_vulnerability_heading \
  "qualifying-one-space-vulnerability-heading" \
  ' ' \
  "$QUALIFYING_VULNERABILITIES" \
  rejected
expect_indented_vulnerability_heading \
  "qualifying-two-space-vulnerability-heading" \
  '  ' \
  "$QUALIFYING_VULNERABILITIES" \
  rejected
expect_indented_vulnerability_heading \
  "qualifying-three-space-vulnerability-heading" \
  '   ' \
  "$QUALIFYING_VULNERABILITIES" \
  rejected

bare_level_three_heading=$(
  write_fixture bare-level-three-heading <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-bare-heading
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
###
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-bare-heading
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "bare level-three heading ends an entry" "$bare_level_three_heading"

tabbed_level_three_heading="$FIXTURES/tabbed-level-three-heading"
{
  printf '%s\n' "$QUALIFYING_VULNERABILITIES"
  printf '%s\n' '#### Vulnerability: GHSA-tabbed-heading'
  printf '%s\n' '- Affected bzr versions: before 1.2.3'
  printf '%s\n' '- First fixed version: 1.2.3'
  printf '###\tDependency security\n'
  printf '%s\n' '- Runtime impact: A remote server could cause a denial of service.'
  printf '%s\n' '- Advisory: https://example.com/GHSA-tabbed-heading'
  printf '%s\n' '- Upgrade guidance: Upgrade to bzr 1.2.3 or later.'
} >"$tabbed_level_three_heading"
expect_rejected "tabbed level-three heading ends an entry" "$tabbed_level_three_heading"

expect_indented_level_three_heading_rejected one-space-level-three-heading ' '
expect_indented_level_three_heading_rejected two-space-level-three-heading '  '
expect_indented_level_three_heading_rejected three-space-level-three-heading '   '

four_space_code_block=$(
  write_fixture four-space-code-block <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-four-space-code-block
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
    ### Dependency security
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-four-space-code-block
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_allowed "four-space code block does not end an entry" "$four_space_code_block"

fenced_fields=$(
  write_fixture fenced-fields <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-fenced-fields
\`\`\`text
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: These fields are hidden in a fenced code block.
- Advisory: https://example.com/GHSA-fenced-fields
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
\`\`\`
EOF
)
expect_rejected "required fields hidden in a fenced code block" "$fenced_fields"

shorter_fence_pseudo_closer=$(
  write_fixture shorter-fence-pseudo-closer <<EOF
\`\`\`\`text
\`\`\`
$NO_VULNERABILITIES
\`\`\`\`
EOF
)
expect_rejected \
  "assessment hidden after a shorter fence pseudo-closer" \
  "$shorter_fence_pseudo_closer"

trailing_text_pseudo_closer=$(
  write_fixture trailing-text-pseudo-closer <<EOF
\`\`\`text
\`\`\`not-a-closer
$NO_VULNERABILITIES
\`\`\`
EOF
)
expect_rejected \
  "assessment hidden after a trailing-text fence pseudo-closer" \
  "$trailing_text_pseudo_closer"

longer_fence_closer="$FIXTURES/longer-fence-closer"
{
  printf '%s\n' '````text'
  printf '%s\n' 'This fenced text is ignored.'
  printf '%s   \n' '`````'
  printf '%s\n' "$NO_VULNERABILITIES"
} >"$longer_fence_closer"
expect_rejected \
  "fenced code with a longer closer" \
  "$longer_fence_closer"

commented_fields=$(
  write_fixture commented-fields <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-commented-fields
<!--
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: These fields are hidden in an HTML comment.
- Advisory: https://example.com/GHSA-commented-fields
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
-->
EOF
)
expect_rejected "required fields hidden in an HTML comment" "$commented_fields"

# The bounded plain-text grammar is enforced by the validator on the full
# generated notes; each fixture below is a complete notes file.
# shellcheck disable=SC2016  # Backticks are fixture content, not command substitution.
forbidden_note_lines=(
  'raw < opener'
  'raw > closer'
  'raw [ square opener'
  'raw ] square closer'
  'raw ` code delimiter'
  'raw & character-reference opener'
  'raw \ escape delimiter'
  '- Use `bzr bug view` to inspect a bug.'
  '- Use <!-- when describing an HTML comment opener.'
  '- A literal --> token.'
)
for index in "${!forbidden_note_lines[@]}"; do
  offending=${forbidden_note_lines[$index]}
  fixture=$(
    write_fixture "forbidden-note-$index" <<EOF
$NO_VULNERABILITIES

$offending
EOF
  )
  expect_rejected "notes containing $offending" "$fixture"
done

leading_hyphen_directory="$FIXTURES/leading-hyphen"
mkdir "$leading_hyphen_directory"
printf '%s\n\n%s\n' '### Security' "$NO_VULNERABILITIES" \
  >"$leading_hyphen_directory/-release-notes"
expect_allowed_from_directory "leading-hyphen filename" "$leading_hyphen_directory" '-release-notes'

missing_assessment=$(
  write_fixture missing-assessment <<'EOF'
### Dependency security

- Updated a dependency to address CVE-2026-12345.
EOF
)
expect_rejected "missing assessment" "$missing_assessment"

both_outcomes=$(
  write_fixture both-outcomes <<EOF
$NO_VULNERABILITIES
$QUALIFYING_VULNERABILITIES
EOF
)
expect_rejected "both outcomes" "$both_outcomes"

empty_field=$(
  write_fixture empty-field <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-empty-field
- Affected bzr versions: before 1.2.3
- First fixed version:
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://github.com/randomparity/bzr/security/advisories/GHSA-empty-field
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "empty field" "$empty_field"

render_empty_entity_fields=$(
  write_fixture render-empty-entity-fields <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-render-empty-fields
- Affected bzr versions: &#32;
- First fixed version: &Tab;
- Runtime impact: &#x20;
- Advisory: https://github.com/randomparity/bzr/security/advisories/GHSA-render-empty-fields
- Upgrade guidance: &NewLine;
EOF
)
expect_rejected "character references that render required fields empty" "$render_empty_entity_fields"

zero_width_fields=$(
  write_fixture zero-width-fields <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-zero-width-fields
- Affected bzr versions: $ZERO_WIDTH_SPACE
- First fixed version: $ZERO_WIDTH_SPACE
- Runtime impact: $ZERO_WIDTH_SPACE
- Advisory: https://github.com/randomparity/bzr/security/advisories/GHSA-zero-width-fields
- Upgrade guidance: $ZERO_WIDTH_SPACE
EOF
)
expect_rejected "zero-width-only required fields" "$zero_width_fields"

backslash_only_fields=$(
  write_fixture backslash-only-fields <<'EOF'
Security assessment: Publicly identified runtime vulnerabilities in bzr were fixed in this release.

#### Vulnerability: GHSA-backslash-only-fields
- Affected bzr versions: \
- First fixed version: \
- Runtime impact: \
- Advisory: https://github.com/randomparity/bzr/security/advisories/GHSA-backslash-only-fields
- Upgrade guidance: \
EOF
)
expect_rejected "backslash-only required fields" "$backslash_only_fields"

duplicate_field=$(
  write_fixture duplicate-field <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-duplicate-field
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://github.com/randomparity/bzr/security/advisories/GHSA-duplicate-field
- Advisory: https://example.com/duplicate
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "duplicate field" "$duplicate_field"

non_https_advisory=$(
  write_fixture non-https-advisory <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-http-advisory
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: http://example.com/advisory
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
EOF
)
expect_rejected "non-HTTPS advisory" "$non_https_advisory"

dependency_only=$(
  write_fixture dependency-only <<'EOF'
### Dependency security

#### Vulnerability: CVE-2026-12345
- Affected bzr versions: dependency only
- First fixed version: dependency update
- Runtime impact: Dependency-only issue.
- Advisory: https://example.com/CVE-2026-12345
- Upgrade guidance: Update the dependency.
EOF
)
expect_rejected "dependency-only CVE without project assessment" "$dependency_only"

incomplete_second_entry=$(
  write_fixture incomplete-second-entry <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-complete-entry
- Affected bzr versions: before 1.2.3
- First fixed version: 1.2.3
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-complete-entry
- Upgrade guidance: Upgrade to bzr 1.2.3 or later.
#### Vulnerability: GHSA-incomplete-entry
- Affected bzr versions: before 1.2.4
- First fixed version: 1.2.4
- Runtime impact: A remote server could cause a denial of service.
- Advisory: https://example.com/GHSA-incomplete-entry
EOF
)
expect_rejected_with_stderr \
  "incomplete-second-entry" \
  "$incomplete_second_entry" \
  "ERROR: release-note vulnerability assessment entry 2 violates each vulnerability entry must contain every required field exactly once."

over_1_mib="$FIXTURES/over-1-mib"
dd if=/dev/zero of="$over_1_mib" bs=1048577 count=1 2>/dev/null
expect_rejected "file over 1 MiB" "$over_1_mib"

over_4096_byte_line="$FIXTURES/over-4096-byte-line"
printf '%*s\n' 4097 '' >"$over_4096_byte_line"
expect_rejected "line over 4,096 bytes" "$over_4096_byte_line"

literal_metacharacters=$(
  write_fixture literal-metacharacters <<EOF
$QUALIFYING_VULNERABILITIES

#### Vulnerability: GHSA-literal-data
- Affected bzr versions: \$(touch "$SIDE_EFFECT")
- First fixed version: 1.2.3; echo not-executed
- Runtime impact: \$HOME and | ; wildcards * ? remain data
- Advisory: https://example.com/advisory?note=\$(touch "$SIDE_EFFECT")
- Upgrade guidance: Keep literal \$(touch "$SIDE_EFFECT") text as data.
EOF
)
expect_allowed "literal shell metacharacters" "$literal_metacharacters"
if [[ -e $SIDE_EFFECT ]]; then
  echo "validator executed a release-note field value" >&2
  exit 1
fi

if bash "$VALIDATOR" >/dev/null 2>&1; then
  echo "expected release-security validator to reject missing arguments" >&2
  exit 1
fi

if bash "$VALIDATOR" "$no_qualifying" "$complete_entry" >/dev/null 2>&1; then
  echo "expected release-security validator to reject extra arguments" >&2
  exit 1
fi

if bash "$VALIDATOR" "$FIXTURES" >/dev/null 2>&1; then
  echo "expected release-security validator to reject a directory" >&2
  exit 1
fi

phony_probe="$FIXTURES/phony-probe"
mkdir "$phony_probe"
touch "$phony_probe/check-release-security-notes"
phony_output=$(
  make --no-print-directory \
    -C "$phony_probe" \
    -f "$SCRIPT_DIR/../Makefile" \
    -n check-release-security-notes
)
if [[ $phony_output != *'bash tools/check-release-security-notes-tests.sh'* ]]; then
  echo "expected check-release-security-notes Make target to remain runnable when a same-name file exists" >&2
  exit 1
fi
