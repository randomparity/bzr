#!/bin/sh
# Flag-level drift check for the Command Tree in docs/bzr-cli.md.
#
# Every command-specific long flag the bzr binary exposes must appear in that
# command's block of the "## Command Tree" section, and every long flag the
# tree lists for a command must be a real flag of that command. This catches
# the tree drifting from the actual CLI surface (a `--flag` added in code but
# never documented, or a documented flag that was removed/renamed).
#
# It complements drift-check.sh (which checks command *verbs* against
# commands.yml) by checking the *flag* surface, as requested in issue #306.
#
# Coverage is bounded by the manifest: only the (group, verb) commands listed in
# commands.yml are checked. Top-level commands absent from it (e.g. `completion`)
# are not validated here -- add them to the manifest if they grow flags worth
# guarding. Only long flags are compared; short-only flags (e.g. `-o`) are out
# of scope.
#
# Usage: flag-drift-check.sh [MANIFEST] [DOC]
#   MANIFEST defaults to the bzr-reference commands.yml (same file as
#            drift-check.sh); it enumerates the (group, verb) commands to check.
#   DOC      defaults to docs/bzr-cli.md.
# Env: BZR_BIN overrides the bzr binary path.
#
# Exit: 1 on any drift, a missing doc, or a fail-closed BZR_BIN. 0 on a clean
#       match or a legitimate skip (no binary available).
set -eu

# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
MANIFEST="${1:-$HERE/../skills/bzr-reference/reference/commands.yml}"
DOC="${2:-$HERE/../../docs/bzr-cli.md}"
BZR="${BZR_BIN:-bzr}"
fail=0

# Globals the tree's root line is expected to spell out, and that every command
# inherits. -v/--verbose, --help and --version are auto/standard flags the
# curated tree abbreviates (`[-v...]`) or omits, so they are not required there.
ROOT_GLOBALS='--server --output --json --config --no-color --quiet --api --timeout --retry --dry-run'
# Full inherited set excluded from per-command comparison on both sides (the
# tree lists them once on its root line, but a command block may still repeat
# one, e.g. `query run [--server <NAME>]`, and that is not drift). Derived from
# ROOT_GLOBALS plus clap's auto flags so the shared list has one source of
# truth. `--version` is deliberately absent: it is a real per-command field flag
# (`bug create --version`, `bug list --version`, ...), not propagated from the
# top-level `-V, --version` to subcommands.
GLOBALS_RE="$(printf '%s' "$ROOT_GLOBALS --verbose --help" | tr ' ' '|')"

# Resolve the binary with the same fail-closed / skip semantics as
# drift-check.sh so the two checks behave identically under CI and locally.
if ! command -v "$BZR" >/dev/null 2>&1; then
  if [ -n "${BZR_BIN:-}" ]; then
    printf 'flag-drift-check: ERROR BZR_BIN set to "%s" but it is not an executable\n' "$BZR" >&2
    exit 1
  fi
  printf 'flag-drift-check: SKIPPED (no binary): BZR_BIN unset and bzr not on PATH\n'
  exit 0
fi

if [ ! -f "$DOC" ]; then
  printf 'flag-drift-check: ERROR doc not found: %s\n' "$DOC" >&2
  exit 1
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Command-specific long flags the binary exposes for `<group> <verb>`. Only
# option-name lines (≤8 leading spaces then a dash) are scanned, so flags named
# in the prose help text are ignored; globals are filtered out.
binary_flags() {
  "$BZR" "$1" "$2" --help 2>/dev/null |
    grep -E '^[[:space:]]{1,8}-' |
    grep -oE -- '--[a-zA-Z][a-zA-Z0-9-]*' |
    grep -vxE -- "$GLOBALS_RE" |
    sort -u
}

# Group names from the manifest (the leading `<group>:` token of each entry);
# the tree parser uses this set to tell a group node (`├── bug`) from a verb
# node (`├── list`). Surrounding spaces let the parser match with `" $n "`.
groups=" $(sed -n 's/^\([A-Za-z][A-Za-z0-9_-]*\):.*/\1/p' "$MANIFEST" | tr '\n' ' ')"

# Flatten the tree into "group<TAB>verb<TAB>--flag" rows (group is "ROOT" for
# the root line). Group vs verb is decided by name, not indentation, so the
# parser is robust to the multibyte box-drawing characters and line wrapping.
TREE="$WORK/tree.tsv"
awk -v groups="$groups" '
function emit(g, v, s,   t) {
  while (match(s, /--[a-zA-Z][a-zA-Z0-9-]*/)) {
    t = substr(s, RSTART, RLENGTH)
    print g "\t" v "\t" t
    s = substr(s, RSTART + RLENGTH)
  }
}
/^## Command Tree/ { intree = 1; next }
intree && /^## / { intree = 0 }
intree && /^```/ {
  if (infence == 0) { infence = 1 } else { infence = 0; intree = 0 }
  next
}
intree && infence {
  if ($0 ~ /^bzr /) { emit("ROOT", ".", $0); next }
  if ($0 ~ /──/) {
    name = $0
    sub(/^.*── */, "", name)        # strip pipes + connector
    n = name
    sub(/[ \t].*/, "", n)           # first token = node name
    rest = name
    sub(/^[^ \t]*/, "", rest)       # remainder may carry flags
    if (index(groups, " " n " ") > 0) { g = n; v = "" }
    else { v = n; emit(g, v, rest) }
  } else if (v != "") {
    emit(g, v, $0)                  # continuation line of the current verb
  }
}
' "$DOC" >"$TREE"

err() {
  printf 'flag-drift-check: ERROR %s\n' "$1" >&2
  fail=1
}

# Per-command comparison, driven by the manifest.
while IFS= read -r line; do
  case "$line" in '' | \#*) continue ;; esac
  group=${line%%:*}
  verbs=${line#*:}
  group=$(printf '%s' "$group" | tr -d ' ')
  [ -n "$group" ] || continue
  for v in $verbs; do
    awk -F'\t' -v g="$group" -v vv="$v" '$1==g && $2==vv {print $3}' "$TREE" |
      grep -vxE -- "$GLOBALS_RE" |
      sort -u >"$WORK/doc.flags"
    binary_flags "$group" "$v" >"$WORK/bin.flags"
    for f in $(comm -23 "$WORK/bin.flags" "$WORK/doc.flags"); do
      err "command tree is missing $f for \`$group $v\`"
    done
    for f in $(comm -13 "$WORK/bin.flags" "$WORK/doc.flags"); do
      err "command tree lists $f for \`$group $v\` but the binary has no such flag"
    done
  done
done <"$MANIFEST"

# Root globals: every flag the tree promises on its root line must be present.
awk -F'\t' '$1=="ROOT"{print $3}' "$TREE" | sort -u >"$WORK/root.flags"
for gl in $ROOT_GLOBALS; do
  grep -qxF -- "$gl" "$WORK/root.flags" ||
    err "command tree root line is missing global $gl"
done

exit "$fail"
