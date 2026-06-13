#!/bin/sh
# Validate skill folders: frontmatter, name==folder, description<=500, links.
# Usage: validate-skills.sh [SKILLS_DIR]   (defaults to ../skills relative to repo)
set -eu

# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SKILLS_DIR="${1:-$HERE/../skills}"
MAX_DESC=500
fail=0

err() {
  printf 'validate: %s\n' "$1" >&2
  fail=1
}

# Extract a frontmatter scalar field's value (first match) from a SKILL.md.
# Frontmatter is the block between the first two '---' lines.
fm_field() {
  awk -v key="$2" '
    NR==1 && $0=="---" { infm=1; next }
    infm && $0=="---" { exit }
    infm {
      idx=index($0, ":")
      if (idx>0) {
        k=substr($0,1,idx-1)
        v=substr($0,idx+1)
        sub(/^[ \t]+/,"",k); sub(/[ \t]+$/,"",k)
        sub(/^[ \t]+/,"",v); sub(/[ \t]+$/,"",v)
        if (k==key) { print v; exit }
      }
    }
  ' "$1"
}

has_frontmatter() {
  head -n 1 "$1" | grep -q '^---$'
}

for skilldir in "$SKILLS_DIR"/*/; do
  [ -d "$skilldir" ] || continue
  name=$(basename "$skilldir")
  md="$skilldir/SKILL.md"

  if [ ! -f "$md" ]; then
    err "$name: missing SKILL.md"
    continue
  fi
  if ! has_frontmatter "$md"; then
    err "$name: missing or malformed frontmatter (must start with ---)"
    continue
  fi

  fmname=$(fm_field "$md" name)
  desc=$(fm_field "$md" description)

  [ "$fmname" = "$name" ] || err "$name: frontmatter name [$fmname] != folder name [$name]"
  if [ -z "$desc" ]; then
    err "$name: description is empty"
  elif [ "${#desc}" -gt "$MAX_DESC" ]; then
    err "$name: description is ${#desc} chars (max $MAX_DESC)"
  fi

  # Resolve relative reference links of the form ](reference/<file>)
  grep -o '](reference/[^)]*)' "$md" 2>/dev/null | sed 's/^](//; s/)$//' | while IFS= read -r rel; do
    [ -z "$rel" ] && continue
    if [ ! -f "$skilldir/$rel" ]; then
      printf 'validate: %s: broken link to %s\n' "$name" "$rel" >&2
      : >"$skilldir/.link-broken-marker"
    fi
  done
  if [ -f "$skilldir/.link-broken-marker" ]; then
    rm -f "$skilldir/.link-broken-marker"
    fail=1
  fi
done

exit "$fail"
