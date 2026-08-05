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

# Remote archives from current releases use content/skills. Explicitly pinned
# historical archives use agent-skills/skills, and a transition archive must
# prefer its canonical content/skills payload when both are present.
fx=$(mktemp -d)

write_skills() {
  root="$1"
  marker="$2"
  for skill in bzr-reference bzr-setup bzr-file-bug bzr-triage-bug bzr-search-report bzr-bulk-triage; do
    mkdir -p "$root/$skill"
    printf '%s\n' "$marker" >"$root/$skill/SKILL.md"
  done
}

current_root="$fx/current/bzr-fixture"
mkdir -p "$current_root/content"
write_skills "$current_root/content/skills" "CURRENT-ONLY"
pwsh -NoProfile -Command "Compress-Archive -Path '$current_root' -DestinationPath '$fx/current.zip'" >/dev/null 2>&1

historical_root="$fx/historical/bzr-fixture"
mkdir -p "$historical_root/agent-skills"
write_skills "$historical_root/agent-skills/skills" "HISTORICAL-ONLY"
pwsh -NoProfile -Command "Compress-Archive -Path '$historical_root' -DestinationPath '$fx/historical.zip'" >/dev/null 2>&1

both_root="$fx/both/bzr-fixture"
mkdir -p "$both_root/content" "$both_root/agent-skills"
write_skills "$both_root/content/skills" "CURRENT-WINS"
write_skills "$both_root/agent-skills/skills" "HISTORICAL-LOSES"
pwsh -NoProfile -Command "Compress-Archive -Path '$both_root' -DestinationPath '$fx/both.zip'" >/dev/null 2>&1

root=$(mktemp -d)
PS_RC=0
if pwsh -NoProfile -Command "\$env:BZR_SKILL_DEST_ROOT='$root'; \$env:BZR_SKILL_TARBALL_URL='$fx/current.zip'; & '$PS1' -Agent standard; exit \$LASTEXITCODE" >/dev/null 2>&1; then PS_RC=0; else PS_RC=$?; fi
assert_eq "ps1 current-layout remote install exits 0" "0" "$PS_RC"
marker=$(cat "$root/.agents/skills/bzr-reference/SKILL.md" 2>/dev/null || true)
assert_contains "ps1 current-layout installs canonical payload" "$marker" "CURRENT-ONLY"
trash "$root" 2>/dev/null || rm -r "$root" 2>/dev/null || true

root=$(mktemp -d)
PS_RC=0
if pwsh -NoProfile -Command "\$env:BZR_SKILL_DEST_ROOT='$root'; \$env:BZR_SKILL_TARBALL_URL='$fx/historical.zip'; & '$PS1' -Agent standard; exit \$LASTEXITCODE" >/dev/null 2>&1; then PS_RC=0; else PS_RC=$?; fi
assert_eq "ps1 historical-layout remote install exits 0" "0" "$PS_RC"
marker=$(cat "$root/.agents/skills/bzr-reference/SKILL.md" 2>/dev/null || true)
assert_contains "ps1 historical-layout installs fallback payload" "$marker" "HISTORICAL-ONLY"
trash "$root" 2>/dev/null || rm -r "$root" 2>/dev/null || true

root=$(mktemp -d)
PS_RC=0
if pwsh -NoProfile -Command "\$env:BZR_SKILL_DEST_ROOT='$root'; \$env:BZR_SKILL_TARBALL_URL='$fx/both.zip'; & '$PS1' -Agent standard; exit \$LASTEXITCODE" >/dev/null 2>&1; then PS_RC=0; else PS_RC=$?; fi
assert_eq "ps1 both-layout remote install exits 0" "0" "$PS_RC"
marker=$(cat "$root/.agents/skills/bzr-reference/SKILL.md" 2>/dev/null || true)
assert_contains "ps1 both-layout prefers canonical payload" "$marker" "CURRENT-WINS"
trash "$root" "$fx" 2>/dev/null || rm -r "$root" "$fx" 2>/dev/null || true

report "installer-ps1-test"
