#!/usr/bin/env bash
# Fixture bodies intentionally contain literal shell expansions for the checker to inspect.
# shellcheck disable=SC2016
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CHECKER="$SCRIPT_DIR/check-functional-test-ids.sh"
FIXTURES=$(mktemp -d)
trap 'rm -rf "$FIXTURES"' EXIT

write_runner() {
  local root=$1
  shift
  mkdir -p "$root/tests/functional/phases"
  {
    printf 'SCRIPT_DIR=/fixture/tests/functional\n'
    printf 'for _phase in \\\n'
    printf '  %s; do\n' "$*"
    printf '  CURRENT_TEST_GROUP="$_phase"\n'
    printf '  source "$SCRIPT_DIR/phases/${_phase}.sh"\n'
    printf 'done\n'
  } >"$root/tests/functional/run-tests.sh"
}

new_fixture() {
  local name=$1
  local root="$FIXTURES/$name"
  write_runner "$root" 01-config 08-bugs
  printf 'test_begin "show-config" "config show"\n' \
    >"$root/tests/functional/phases/01-config.sh"
  printf '  test_begin "create-bug" "bug create"\n' \
    >"$root/tests/functional/phases/08-bugs.sh"
  printf '%s\n' "$root"
}

expect_allowed() {
  local name=$1
  local root=$2
  if ! bash "$CHECKER" "$root" >"$FIXTURES/$name.stdout" 2>"$FIXTURES/$name.stderr"; then
    printf 'expected semantic-ID checker to allow %s\n' "$name" >&2
    cat "$FIXTURES/$name.stderr" >&2
    return 1
  fi
}

expect_rejected() {
  local name=$1
  local root=$2
  local diagnostic=$3
  if bash "$CHECKER" "$root" >"$FIXTURES/$name.stdout" 2>"$FIXTURES/$name.stderr"; then
    printf 'expected semantic-ID checker to reject %s\n' "$name" >&2
    return 1
  fi
  if ! grep -Fq "$diagnostic" "$FIXTURES/$name.stderr"; then
    printf 'expected %s diagnostic to contain: %s\n' "$name" "$diagnostic" >&2
    cat "$FIXTURES/$name.stderr" >&2
    return 1
  fi
}

valid=$(new_fixture valid)
expect_allowed valid "$valid"

valid_branches=$(new_fixture valid-branches)
printf '%s\n' \
  'case "$mode" in' \
  'rest)' \
  '  test_begin "rest-view-bug" "rest view bug"' \
  '  ;;' \
  'xmlrpc)' \
  '  test_begin "xmlrpc-view-bug" "xmlrpc view bug"' \
  '  ;;' \
  'esac' >"$valid_branches/tests/functional/phases/08-bugs.sh"
expect_allowed valid-branches "$valid_branches"

invalid_phase=$(new_fixture invalid-phase)
mv "$invalid_phase/tests/functional/phases/08-bugs.sh" \
  "$invalid_phase/tests/functional/phases/08_Bugs.sh"
write_runner "$invalid_phase" 01-config 08_Bugs
expect_rejected invalid-phase "$invalid_phase" "invalid phase basename"

duplicate_phase=$(new_fixture duplicate-phase)
write_runner "$duplicate_phase" 01-config 08-bugs 08-bugs
expect_rejected duplicate-phase "$duplicate_phase" "duplicate runner phase"

missing_file=$(new_fixture missing-file)
rm "$missing_file/tests/functional/phases/08-bugs.sh"
expect_rejected missing-file "$missing_file" "runner/phase file mismatch"

unlisted_file=$(new_fixture unlisted-file)
printf 'test_begin "orphan" "orphan"\n' \
  >"$unlisted_file/tests/functional/phases/09-orphan.sh"
expect_rejected unlisted-file "$unlisted_file" "runner/phase file mismatch"

swapped_binding=$(new_fixture swapped-binding)
{
  printf 'SCRIPT_DIR=/fixture/tests/functional\n'
  printf 'for _phase in \\\n'
  printf '  01-config 08-bugs; do\n'
  printf '  source "$SCRIPT_DIR/phases/${_phase}.sh"\n'
  printf '  CURRENT_TEST_GROUP="$_phase"\n'
  printf 'done\n'
} >"$swapped_binding/tests/functional/run-tests.sh"
expect_rejected swapped-binding "$swapped_binding" "canonical adjacent assignment/source pair"

alternate_source=$(new_fixture alternate-source)
sed -i.bak 's/${_phase}/${phase}/' "$alternate_source/tests/functional/run-tests.sh"
rm "$alternate_source/tests/functional/run-tests.sh.bak"
expect_rejected alternate-source "$alternate_source" "canonical adjacent assignment/source pair"

intervening=$(new_fixture intervening)
sed -i.bak '/CURRENT_TEST_GROUP/a\
  : intervening' "$intervening/tests/functional/run-tests.sh"
rm "$intervening/tests/functional/run-tests.sh.bak"
expect_rejected intervening "$intervening" "canonical adjacent assignment/source pair"

direct_group=$(new_fixture direct-group)
printf '%s\n' 'printf "%s\n" "$CURRENT_TEST_GROUP"' \
  >>"$direct_group/tests/functional/phases/01-config.sh"
expect_rejected direct-group "$direct_group" "must not reference CURRENT_TEST_GROUP"

derived_group=$(new_fixture derived-group)
printf 'group=${CURRENT_TEST_GROUP}-suffix\n' \
  >>"$derived_group/tests/functional/phases/01-config.sh"
expect_rejected derived-group "$derived_group" "must not reference CURRENT_TEST_GROUP"

invalid_calls=(
  'test_begin "123. legacy" "legacy numeric"'
  'test_begin "Bad_Slug" "bad slug"'
  'test_begin "$slug" "expanded slug"'
  'test_begin "missing-description"'
  'test_begin "too" "many" "arguments"'
  'false || test_begin "hidden" "hidden call"'
  'if false; then test_begin "hidden" "hidden call"; fi'
)

for index in "${!invalid_calls[@]}"; do
  root=$(new_fixture "invalid-call-$index")
  printf '%s\n' "${invalid_calls[$index]}" \
    >"$root/tests/functional/phases/08-bugs.sh"
  expect_rejected "invalid-call-$index" "$root" "noncanonical test_begin"
done

indented_invalid=$(new_fixture indented-invalid)
printf '    test_begin "bad_slug" "bad slug"\n' \
  >"$indented_invalid/tests/functional/phases/08-bugs.sh"
expect_rejected indented-invalid "$indented_invalid" "noncanonical test_begin"

duplicate_id=$(new_fixture duplicate-id)
printf '%s\n' \
  'test_begin "create-bug" "first description"' \
  'test_begin "create-bug" "second description"' \
  >"$duplicate_id/tests/functional/phases/08-bugs.sh"
expect_rejected duplicate-id "$duplicate_id" "duplicate functional test ID: 08-bugs/create-bug"

printf 'semantic functional-test ID checker fixtures passed\n'
