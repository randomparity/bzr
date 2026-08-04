#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
CHECK="$HERE/skills-flag-check.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Fake bzr: `<group> <verb> --help` prints a clap-style Options: block. Globals
# are propagated by clap into every subcommand's help, so they appear here too
# -- which is why the check needs no separate globals list.
cat >"$WORK/bzr" <<'EOF'
#!/bin/sh
# args: <group> <verb> --help
case "$1 $2" in
"demo alpha")
  cat <<H
Alpha command.

Prose mentioning --notaflag which is not an option line.

Options:
      --foo <FOO>
          A flag.
      --bar
          A bare boolean.
      --server <SERVER>
          A propagated global.
  -h, --help
          Print help
H
  ;;
"demo beta")
  cat <<H
Options:
      --baz <BAZ>
          Beta flag.
      --server <SERVER>
          A propagated global.
H
  ;;
esac
EOF
chmod +x "$WORK/bzr"

printf 'demo: alpha beta\n' >"$WORK/commands.yml"

SKILLS="$WORK/skills"
mkdir -p "$SKILLS/demo-skill"

# A clean doc: every flag shown is real for the command it is shown on.
clean_doc() {
  rm -rf "${SKILLS:?}/demo-skill"
  mkdir -p "$SKILLS/demo-skill"
  cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
# Demo skill

## demo
- `bzr demo alpha 123 --foo x --bar`
  - `--foo <FOO>` sets the thing; `--bar` is a bare boolean that takes no value.
- `bzr demo beta --baz y --server prod`

Unattributed prose about --notaflag after a blank line must not be checked.
EOF
}

run_check() {
  BZR_BIN="$WORK/bzr" "$CHECK" "$WORK/commands.yml" "$SKILLS" 2>&1
}

# 1. Clean docs -> exit 0.
clean_doc
out=$(run_check) && rc=0 || rc=$?
assert_eq "clean skills docs exit 0" "0" "$rc"

# 2. A flag the binary does not have, on the invocation line -> error naming it.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha 123 --is-private true`
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "phantom flag on invocation line fails" "1" "$rc"
assert_contains "phantom flag named" "$out" "--is-private"
assert_contains "phantom flag attributes command" "$out" "demo alpha"

# 3. A phantom on an indented continuation line -> error. This is where the
#    real reference documents most flags, so continuation coverage matters.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo beta --baz y`
  - `--baz <BAZ>` is fine; `--ghost` does not exist.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "phantom flag on continuation line fails" "1" "$rc"
assert_contains "continuation phantom named" "$out" "--ghost"

# 3b. Two invocations on one line: flags after the second belong to the second,
#     not to the first. The reference writes `X ... / Y ...` on a single line.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha --foo x` / `bzr demo beta --baz y`
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "second invocation on a line owns its own flags" "0" "$rc"

# 3c. ...and a phantom in the second invocation is attributed to the second.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha --foo x` / `bzr demo beta --foo y`
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "phantom in second invocation fails" "1" "$rc"
assert_contains "phantom attributed to second command" "$out" "demo beta"

# 4. A propagated global mentioned on a command is real -> exit 0.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha --server prod --foo x`
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "propagated global accepted" "0" "$rc"

# 5. A command absent from the manifest is not checked (coverage is bounded by
#    the manifest, same rule as flag-drift-check.sh).
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr other thing --whatever`
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "unmanifested command ignored" "0" "$rc"
assert_not_contains "unmanifested command not reported" "$out" "--whatever"

# 6. Context closes at a blank line: prose flags after it are unattributed.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha --foo x`

See also --ghost elsewhere.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "flag after blank line unattributed" "0" "$rc"

# 7. Context closes at a heading.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha --foo x`
## Another section
Mentions --ghost in prose.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "flag after heading unattributed" "0" "$rc"

# 8. BZR_BIN set but unresolvable -> fail closed, names BZR_BIN.
clean_doc
out=$(BZR_BIN="$WORK/nope" "$CHECK" "$WORK/commands.yml" "$SKILLS" 2>&1) && rc=0 || rc=$?
assert_eq "set-but-missing BZR_BIN fails closed" "1" "$rc"
assert_contains "fail-closed names BZR_BIN" "$out" "BZR_BIN"

# 9. BZR_BIN unset and bzr not on PATH -> legitimate skip, exit 0.
mkdir -p "$WORK/toolbin"
for t in dirname grep sed sort comm awk tr cat sh mktemp find; do
  p=$(command -v "$t" 2>/dev/null) && ln -sf "$p" "$WORK/toolbin/$t"
done
clean_doc
out=$(
  unset BZR_BIN
  PATH="$WORK/toolbin" "$CHECK" "$WORK/commands.yml" "$SKILLS" 2>&1
) && rc=0 || rc=$?
assert_eq "unset BZR_BIN + no PATH bzr skips" "0" "$rc"
assert_contains "skip notice printed" "$out" "SKIPPED"

# 9b. Every phantom is reported, not just the first. The comparison loop uses
#     `grep -q ... && continue` under `set -eu`, which must not abort the scan
#     on the first miss -- a check that stops at one finding hides the rest.
clean_doc
cat >"$SKILLS/demo-skill/SKILL.md" <<'EOF'
- `bzr demo alpha --ghost1 --ghost2`
- `bzr demo beta --ghost3`
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "multiple phantoms still fail" "1" "$rc"
assert_contains "first phantom reported" "$out" "--ghost1"
assert_contains "second phantom on same line reported" "$out" "--ghost2"
assert_contains "phantom on a later command reported" "$out" "--ghost3"

# 10. A missing skills directory is an error, not a silent pass.
clean_doc
out=$(BZR_BIN="$WORK/bzr" "$CHECK" "$WORK/commands.yml" "$WORK/nosuchdir" 2>&1) && rc=0 || rc=$?
assert_eq "missing skills dir fails" "1" "$rc"

report "skills-flag-check-test"
