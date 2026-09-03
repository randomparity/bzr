#!/usr/bin/env bash
# Fixture bodies intentionally contain literal shell expansions for the checker to inspect;
# runtime globals are consumed by the dynamically sourced functional library.
# shellcheck disable=SC1091,SC2016,SC2034
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
  shift 2
  if ! bash "$CHECKER" "$root" "$@" >"$FIXTURES/$name.stdout" 2>"$FIXTURES/$name.stderr"; then
    printf 'expected semantic-ID checker to allow %s\n' "$name" >&2
    cat "$FIXTURES/$name.stderr" >&2
    return 1
  fi
}

expect_rejected() {
  local name=$1
  local root=$2
  local diagnostic=$3
  shift 3
  if bash "$CHECKER" "$root" "$@" >"$FIXTURES/$name.stdout" 2>"$FIXTURES/$name.stderr"; then
    printf 'expected semantic-ID checker to reject %s\n' "$name" >&2
    return 1
  fi
  if ! grep -Fq "$diagnostic" "$FIXTURES/$name.stderr"; then
    printf 'expected %s diagnostic to contain: %s\n' "$name" "$diagnostic" >&2
    cat "$FIXTURES/$name.stderr" >&2
    return 1
  fi
}

run_runtime_tests() (
  source "$SCRIPT_DIR/../tests/functional/lib.sh"
  GREEN='' RED='' YELLOW='' CYAN='' RESET=''
  TEST_ID_PREFIX=''

  runtime_output="$FIXTURES/runtime.stdout"
  runtime_error="$FIXTURES/runtime.stderr"

  fail_runtime() {
    printf 'runtime semantic-ID test failed: %s\n' "$*" >&2
    return 1
  }

  expect_runtime_rejected() {
    local name=$1
    local expected_error=$2
    local group=$3
    shift 3
    CURRENT_TEST='sentinel current test'
    SEEN_TEST_IDS=$'\n08-bugs/already-seen\n'
    CURRENT_TEST_GROUP=$group
    local before_current=$CURRENT_TEST
    local before_seen=$SEEN_TEST_IDS
    set +e
    test_begin "$@" >"$runtime_output" 2>"$runtime_error"
    local status=$?
    set -e
    [[ $status -eq 2 ]] || fail_runtime "$name returned $status instead of 2"
    grep -Fq "$expected_error" "$runtime_error" ||
      fail_runtime "$name stderr did not contain '$expected_error'"
    [[ $CURRENT_TEST == "$before_current" ]] ||
      fail_runtime "$name changed CURRENT_TEST"
    [[ $SEEN_TEST_IDS == "$before_seen" ]] ||
      fail_runtime "$name changed SEEN_TEST_IDS"
  }

  CURRENT_TEST_GROUP=08-bugs
  test_begin create-first-bug 'bug create (bug one)' >"$runtime_output"
  [[ $(<"$runtime_output") == '  TEST  [08-bugs/create-first-bug] bug create (bug one) ... ' ]] ||
    fail_runtime 'valid call output differs from the semantic reference contract'
  [[ $CURRENT_TEST == 'bug create (bug one)' ]] ||
    fail_runtime 'valid call did not set CURRENT_TEST to the description'
  test_begin create-second-bug 'bug create (bug two)' >"$runtime_output"
  [[ $SEEN_TEST_IDS == *$'\n08-bugs/create-first-bug\n'* ]] ||
    fail_runtime 'first valid ID was not recorded'
  [[ $SEEN_TEST_IDS == *$'\n08-bugs/create-second-bug\n'* ]] ||
    fail_runtime 'second valid ID was not recorded'

  TEST_ID_PREFIX=compare
  CURRENT_TEST_GROUP=08-bugs
  test_begin create-first-bug 'comparison bug create' >"$runtime_output"
  [[ $(<"$runtime_output") == '  TEST  [compare/08-bugs/create-first-bug] comparison bug create ... ' ]] ||
    fail_runtime 'comparison ID output differs from the semantic reference contract'
  [[ $SEEN_TEST_IDS == *$'\ncompare/08-bugs/create-first-bug\n'* ]] ||
    fail_runtime 'comparison ID was not recorded independently of the default tree'
  TEST_ID_PREFIX=''

  expect_runtime_rejected missing-group "invalid functional test group ''" '' \
    retry-after-missing-group 'retry after missing group'
  CURRENT_TEST_GROUP=08-bugs
  test_begin retry-after-missing-group 'retry after missing group' >"$runtime_output"

  expect_runtime_rejected one-argument 'expected exactly 2 arguments, got 1' 08-bugs \
    retry-after-one-argument
  CURRENT_TEST_GROUP=08-bugs
  test_begin retry-after-one-argument 'retry after one argument' >"$runtime_output"

  expect_runtime_rejected three-arguments 'expected exactly 2 arguments, got 3' 08-bugs \
    retry-after-three-arguments description extra
  CURRENT_TEST_GROUP=08-bugs
  test_begin retry-after-three-arguments 'retry after three arguments' >"$runtime_output"

  group_index=0
  for invalid_group in 08--bugs 08_Bugs bugs-08; do
    expect_runtime_rejected "invalid-group-$invalid_group" \
      "invalid functional test group '$invalid_group'" "$invalid_group" \
      retry-after-invalid-group 'retry after invalid group'
    CURRENT_TEST_GROUP=08-bugs
    test_begin retry-after-invalid-group-$group_index \
      'retry after invalid group' >"$runtime_output"
    group_index=$((group_index + 1))
  done

  slug_index=0
  for invalid_slug in Bad_Slug double--hyphen leading- trailing-; do
    expect_runtime_rejected "invalid-slug-$invalid_slug" \
      "invalid functional test slug '$invalid_slug'" 08-bugs \
      "$invalid_slug" 'invalid slug'
    CURRENT_TEST_GROUP=08-bugs
    test_begin corrected-invalid-slug-$slug_index \
      'retry after invalid slug' >"$runtime_output"
    slug_index=$((slug_index + 1))
  done

  CURRENT_TEST='already seen description'
  CURRENT_TEST_GROUP=08-bugs
  SEEN_TEST_IDS=$'\n08-bugs/already-seen\n'
  duplicate_before_current=$CURRENT_TEST
  duplicate_before_seen=$SEEN_TEST_IDS
  set +e
  test_begin already-seen 'duplicate description' >"$runtime_output" 2>"$runtime_error"
  duplicate_status=$?
  set -e
  [[ $duplicate_status -eq 2 ]] || fail_runtime 'duplicate ID did not return 2'
  grep -Fq "duplicate functional test ID '08-bugs/already-seen'" "$runtime_error" ||
    fail_runtime 'duplicate ID stderr omitted the full ID'
  [[ $CURRENT_TEST == "$duplicate_before_current" ]] ||
    fail_runtime 'duplicate ID changed CURRENT_TEST'
  [[ $SEEN_TEST_IDS == "$duplicate_before_seen" ]] ||
    fail_runtime 'duplicate ID changed SEEN_TEST_IDS'
  test_begin distinct-after-duplicate 'distinct after duplicate' >"$runtime_output"

  printf 'semantic functional-test runtime tests passed\n'
)

