#!/usr/bin/env bash
# Exercise the real runner's runtime boundaries without containers or compilation.
# SC1091: lib.sh is resolved from ROOT; the gate lists it separately.
# SC2030/SC2031: cases deliberately isolate HOME/XDG and expectations in subshells.
# shellcheck disable=SC1090,SC1091,SC2030,SC2031,SC2034,SC2329
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
FIXTURES=$(mktemp -d)
trap 'rm -rf "$FIXTURES"' EXIT
export RUNTIME_TRACE="$FIXTURES/trace" RUNTIME_STDIN="$FIXTURES/stdin"
export RUNTIME_DOUBLE="$FIXTURES/runtime"

cat >"$RUNTIME_DOUBLE" <<'SH'
#!/bin/bash
set -euo pipefail
[[ ${HOME+x} == "$EXPECT_HOME_SET" && ${HOME-} == "$EXPECT_HOME" ]] || {
    echo 'runtime HOME differs from caller environment' >&2; exit 90;
}
[[ ${XDG_CONFIG_HOME+x} == "$EXPECT_XDG_SET" && ${XDG_CONFIG_HOME-} == "$EXPECT_XDG" ]] || {
    echo 'runtime XDG_CONFIG_HOME differs from caller environment' >&2; exit 91;
}
printf '%s\n' "$@" >>"$RUNTIME_TRACE"
if [[ $1 == exec && ${2:-} == -i ]]; then cat >"$RUNTIME_STDIN"; fi
printf 'runtime stdout\n'
printf 'runtime stderr\n' >&2
exit "${RUNTIME_EXIT:-0}"
SH
chmod +x "$RUNTIME_DOUBLE"

RUNNER="$FIXTURES/checkout/tests/functional"
mkdir -p "$RUNNER/phases"
cp "$ROOT/tests/functional/run-tests.sh" "$RUNNER/run-tests.sh"
cp "$ROOT/tests/functional/lib.sh" "$ROOT/tests/functional/container-env.sh" "$RUNNER/"
cat >"$RUNNER/setup-bugzilla.sh" <<'SH'
#!/bin/bash
exec "$RUNTIME_DOUBLE" "$@"
SH
chmod +x "$RUNNER/setup-bugzilla.sh"
export LINKS_PHASE="$ROOT/tests/functional/phases/09c-bug-links.sh"

cat >"$RUNNER/phases/00-build.sh" <<'SH'
# shellcheck shell=bash
container_runtime() { printf '%s' "$RUNTIME_DOUBLE"; }
export HOME="$FUNC_CONFIG_DIR/changed home" XDG_CONFIG_HOME="$FUNC_CONFIG_DIR/changed xdg"
printf 'select 1;\n' >"$FUNC_CONFIG_DIR/fixture sql"
run_bugzilla_sql_file "$FUNC_CONFIG_DIR/fixture sql"
[[ $HOME == "$FUNC_CONFIG_DIR/changed home" && $XDG_CONFIG_HOME == "$FUNC_CONFIG_DIR/changed xdg" ]]
# Exercise the complete CGI phase, stubbing only bzr/server assertions.
run_bzr() { :; }
run_bzr_raw() { :; }
test_begin() { :; }
test_skip() { :; }
test_fail() { echo "$*" >&2; exit 92; }
assert_success() { return 1; }
assert_exit_code() { return 1; }
source "$LINKS_PHASE"
[[ $HOME == "$FUNC_CONFIG_DIR/changed home" && $XDG_CONFIG_HOME == "$FUNC_CONFIG_DIR/changed xdg" ]]
export RUNTIME_EXIT=37
rc=0
with_runtime_environment "$RUNTIME_DOUBLE" 'argument with spaces' '' >"$FUNC_CONFIG_DIR/out" 2>"$FUNC_CONFIG_DIR/err" || rc=$?
[[ $rc == 37 ]] || { echo "runtime status changed: $rc" >&2; exit 93; }
[[ $(cat "$FUNC_CONFIG_DIR/out") == 'runtime stdout' ]] || exit 94
[[ $(cat "$FUNC_CONFIG_DIR/err") == 'runtime stderr' ]] || exit 95
[[ $HOME == "$FUNC_CONFIG_DIR/changed home" && $XDG_CONFIG_HOME == "$FUNC_CONFIG_DIR/changed xdg" ]] || exit 96
unset RUNTIME_EXIT
# A failing phase must still reclaim its container with the original environment.
exit 23
SH

run_case() (
  local home_state=$1 xdg_state=$2
  case $home_state in
  set) export HOME="$FIXTURES/original home" ;;
  empty) export HOME='' ;;
  unset) unset HOME ;;
  esac
  case $xdg_state in
  set) export XDG_CONFIG_HOME="$FIXTURES/original xdg" ;;
  empty) export XDG_CONFIG_HOME='' ;;
  unset) unset XDG_CONFIG_HOME ;;
  esac
  export EXPECT_HOME_SET=${HOME+x} EXPECT_HOME=${HOME-}
  export EXPECT_XDG_SET=${XDG_CONFIG_HOME+x} EXPECT_XDG=${XDG_CONFIG_HOME-}
  : >"$RUNTIME_TRACE"
  local rc=0
  BZR_FUNC_PORT=1234 BZR_FUNC_CONTAINER='fixture-container' BZR_FUNC_KEEP='' \
    bash "$RUNNER/run-tests.sh" >"$FIXTURES/out" 2>"$FIXTURES/err" || rc=$?
  if [[ $rc != 23 ]]; then
    printf 'HOME=%s XDG=%s: expected phase exit 23, got %s\n' "$home_state" "$xdg_state" "$rc" >&2
    cat "$FIXTURES/out" "$FIXTURES/err" >&2
    return 1
  fi
  [[ $(cat "$RUNTIME_STDIN") == 'select 1;' ]]
  # Literal argv expectations catch splitting, missing migrations and skipped cleanup.
  cat >"$FIXTURES/expected" <<EOF
exec
-i
fixture-container
mysql
-u
root
bugs
cp
$RUNNER/fixtures/redhat-links.cgi
fixture-container:/var/www/html/bugzilla/redhat-links.cgi
exec
fixture-container
chmod
755
/var/www/html/bugzilla/redhat-links.cgi
exec
fixture-container
rm
-f
/var/www/html/bugzilla/redhat-links.cgi
argument with spaces

stop
EOF
  diff -u "$FIXTURES/expected" "$RUNTIME_TRACE"
)

for home_state in set empty unset; do
  for xdg_state in set empty unset; do run_case "$home_state" "$xdg_state"; done
done

# Other consumers source lib.sh without the functional runner's snapshot.
(
  source "$ROOT/tests/functional/lib.sh"
  export EXPECT_HOME_SET=${HOME+x} EXPECT_HOME=${HOME-}
  export EXPECT_XDG_SET=${XDG_CONFIG_HOME+x} EXPECT_XDG=${XDG_CONFIG_HOME-}
  with_runtime_environment "$RUNTIME_DOUBLE" unsnapshotted
) >"$FIXTURES/out" 2>"$FIXTURES/err"
printf 'Functional runtime environment checks passed (9 state pairs and unsnapshotted consumer).\n'
