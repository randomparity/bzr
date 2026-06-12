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

# BZR_BIN set but unresolvable -> fail closed (non-zero), names BZR_BIN.
out=$(BZR_BIN="$WORK/nope" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "set-but-missing BZR_BIN fails closed" "1" "$rc"
assert_contains "fail-closed names BZR_BIN" "$out" "BZR_BIN"

# BZR_BIN unset and bzr not on PATH -> legitimate skip, exit 0, visible notice.
# Build a tool dir with the coreutils drift-check needs but no bzr, then run with
# only that on PATH. Explicitly unset BZR_BIN in the subshell so the arm is
# deterministic even when the suite is run with BZR_BIN exported (CI / run.sh).
mkdir -p "$WORK/toolbin"
for t in dirname awk tr cat sh; do
  p=$(command -v "$t" 2>/dev/null) && ln -sf "$p" "$WORK/toolbin/$t"
done
out=$(
  unset BZR_BIN
  PATH="$WORK/toolbin" "$DRIFT" "$WORK/commands.yml" 2>&1
) && rc=0 || rc=$?
assert_eq "unset BZR_BIN + no PATH bzr skips" "0" "$rc"
assert_contains "skip notice printed" "$out" "SKIPPED"

report "drift-check-test"
