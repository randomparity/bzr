#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

if [[ $# -ne 1 && $# -ne 3 ]]; then
  printf 'usage: %s <repo-root> [runner-relative-path phase-dir-relative-path]\n' "$0" >&2
  exit 1
fi

repo_root=$1
runner_relative_path=tests/functional/run-tests.sh
phase_dir_relative_path=tests/functional/phases
test_id_prefix=''
if [[ $# -eq 3 ]]; then
  runner_relative_path=$2
  phase_dir_relative_path=$3
  phase_dir_basename=$(basename "$phase_dir_relative_path")
  if [[ $phase_dir_basename != phases ]]; then
    test_id_prefix=$phase_dir_basename
  fi
fi

if [[ $runner_relative_path == /* || $phase_dir_relative_path == /* ||
  $runner_relative_path == .. || $runner_relative_path == ../* ||
  $phase_dir_relative_path == .. || $phase_dir_relative_path == ../* ]]; then
  printf 'ERROR: runner and phase paths must be relative to the repository root\n' >&2
  exit 1
fi

runner="$repo_root/$runner_relative_path"
phases_dir="$repo_root/$phase_dir_relative_path"
phase_dir_basename=$(basename "$phase_dir_relative_path")
phase_re='^[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*$'
call_re='^[[:space:]]*test_begin[[:space:]]+"([a-z0-9]+(-[a-z0-9]+)*)"[[:space:]]+"[^"]*"[[:space:]]*$'
errors=0

error() {
  printf 'ERROR: %s\n' "$*" >&2
  errors=$((errors + 1))
}

if ! command -v rg >/dev/null 2>&1; then
  printf 'ERROR: ripgrep (rg) is required for the functional test ID guard\n' >&2
  exit 1
fi

if [[ ! -f $runner ]]; then
  printf 'ERROR: functional runner not found: %s\n' "$runner" >&2
  exit 1
fi

if [[ ! -d $phases_dir ]]; then
  printf 'ERROR: functional phase directory not found: %s\n' "$phases_dir" >&2
  exit 1
fi

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
runner_phases="$temporary/runner-phases"
disk_phases="$temporary/disk-phases"
runner_sorted="$temporary/runner-sorted"
disk_sorted="$temporary/disk-sorted"
occurrences="$temporary/test-begin-occurrences"
runner_state_references="$temporary/runner-state-references"

phase_files=()
for phase_file in "$phases_dir"/*.sh; do
  if [[ ! -e $phase_file ]]; then
    error "no functional phase files found in $phases_dir"
    break
  fi
  phase_files+=("$phase_file")
  phase=$(basename "$phase_file" .sh)
  printf '%s\n' "$phase" >>"$disk_phases"
  if [[ ! $phase =~ $phase_re ]]; then
    error "invalid phase basename '$phase'; expected $phase_re"
  fi
done

if ! awk '
  /^for _phase in \\$/ {
    starts++
    inside = 1
    next
  }
  inside {
    line = $0
    if (line ~ /; do[[:space:]]*$/) {
      sub(/; do[[:space:]]*$/, "", line)
      ends++
      inside = 0
    }
    gsub(/\\/, "", line)
    count = split(line, fields, /[[:space:]]+/)
    for (field_index = 1; field_index <= count; field_index++) {
      if (fields[field_index] != "") print fields[field_index]
    }
  }
  END {
    if (starts != 1 || ends != 1 || inside) exit 2
  }
' "$runner" >"$runner_phases"; then
  error "runner must contain exactly one canonical 'for _phase in \\' list ending in '; do'"
fi

duplicate_runner_phases=$(sort "$runner_phases" | uniq -d)
if [[ -n $duplicate_runner_phases ]]; then
  while IFS= read -r phase; do
    [[ -n $phase ]] && error "duplicate runner phase: $phase"
  done <<<"$duplicate_runner_phases"
fi

sort -u "$runner_phases" >"$runner_sorted"
sort -u "$disk_phases" >"$disk_sorted"
if ! cmp -s "$runner_sorted" "$disk_sorted"; then
  error "runner/phase file mismatch; the runner list must equal tests/functional/phases/*.sh"
  while IFS= read -r phase; do
    [[ -n $phase ]] && printf '  only in runner: %s\n' "$phase" >&2
  done < <(comm -23 "$runner_sorted" "$disk_sorted")
  while IFS= read -r phase; do
    [[ -n $phase ]] && printf '  only on disk: %s\n' "$phase" >&2
  done < <(comm -13 "$runner_sorted" "$disk_sorted")
fi

if ! awk -v expected_source="source \"\$SCRIPT_DIR/$phase_dir_basename/\${_phase}.sh\"" \
  -v expected_prefix="$test_id_prefix" '
  !loop_seen && /^TEST_ID_PREFIX=/ {
    prefix_assignments++
    if ($0 == "TEST_ID_PREFIX=" expected_prefix) matching_prefixes++
  }
  /^for _phase in \\$/ {
    loop_seen = 1
    inside = 1
  }
  inside && /^[[:space:]]*CURRENT_TEST_GROUP="\$_phase"[[:space:]]*$/ {
    assignments++
    previous_assignment = 1
    next
  }
  inside {
    line = $0
    sub(/^[[:space:]]+/, "", line)
    sub(/[[:space:]]+$/, "", line)
    if (line == expected_source) {
      sources++
      if (previous_assignment) pairs++
      previous_assignment = 0
      next
    }
  }
  inside && /^done[[:space:]]*$/ { inside = 0 }
  { previous_assignment = 0 }
  END {
    prefix_error = (prefix_assignments != 1 || matching_prefixes != 1)
    if (assignments != 1 || sources != 1 || pairs != 1 || prefix_error) exit 1
  }
' "$runner"; then
  error 'runner must contain exactly one canonical adjacent assignment/source pair'
fi

for runner_variable in CURRENT_TEST_GROUP TEST_ID_PREFIX; do
  set +e
  rg -n --with-filename --no-heading -F "$runner_variable" \
    "${phase_files[@]}" >"$runner_state_references"
  reference_status=$?
  set -e
  case $reference_status in
  0)
    while IFS=: read -r file line content; do
      error "$file:$line must not reference $runner_variable: $content"
    done <"$runner_state_references"
    ;;
  1) ;;
  *)
    error "rg failed while checking phase ownership of $runner_variable (exit $reference_status)"
    ;;
  esac
done

set +e
rg -n --with-filename --no-heading -F 'test_begin' "${phase_files[@]}" >"$occurrences"
occurrence_status=$?
set -e
case $occurrence_status in
0) ;;
1) : >"$occurrences" ;;
*)
  error "rg failed while inventorying test_begin occurrences (exit $occurrence_status)"
  ;;
esac

seen_ids=$'\n'
while IFS=: read -r file line content; do
  [[ -n $file ]] || continue
  if [[ ! $content =~ $call_re ]]; then
    error "$file:$line noncanonical test_begin; expected: test_begin \"<literal-slug>\" \"<description>\""
    continue
  fi
  slug=${BASH_REMATCH[1]}
  phase=$(basename "$file" .sh)
  full_id="${test_id_prefix:+$test_id_prefix/}$phase/$slug"
  case $seen_ids in
  *$'\n'"$full_id"$'\n'*)
    error "duplicate functional test ID: $full_id ($file:$line)"
    ;;
  *)
    seen_ids="${seen_ids}${full_id}"$'\n'
    ;;
  esac
done <"$occurrences"

if [[ $errors -ne 0 ]]; then
  exit 1
fi

printf 'functional test semantic IDs are valid\n'
