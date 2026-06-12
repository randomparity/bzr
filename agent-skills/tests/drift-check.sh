#!/bin/sh
# Compare a command manifest against the real bzr binary, both directions.
# Listed-but-absent => error (exit 1). Real-but-unlisted => warning (exit 0).
# Usage: drift-check.sh [MANIFEST]   env: BZR_BIN overrides the bzr binary path.
set -eu

# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
MANIFEST="${1:-$HERE/../skills/bzr-reference/reference/commands.yml}"
BZR="${BZR_BIN:-bzr}"
fail=0

if ! command -v "$BZR" >/dev/null 2>&1; then
  printf 'drift-check: bzr not found (%s); skip.\n' "$BZR"
  exit 0
fi

# Verbs the binary reports for a group: parse the `Commands:` block of --help.
real_verbs() {
  "$BZR" "$1" --help 2>/dev/null | awk '
    /^Commands:/ { inb=1; next }
    inb && /^[A-Za-z]/ { exit }
    inb && /^[ \t]+[a-z]/ {
      gsub(/^[ \t]+/,""); print $1
    }
  '
}

while IFS= read -r line; do
  case "$line" in
  '' | \#*) continue ;;
  esac
  group=${line%%:*}
  verbs=${line#*:}
  group=$(printf '%s' "$group" | tr -d ' ')
  [ -n "$group" ] || continue

  real=$(real_verbs "$group" || true)

  # Direction 1: every listed verb must exist.
  for v in $verbs; do
    found=0
    for r in $real; do [ "$v" = "$r" ] && found=1 && break; done
    if [ "$found" -eq 0 ]; then
      printf 'drift-check: ERROR %s: documented verb "%s" not in bzr surface\n' "$group" "$v" >&2
      fail=1
    fi
  done

  # Direction 2: real verbs not listed -> warn only.
  for r in $real; do
    [ "$r" = "help" ] && continue
    listed=0
    for v in $verbs; do [ "$v" = "$r" ] && listed=1 && break; done
    [ "$listed" -eq 0 ] && printf 'drift-check: warn %s: bzr verb "%s" not documented\n' "$group" "$r"
  done
done <"$MANIFEST"

exit "$fail"
