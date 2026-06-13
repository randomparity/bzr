#!/bin/sh
# Smoke-test install.ps1 via pwsh. Skips cleanly when pwsh is unavailable.
set -eu
# shellcheck disable=SC1007  # CDPATH= is intentional: zero it before cd
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
PS1="$HERE/../install.ps1"

if ! command -v pwsh >/dev/null 2>&1; then
  printf 'installer-ps1-test: pwsh not found; skipping.\n'
  exit 0
fi

run_ps() {
  # $1 = dest root, rest = args. Echoes nothing; sets global PS_RC.
  root="$1"
  shift
  if pwsh -NoProfile -Command "\$env:BZR_SKILL_DEST_ROOT='$root'; & '$PS1' $*; exit \$LASTEXITCODE" >/dev/null 2>&1; then
    PS_RC=0
  else
    PS_RC=$?
  fi
}

# clean install exits 0 and lands SKILL.md + sentinel
root=$(mktemp -d)
PS_RC=0
run_ps "$root" -Agent standard
assert_eq "ps1 clean install exits 0" "0" "$PS_RC"
assert_file "ps1 installs SKILL.md" "$root/.agents/skills/bzr-reference/SKILL.md"
assert_file "ps1 stamps sentinel" "$root/.agents/skills/bzr-reference/.bzr-skill-managed"
trash "$root" 2>/dev/null || rm -r "$root" 2>/dev/null || true

# foreign folder refusal exits non-zero, leaves content intact
root=$(mktemp -d)
mkdir -p "$root/.agents/skills/bzr-reference"
printf 'MINE\n' >"$root/.agents/skills/bzr-reference/keep.txt"
PS_RC=0
run_ps "$root" -Agent standard
assert_eq "ps1 foreign refusal exits non-zero" "1" "$PS_RC"
assert_file "ps1 foreign content kept" "$root/.agents/skills/bzr-reference/keep.txt"
trash "$root" 2>/dev/null || rm -r "$root" 2>/dev/null || true

report "installer-ps1-test"
