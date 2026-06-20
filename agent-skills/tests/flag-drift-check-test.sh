#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
CHECK="$HERE/flag-drift-check.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Fake bzr: `<group> <verb> --help` prints a clap-style Options: block. The
# real check extracts long flags only from option-name lines (≤8 leading
# spaces), so prose mentions of non-flags must be ignored.
cat >"$WORK/bzr" <<'EOF'
#!/bin/sh
# args: <group> <verb> --help
case "$1 $2" in
"demo alpha")
  cat <<H
Alpha command.

Mentions --notaflag in prose which must be ignored.

Options:
      --server <SERVER>
          Global, must be excluded.
      --foo <FOO>
          A flag.
      --bar <BAR>
          Another flag.
  -h, --help
          Print help
H
  ;;
"demo beta")
  cat <<H
Options:
      --server <SERVER>
          Global.
      --baz <BAZ>
          Beta flag.
H
  ;;
"demo gamma")
  cat <<H
Options:
      --server <SERVER>
          Only globals here -> no command-specific flags.
  -h, --help
          Print help
H
  ;;
esac
EOF
chmod +x "$WORK/bzr"

printf 'demo: alpha beta gamma\n' >"$WORK/commands.yml"

# A matching doc: tree lists exactly the command-specific flags per command,
# globals only on the root line.
matching_doc() {
  cat >"$WORK/doc.md" <<'EOF'
# Title

## Command Tree

```
bzr [--server <NAME>] [--server-url <URL>] [--server-api-key-env <ENV>] [--server-email <EMAIL>] [--output table|json] [--json] [--config <PATH>] [--no-color] [--quiet] [--api rest|xmlrpc|hybrid] [--timeout <SECS>] [--retry <N>] [--dry-run] [--yes] [-v...]
├── demo
│   ├── alpha [--foo <X>] [--bar <Y>]
│   ├── beta [--baz <Z>]
│   └── gamma <ARG>
```

## Next Section

Unrelated --whatever text outside the tree must not count.
EOF
}

run_check() {
  BZR_BIN="$WORK/bzr" "$CHECK" "$WORK/commands.yml" "$WORK/doc.md" 2>&1
}

# 1. Matching tree -> exit 0.
matching_doc
out=$(run_check) && rc=0 || rc=$?
assert_eq "matching tree exits 0" "0" "$rc"

# 2. A flag present in the binary but missing from the tree -> error naming it.
matching_doc
# drop --bar from alpha's block
sed 's/\[--foo <X>\] \[--bar <Y>\]/[--foo <X>]/' "$WORK/doc.md" >"$WORK/doc.md.tmp"
mv "$WORK/doc.md.tmp" "$WORK/doc.md"
out=$(run_check) && rc=0 || rc=$?
assert_eq "missing flag fails" "1" "$rc"
assert_contains "missing flag named" "$out" "--bar"
assert_contains "missing flag attributes command" "$out" "alpha"

# 3. A flag in the tree the binary does not have -> error naming it.
matching_doc
sed 's/\[--baz <Z>\]/[--baz <Z>] [--ghost <G>]/' "$WORK/doc.md" >"$WORK/doc.md.tmp"
mv "$WORK/doc.md.tmp" "$WORK/doc.md"
out=$(run_check) && rc=0 || rc=$?
assert_eq "stale flag fails" "1" "$rc"
assert_contains "stale flag named" "$out" "--ghost"

# 4. Prose mention of a non-flag (--notaflag) is ignored.
matching_doc
out=$(run_check) && rc=0 || rc=$?
assert_eq "prose non-flag ignored (still 0)" "0" "$rc"
assert_not_contains "prose non-flag not reported" "$out" "notaflag"

# 4b. A global flag redundantly listed in a command's block is ignored, not
# flagged as stale (globals belong on the root line but may be repeated).
matching_doc
sed 's/\[--baz <Z>\]/[--baz <Z>] [--server <S>]/' "$WORK/doc.md" >"$WORK/doc.md.tmp"
mv "$WORK/doc.md.tmp" "$WORK/doc.md"
out=$(run_check) && rc=0 || rc=$?
assert_eq "global in command block ignored (still 0)" "0" "$rc"
assert_not_contains "global in command block not reported" "$out" "--server"

# 5. A required global missing from the tree root -> error.
matching_doc
sed 's/\[--no-color\] //' "$WORK/doc.md" >"$WORK/doc.md.tmp"
mv "$WORK/doc.md.tmp" "$WORK/doc.md"
out=$(run_check) && rc=0 || rc=$?
assert_eq "missing root global fails" "1" "$rc"
assert_contains "missing root global named" "$out" "--no-color"

# 6. BZR_BIN set but unresolvable -> fail closed, names BZR_BIN.
matching_doc
out=$(BZR_BIN="$WORK/nope" "$CHECK" "$WORK/commands.yml" "$WORK/doc.md" 2>&1) && rc=0 || rc=$?
assert_eq "set-but-missing BZR_BIN fails closed" "1" "$rc"
assert_contains "fail-closed names BZR_BIN" "$out" "BZR_BIN"

# 7. BZR_BIN unset and bzr not on PATH -> legitimate skip, exit 0.
mkdir -p "$WORK/toolbin"
for t in dirname grep sed sort comm awk tr cat sh mktemp; do
  p=$(command -v "$t" 2>/dev/null) && ln -sf "$p" "$WORK/toolbin/$t"
done
matching_doc
out=$(
  unset BZR_BIN
  PATH="$WORK/toolbin" "$CHECK" "$WORK/commands.yml" "$WORK/doc.md" 2>&1
) && rc=0 || rc=$?
assert_eq "unset BZR_BIN + no PATH bzr skips" "0" "$rc"
assert_contains "skip notice printed" "$out" "SKIPPED"

report "flag-drift-check-test"
