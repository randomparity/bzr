#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
DRIFT="$HERE/drift-check.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Fake bzr: responds to `<group> --help` listing verbs under a Commands: block.
cat >"$WORK/bzr" <<'EOF'
#!/bin/sh
group="$1"
case "$group" in
bug) cat <<H
Commands:
  list  do
  view  do
H
;;
*) echo "Commands:"; ;;
esac
EOF
chmod +x "$WORK/bzr"

# manifest that matches the fake -> exit 0
printf 'bug: list view\n' >"$WORK/commands.yml"
out=$(BZR_BIN="$WORK/bzr" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "matching manifest exits 0" "0" "$rc"

# manifest with an invented verb -> non-zero, names it
printf 'bug: list view invented\n' >"$WORK/commands.yml"
out=$(BZR_BIN="$WORK/bzr" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "invented verb fails" "1" "$rc"
assert_contains "invented verb named" "$out" "invented"

# manifest missing a real verb -> warn but exit 0
printf 'bug: list\n' >"$WORK/commands.yml"
out=$(BZR_BIN="$WORK/bzr" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "missing-from-docs still exits 0" "0" "$rc"
assert_contains "missing-from-docs warns" "$out" "view"

# no bzr available -> skip gracefully, exit 0
out=$(BZR_BIN="$WORK/nope" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "absent bzr skips" "0" "$rc"
assert_contains "absent bzr message" "$out" "skip"

report "drift-check-test"
