#!/bin/sh
# Run all bzr-skill checks. Exit non-zero if anything fails.
set -eu
# shellcheck disable=SC1007  # CDPATH= is intentional: zero it before cd
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rc=0

run() {
  printf '\n== %s ==\n' "$1"
  sh "$HERE/$1" || rc=1
}

# Content checks
sh "$HERE/validate-skills.sh" || rc=1
sh "$HERE/drift-check.sh" || rc=1

# Self-tests
run "validate-skills-test.sh"
run "drift-check-test.sh"
run "installer-test.sh"
run "installer-ps1-test.sh"

# Lint shell sources
printf '\n== shellcheck/shfmt ==\n'
shellcheck "$HERE/../install.sh" "$HERE"/*.sh || rc=1
shfmt -i 2 -d "$HERE/../install.sh" "$HERE"/*.sh || rc=1

exit "$rc"
