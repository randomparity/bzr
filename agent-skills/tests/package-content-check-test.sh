#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
CHECK="$HERE/package-content-check.sh"
WORK=$(mktemp -d)
trap 'rm -r "$WORK"' EXIT

mkdir -p "$WORK/content/skills/one" "$WORK/content/skills/two/reference"
printf '%s\n' 'one' >"$WORK/content/skills/one/SKILL.md"
printf '%s\n' 'two' >"$WORK/content/skills/two/SKILL.md"
printf '%s\n' 'reference' >"$WORK/content/skills/two/reference/commands.md"

cat >"$WORK/complete.list" <<'EOF'
content/skills/one/SKILL.md
content/skills/two/SKILL.md
content/skills/two/reference/commands.md
EOF
out=$(sh "$CHECK" "$WORK/complete.list" "$WORK/content/skills" 2>&1) && rc=0 || rc=$?
assert_eq "complete package list passes" "0" "$rc"
assert_contains "success reports checked file count" "$out" "checked 3 canonical skill files"

cat >"$WORK/incomplete.list" <<'EOF'
content/skills/one/SKILL.md
content/skills/two/SKILL.md
EOF
out=$(sh "$CHECK" "$WORK/incomplete.list" "$WORK/content/skills" 2>&1) && rc=0 || rc=$?
assert_eq "missing canonical file fails" "1" "$rc"
assert_contains "failure names omitted file" "$out" \
  "content/skills/two/reference/commands.md"

report "package-content-check-test"
