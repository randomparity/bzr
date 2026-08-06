#!/bin/sh
# Phantom-flag check for the agent-skills documentation.
#
# Every long flag the skills docs show on a `bzr <group> <verb>` invocation must
# be a real long flag of that command. This catches an agent-facing doc naming a
# flag the binary does not have -- the failure mode behind issue #503, where a
# skill template invoked `bzr comment add --is-private true` (the real flag is
# `--private`, and `--is-private` was removed from `attachment update` in #312).
#
# The check is deliberately ONE-DIRECTIONAL: a flag the docs omit is not an
# error. That is the difference from flag-drift-check.sh, which is bidirectional
# against docs/bzr-cli.md. The Command Tree there is exhaustive by contract, so
# both directions apply; the skills docs are a curated cheat-sheet, so demanding
# every binary flag appear would fail on hundreds of legitimate omissions. The
# direction that matters for correctness is the one kept: a documented flag that
# does not exist makes an agent emit a command that cannot parse, and -- when the
# flag governs privacy -- a documented fallback of "omit it" silently publishes.
#
# No globals list is needed. clap propagates the global flags into every
# subcommand's --help, so a global mentioned on a command resolves against that
# command's own flag set. (flag-drift-check.sh must filter them out only because
# it compares against a tree that lists them once on its root line.)
#
# Attribution. A command context opens on a line containing a manifest-listed
# `bzr <group> <verb>`, and covers that line's remaining flags plus any
# following *indented* continuation lines -- the reference documents most flags
# on indented sub-bullets under the invocation. The context closes at a blank
# line, a heading, a non-indented line, or another bzr invocation, so prose
# elsewhere in the file is never attributed to a command. Only the first
# invocation on a line is used.
#
# Usage: skills-flag-check.sh [MANIFEST] [SKILLS_DIR]
#   MANIFEST   defaults to the bzr-reference commands.yml (same file as
#              drift-check.sh); it bounds which (group, verb) pairs are checked.
#   SKILLS_DIR defaults to content/skills.
# Env: BZR_BIN overrides the bzr binary path.
#
# Exit: 1 on any phantom flag, a missing skills dir, or a fail-closed BZR_BIN.
#       0 on a clean match or a legitimate skip (no binary available).
set -eu

# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
MANIFEST="${1:-$HERE/../../content/skills/bzr-reference/reference/commands.yml}"
SKILLS_DIR="${2:-$HERE/../../content/skills}"
BZR="${BZR_BIN:-bzr}"
fail=0

# Resolve the binary with the same fail-closed / skip semantics as
# drift-check.sh and flag-drift-check.sh so all three behave identically.
if ! command -v "$BZR" >/dev/null 2>&1; then
  if [ -n "${BZR_BIN:-}" ]; then
    printf 'skills-flag-check: ERROR BZR_BIN set to "%s" but it is not an executable\n' "$BZR" >&2
    exit 1
  fi
  printf 'skills-flag-check: SKIPPED (no binary): BZR_BIN unset and bzr not on PATH\n'
  exit 0
fi

if [ ! -f "$MANIFEST" ]; then
  printf 'skills-flag-check: ERROR manifest not found: %s\n' "$MANIFEST" >&2
  exit 1
fi

if [ ! -d "$SKILLS_DIR" ]; then
  printf 'skills-flag-check: ERROR skills directory not found: %s\n' "$SKILLS_DIR" >&2
  exit 1
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Flatten the manifest into " group:verb " pairs for the awk membership test.
CMDS=" $(awk -F: '
  /^[[:space:]]*(#|$)/ { next }
  {
    g = $1
    gsub(/[[:space:]]/, "", g)
    if (g == "") next
    n = split($2, vs, /[[:space:]]+/)
    for (i = 1; i <= n; i++) if (vs[i] != "") print g ":" vs[i]
  }
' "$MANIFEST" | tr '\n' ' ')"

# Scan the skills docs into "group<TAB>verb<TAB>--flag<TAB>file<TAB>line" rows.
ROWS="$WORK/rows.tsv"
: >"$ROWS"
find "$SKILLS_DIR" -type f -name '*.md' | sort | while IFS= read -r f; do
  awk -v cmds="$CMDS" -v file="$f" '
function emit(s,   t) {
  while (match(s, /--[a-zA-Z][a-zA-Z0-9-]*/)) {
    t = substr(s, RSTART, RLENGTH)
    print g "\t" v "\t" t "\t" file "\t" NR
    s = substr(s, RSTART + RLENGTH)
  }
}
# A blank line or a heading always closes the current command context.
/^[[:space:]]*$/ || /^[[:space:]]*#/ { g = ""; v = ""; next }
{
  # Walk every invocation on the line, not just the first: the reference writes
  # `bzr X ...` / `bzr Y ...` on one line, and Y owns the flags that follow it.
  rest = $0
  seen = 0
  while (match(rest, /bzr[[:space:]]+[a-z][a-z0-9-]*[[:space:]]+[a-z][a-z0-9-]*/)) {
    # Text before this invocation belongs to the one opened by the previous
    # match on this line (never to a context carried in from an earlier line).
    if (seen && g != "") emit(substr(rest, 1, RSTART - 1))
    split(substr(rest, RSTART, RLENGTH), a, /[[:space:]]+/)
    rest = substr(rest, RSTART + RLENGTH)
    seen = 1
    if (index(cmds, " " a[2] ":" a[3] " ") > 0) { g = a[2]; v = a[3] }
    # A bzr invocation outside the manifest closes the context rather than
    # letting the previous command absorb this line and the ones after it.
    else { g = ""; v = "" }
  }
  if (seen) {
    if (g != "") emit(rest)   # trailing segment after the last invocation
    next
  }
  # Indented continuation of an open context; anything else closes it.
  if (g != "" && $0 ~ /^[[:space:]]/) emit($0)
  else { g = ""; v = "" }
}
' "$f" >>"$ROWS"
done

err() {
  printf 'skills-flag-check: ERROR %s\n' "$1" >&2
  fail=1
}

# Real long flags for `<group> <verb>`, from clap's option-name lines only (≤8
# leading spaces then a dash), so flags named in help prose are not treated as
# real. Same extraction as flag-drift-check.sh.
binary_flags() {
  "$BZR" "$1" "$2" --help 2>/dev/null |
    grep -E '^[[:space:]]{1,8}-' |
    grep -oE -- '--[a-zA-Z][a-zA-Z0-9-]*' |
    sort -u
}

# Compare per command, computing each command's flag set once.
cut -f1,2 "$ROWS" | sort -u | while IFS="$(printf '\t')" read -r g v; do
  [ -n "$g" ] || continue
  binary_flags "$g" "$v" >"$WORK/bin.flags"
  awk -F'\t' -v g="$g" -v v="$v" '$1==g && $2==v {print $3"\t"$4"\t"$5}' "$ROWS" |
    sort -u | while IFS="$(printf '\t')" read -r flag file line; do
    grep -qxF -- "$flag" "$WORK/bin.flags" && continue
    printf "%s:%s shows %s for \`%s %s\` but the binary has no such flag\n" \
      "$file" "$line" "$flag" "$g" "$v" >>"$WORK/errors"
  done
done

if [ -s "$WORK/errors" ]; then
  while IFS= read -r e; do err "$e"; done <"$WORK/errors"
fi

exit "$fail"
