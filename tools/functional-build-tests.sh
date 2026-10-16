#!/usr/bin/env bash
# Exercise phase 00 without real compilation, containers, or network access.
# The phase consumes globals and functions from its sourcing runner.
# shellcheck disable=SC1090,SC2034,SC2329
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PHASE="$SCRIPT_DIR/../tests/functional/phases/00-build.sh"
FIXTURES=$(mktemp -d)
trap 'rm -rf "$FIXTURES"' EXIT
FIXTURES=$(cd "$FIXTURES" && pwd -P)

run_case() (
  local name=$1 override=$2 expected_status=$3
  REPO_ROOT="$FIXTURES/$name checkout"
  mkdir -p "$REPO_ROOT/target/release" "$REPO_ROOT/target/debug"
  printf '#!/bin/sh\nexit 0\n' >"$REPO_ROOT/target/release/bzr"
  cp "$REPO_ROOT/target/release/bzr" "$REPO_ROOT/target/debug/bzr"
  chmod +x "$REPO_ROOT/target/"{release,debug}/bzr
  ln -s target/release "$REPO_ROOT/release-link"
  ln -s target/release/bzr "$REPO_ROOT/linked-bzr"
  BZ_VERSION=fixture BZ_URL=http://fixture.invalid
  local expected_binary=$override expected_trace=network
  if [[ -z $override || $override == missing ]]; then
    expected_binary="$REPO_ROOT/target/release/bzr"
    expected_trace=$'build\nnetwork'
    rm "$REPO_ROOT/target/release/bzr"
  fi
  local selected="$REPO_ROOT/selected" trace="$REPO_ROOT/trace"
  : >"$trace"
  cargo() {
    printf 'build\n' >>"$trace"
    printf '#!/bin/sh\nexit 0\n' >"$REPO_ROOT/target/release/bzr"
    chmod +x "$REPO_ROOT/target/release/bzr"
  }
  curl() { printf 'network\n' >>"$trace"; }

  local result
  if (
    cd "$REPO_ROOT"
    if [[ -z $override ]]; then unset BZR_BIN; else BZR_BIN=$override; fi
    source "$PHASE"
    printf '%s' "$BZR_BIN" >"$selected"
  ) >"$REPO_ROOT/stdout" 2>"$REPO_ROOT/stderr"; then
    result=0
  else
    result=$?
  fi
  if [[ $result -ne $expected_status ]]; then
    printf '%s: expected exit %s, got %s\n' "$name" "$expected_status" "$result" >&2
    cat "$REPO_ROOT/stdout" "$REPO_ROOT/stderr" >&2
    return 1
  fi
  if [[ $expected_status -eq 0 ]]; then
    [[ $(cat "$selected") == "$expected_binary" ]]
    [[ $(cat "$trace") == "$expected_trace" ]]
  else
    [[ ! -s $trace && ! -e $selected ]]
    grep -Fq "$override" "$REPO_ROOT/stdout"
    grep -Fq "$REPO_ROOT/target/release/bzr" "$REPO_ROOT/stdout"
    grep -Fq 'unset BZR_BIN' "$REPO_ROOT/stdout"
  fi
)

run_case default '' 0
run_case explicit "$FIXTURES/explicit checkout/target/release/bzr" 0
run_case relative target/release/bzr 0
run_case directory-symlink release-link/bzr 0
run_case missing missing 0
run_case debug target/debug/bzr 1
run_case file-symlink linked-bzr 1
printf 'Functional binary prerequisite fixtures passed (7 cases).\n'
