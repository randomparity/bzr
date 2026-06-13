#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
INSTALL="$HERE/../install.sh"
SENTINEL=".bzr-skill-managed"

newroot() { mktemp -d; }

# --help exits 0 and mentions usage
out=$("$INSTALL" --help 2>&1) && rc=0 || rc=$?
assert_eq "help exits 0" "0" "$rc"
assert_contains "help shows --agent" "$out" "--agent"

# unknown agent exits non-zero
root=$(newroot)
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent bogus 2>&1) && rc=0 || rc=$?
assert_eq "unknown agent fails" "1" "$rc"
rm -rf "$root"

# dry-run standard: names all five skills, writes nothing
root=$(newroot)
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard --dry-run 2>&1) && rc=0 || rc=$?
assert_eq "dry-run exits 0" "0" "$rc"
assert_contains "dry-run names reference" "$out" "bzr-reference"
assert_contains "dry-run names triage" "$out" "bzr-triage-bug"
assert_contains "dry-run targets agents dir" "$out" ".agents/skills"
assert_no_path "dry-run wrote nothing" "$root/.agents/skills/bzr-reference"
rm -rf "$root"

# dry-run all: targets both dirs
root=$(newroot)
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent all --dry-run 2>&1) && rc=0 || rc=$?
assert_contains "dry-run all targets claude dir" "$out" ".claude/skills"
rm -rf "$root"

# real install writes folders + sentinels, reference subtree lands
root=$(newroot)
BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard >/dev/null 2>&1
assert_file "installs reference SKILL.md" "$root/.agents/skills/bzr-reference/SKILL.md"
assert_file "stamps sentinel" "$root/.agents/skills/bzr-reference/$SENTINEL"
assert_file "copies reference subtree" "$root/.agents/skills/bzr-reference/reference/commands.md"
sv=$(cat "$root/.agents/skills/bzr-reference/$SENTINEL")
assert_contains "sentinel records source-version" "$sv" "source-version:"

# idempotent re-run: still exits 0, folder still present
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "re-run idempotent exit 0" "0" "$rc"
assert_file "re-run keeps SKILL.md" "$root/.agents/skills/bzr-reference/SKILL.md"
rm -rf "$root"

# foreign folder (no sentinel) is refused, left intact, non-zero
root=$(newroot)
mkdir -p "$root/.agents/skills/bzr-reference"
printf 'MINE\n' >"$root/.agents/skills/bzr-reference/keep.txt"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "foreign folder refusal non-zero" "1" "$rc"
assert_file "foreign content untouched" "$root/.agents/skills/bzr-reference/keep.txt"
assert_no_path "foreign folder not stamped" "$root/.agents/skills/bzr-reference/$SENTINEL"

# --force overwrites the foreign folder
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard --force 2>&1) && rc=0 || rc=$?
assert_eq "force overwrites foreign exit 0" "0" "$rc"
assert_file "force stamps sentinel" "$root/.agents/skills/bzr-reference/$SENTINEL"
assert_no_path "force replaced foreign content" "$root/.agents/skills/bzr-reference/keep.txt"
rm -rf "$root"

# symlink destination refused even with --force
root=$(newroot)
mkdir -p "$root/.agents/skills" "$root/elsewhere"
ln -s "$root/elsewhere" "$root/.agents/skills/bzr-reference"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard --force 2>&1) && rc=0 || rc=$?
assert_eq "symlink refused even with force" "1" "$rc"
assert_no_path "nothing written through symlink" "$root/elsewhere/SKILL.md"
rm -rf "$root"

# a held lock blocks a second run; stale (dead-pid) lock is broken
root=$(newroot)
ensure_lockdir="$root/.agents/skills/.bzr-skill.lock"
mkdir -p "$ensure_lockdir"
printf '999999\n' >"$ensure_lockdir/pid" # almost certainly-dead pid
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "stale lock is broken, install proceeds" "0" "$rc"
assert_file "install completed past stale lock" "$root/.agents/skills/bzr-reference/SKILL.md"
rm -rf "$root"

# a live lock (our own pid) blocks and reports
root=$(newroot)
livelock="$root/.agents/skills/.bzr-skill.lock"
mkdir -p "$livelock"
printf '%s\n' "$$" >"$livelock/pid"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "live lock blocks" "1" "$rc"
assert_contains "live lock message" "$out" "locked"
rm -rf "$root"

