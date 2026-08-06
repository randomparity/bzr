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

run_remote_ps() {
  root="$1"
  archive="$2"
  command="\$env:BZR_SKILL_DEST_ROOT='$root'; "
  command="${command}\$env:BZR_SKILL_TARBALL_URL='$archive'; "
  command="${command}& '$PS1' -Agent standard; exit \$LASTEXITCODE"
  if pwsh -NoProfile -Command "$command" >/dev/null 2>&1; then
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
  for skill in \
    bzr-reference bzr-setup bzr-file-bug bzr-triage-bug \
    bzr-search-report bzr-bulk-triage; do
    mkdir -p "$root/$skill"
    printf '%s\n' "$marker" >"$root/$skill/SKILL.md"
  done
}

current_root="$fx/current/bzr-fixture"
mkdir -p "$current_root/content" "$current_root/agent-skills"
write_skills "$current_root/content/skills" "CURRENT-ONLY"
printf '%s\n' 'CURRENT-9.9.1' >"$current_root/agent-skills/VERSION"
current_archive="$fx/current.zip"
archive_command="Compress-Archive -Path '$current_root' "
archive_command="${archive_command}-DestinationPath '$current_archive'"
pwsh -NoProfile -Command "$archive_command" >/dev/null 2>&1

historical_root="$fx/historical/bzr-fixture"
mkdir -p "$historical_root/agent-skills"
write_skills "$historical_root/agent-skills/skills" "HISTORICAL-ONLY"
printf '%s\n' 'HISTORICAL-9.9.2' >"$historical_root/agent-skills/VERSION"
historical_archive="$fx/historical.zip"
archive_command="Compress-Archive -Path '$historical_root' "
archive_command="${archive_command}-DestinationPath '$historical_archive'"
pwsh -NoProfile -Command "$archive_command" >/dev/null 2>&1

both_root="$fx/both/bzr-fixture"
mkdir -p "$both_root/content" "$both_root/agent-skills"
write_skills "$both_root/content/skills" "CURRENT-WINS"
write_skills "$both_root/agent-skills/skills" "HISTORICAL-LOSES"
printf '%s\n' 'BOTH-9.9.3' >"$both_root/agent-skills/VERSION"
both_archive="$fx/both.zip"
archive_command="Compress-Archive -Path '$both_root' "
archive_command="${archive_command}-DestinationPath '$both_archive'"
pwsh -NoProfile -Command "$archive_command" >/dev/null 2>&1

root=$(mktemp -d)
PS_RC=0
run_remote_ps "$root" "$current_archive"
assert_eq "ps1 current-layout remote install exits 0" "0" "$PS_RC"
marker=$(cat "$root/.agents/skills/bzr-reference/SKILL.md" 2>/dev/null || true)
assert_contains "ps1 current-layout installs canonical payload" "$marker" "CURRENT-ONLY"
sv=$(cat "$root/.agents/skills/bzr-reference/.bzr-skill-managed" 2>/dev/null || true)
assert_contains "ps1 current-layout uses canonical version" "$sv" "CURRENT-9.9.1"
trash "$root" 2>/dev/null || rm -r "$root" 2>/dev/null || true

root=$(mktemp -d)
PS_RC=0
run_remote_ps "$root" "$historical_archive"
assert_eq "ps1 historical-layout remote install exits 0" "0" "$PS_RC"
marker=$(cat "$root/.agents/skills/bzr-reference/SKILL.md" 2>/dev/null || true)
assert_contains "ps1 historical-layout installs fallback payload" "$marker" "HISTORICAL-ONLY"
sv=$(cat "$root/.agents/skills/bzr-reference/.bzr-skill-managed" 2>/dev/null || true)
assert_contains "ps1 historical-layout uses fallback version" "$sv" "HISTORICAL-9.9.2"
trash "$root" 2>/dev/null || rm -r "$root" 2>/dev/null || true

root=$(mktemp -d)
PS_RC=0
run_remote_ps "$root" "$both_archive"
assert_eq "ps1 both-layout remote install exits 0" "0" "$PS_RC"
marker=$(cat "$root/.agents/skills/bzr-reference/SKILL.md" 2>/dev/null || true)
assert_contains "ps1 both-layout prefers canonical payload" "$marker" "CURRENT-WINS"
sv=$(cat "$root/.agents/skills/bzr-reference/.bzr-skill-managed" 2>/dev/null || true)
assert_contains "ps1 both-layout uses canonical version" "$sv" "BOTH-9.9.3"
trash "$root" "$fx" 2>/dev/null || rm -r "$root" "$fx" 2>/dev/null || true

report "installer-ps1-test"