if [[ ${1:-} == --runtime-only ]]; then
  run_runtime_tests
  exit
fi

valid=$(new_fixture valid)
expect_allowed valid "$valid"

new_compare_fixture() {
  local name=$1
  local root="$FIXTURES/$name"
  mkdir -p "$root/tests/functional/compare"
  {
    printf 'SCRIPT_DIR=/fixture/tests/functional\n'
    printf 'TEST_ID_PREFIX=compare\n'
    printf 'for _phase in \\\n'
    printf '  01-config 08-bugs; do\n'
    printf '  CURRENT_TEST_GROUP="$_phase"\n'
    printf '  source "$SCRIPT_DIR/compare/${_phase}.sh"\n'
    printf 'done\n'
  } >"$root/tests/functional/run-compare.sh"
  printf 'test_begin "show-config" "comparison config show"\n' \
    >"$root/tests/functional/compare/01-config.sh"
  printf 'test_begin "create-bug" "comparison bug create"\n' \
    >"$root/tests/functional/compare/08-bugs.sh"
  printf '%s\n' "$root"
}

valid_compare=$(new_compare_fixture valid-compare)
expect_allowed valid-compare "$valid_compare" tests/functional/run-compare.sh tests/functional/compare

mismatched_compare_source=$(new_compare_fixture mismatched-compare-source)
sed -i.bak 's|/compare/|/phases/|' \
  "$mismatched_compare_source/tests/functional/run-compare.sh"
rm "$mismatched_compare_source/tests/functional/run-compare.sh.bak"
expect_rejected mismatched-compare-source "$mismatched_compare_source" \
  "canonical adjacent assignment/source pair" \
  tests/functional/run-compare.sh tests/functional/compare

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

run_runtime_tests
printf 'semantic functional-test ID checker fixtures passed\n'