# uninstall removes owned folders, leaves a foreign same-named folder alone
root=$(newroot)
BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard >/dev/null 2>&1
mkdir -p "$root/.agents/skills/bzr-setup" # turn bzr-setup foreign by removing sentinel
rm -f "$root/.agents/skills/bzr-setup/$SENTINEL"
printf 'FOREIGN\n' >"$root/.agents/skills/bzr-setup/foreign.txt"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard --uninstall 2>&1) && rc=0 || rc=$?
assert_eq "uninstall exits 0" "0" "$rc"
assert_no_path "owned folder removed" "$root/.agents/skills/bzr-reference"
assert_file "foreign folder kept" "$root/.agents/skills/bzr-setup/foreign.txt"
rm -rf "$root"

# list shows present, absent, shadowed, and stale states
root=$(newroot)
BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard >/dev/null 2>&1
# make bzr-file-bug stale by rewriting its sentinel version
sed 's/^source-version:.*/source-version: 0.0.1/' \
  "$root/.agents/skills/bzr-file-bug/$SENTINEL" >"$root/sm" && mv "$root/sm" "$root/.agents/skills/bzr-file-bug/$SENTINEL"
# make bzr-setup shadowed (foreign)
rm -f "$root/.agents/skills/bzr-setup/$SENTINEL"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard --list 2>&1) && rc=0 || rc=$?
assert_eq "list exits 0" "0" "$rc"
assert_contains "list shows present" "$out" "present"
assert_contains "list flags stale" "$out" "stale"
assert_contains "list flags shadowed" "$out" "shadowed"
rm -rf "$root"

# install warns (but succeeds) when bzr is not on PATH.
# Build a tool dir with the coreutils the installer needs but no bzr, so the
# probe fires while the install still finds its tools. Keying the PATH off a
# single tool's dir (e.g. dirname "$(command -v cp)") is not portable: on macOS
# cp is in /bin but dirname is in /usr/bin, so that drops dirname and the
# installer's own dirname call fails for the wrong reason.
root=$(newroot)
toolbin="$root/.toolbin"
mkdir -p "$toolbin"
for t in sh dirname cp mkdir mv rm cat; do
  p=$(command -v "$t" 2>/dev/null) && ln -sf "$p" "$toolbin/$t"
done
out=$(PATH="$toolbin" BZR_SKILL_DEST_ROOT="$root" sh "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "install succeeds without bzr" "0" "$rc"
assert_contains "warns about missing bzr" "$out" "bzr not found on PATH"
rm -rf "$root"

# FIX C2: dest root containing a space installs to the correct path (no word-splitting)
parent=$(newroot)
root="$parent/a b"
mkdir -p "$root"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "spacey dest root install exits 0" "0" "$rc"
assert_file "spacey dest root got skill" "$root/.agents/skills/bzr-reference/SKILL.md"
rm -rf "$parent"

# FIX C1: intermediate-path symlink is refused even with --force; nothing written through
root=$(newroot)
mkdir -p "$root/outside"
ln -s "$root/outside" "$root/.agents"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard --force 2>&1) && rc=0 || rc=$?
assert_eq "intermediate symlink refused" "1" "$rc"
assert_no_path "nothing written through ancestor symlink" "$root/outside/skills/bzr-reference/SKILL.md"
rm -rf "$root"

# FIX I4: --agent all: if the claude lock is held live, the agents lock must not leak
root=$(newroot)
mkdir -p "$root/.claude/skills/.bzr-skill.lock"
printf '%s\n' "$$" >"$root/.claude/skills/.bzr-skill.lock/pid"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent all 2>&1) && rc=0 || rc=$?
assert_eq "all aborts when claude locked" "1" "$rc"
assert_no_path "agents lock not leaked" "$root/.agents/skills/.bzr-skill.lock"
rm -rf "$root"

# FIX M5: empty pid file is treated as live (fail-closed), not stale
root=$(newroot)
emptylock="$root/.agents/skills/.bzr-skill.lock"
mkdir -p "$emptylock"
: >"$emptylock/pid"
out=$(BZR_SKILL_DEST_ROOT="$root" "$INSTALL" --agent standard 2>&1) && rc=0 || rc=$?
assert_eq "empty pid lock blocks (fail-closed)" "1" "$rc"
assert_contains "empty pid lock message contains locked" "$out" "locked"
rm -rf "$root"

report "installer-test"
