#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
CHECK="$HERE/version-check.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

ROOT="$WORK/agent-skills"
CARGO="$WORK/Cargo.toml"

# A dependency table carries its own `version =`, so the parser must be bounded
# by [package] rather than taking the first one it sees.
cat >"$CARGO" <<'EOF'
[package]
name = "bzr"
version = "1.2.3-dev"
edition = "2021"

[dependencies]
reqwest = { version = "9.9.9" }
EOF

# A tree where all five sites agree with the crate version. The README claim is
# split across a line break, matching the real file's wrap.
clean_tree() {
  rm -rf "$ROOT"
  mkdir -p "$ROOT/skills/bzr-reference/reference"
  printf '1.2.3-dev\n' >"$ROOT/VERSION"
  cat >"$ROOT/README.md" <<'EOF'
# Agent Skills

The command surface is authored
against `bzr` 1.2.3-dev. The installer copies whole skill folders.
EOF
  cat >"$ROOT/skills/bzr-reference/SKILL.md" <<'EOF'
This reference is authored against **bzr 1.2.3-dev**. Check `bzr --help`.
EOF
  cat >"$ROOT/skills/bzr-reference/reference/commands.md" <<'EOF'
# bzr command surface (authored against bzr 1.2.3-dev)
EOF
  cat >"$ROOT/skills/bzr-reference/reference/commands.yml" <<'EOF'
# bzr command manifest — authored against bzr 1.2.3-dev.
bug: list view
EOF
}

run_check() {
  "$CHECK" "$CARGO" "$ROOT" 2>&1
}

# 1. Every site agrees -> exit 0. This also proves the [package] version wins
#    over the 9.9.9 in [dependencies].
clean_tree
out=$(run_check) && rc=0 || rc=$?
assert_eq "agreeing tree exits 0" "0" "$rc"

# 2. VERSION alone drifts -> fail naming both versions. This is the exact shape
#    of #507: the file the README's contract sentence points at.
clean_tree
printf '0.6.1-dev\n' >"$ROOT/VERSION"
out=$(run_check) && rc=0 || rc=$?
assert_eq "stale VERSION fails" "1" "$rc"
assert_contains "stale VERSION named" "$out" "0.6.1-dev"
assert_contains "crate version named" "$out" "1.2.3-dev"

# 3. A prose claim drifts while VERSION is correct -> still fails. A
#    VERSION-only check would pass here, which is why this one is broader.
clean_tree
cat >"$ROOT/skills/bzr-reference/reference/commands.md" <<'EOF'
# bzr command surface (authored against bzr 0.6.1-dev)
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "stale prose claim fails" "1" "$rc"
assert_contains "stale claim names the file" "$out" "commands.md"
assert_contains "stale claim names the version" "$out" "0.6.1-dev"

# 4. The README's claim wraps across a line break; a line-at-a-time scan would
#    miss it entirely and report a clean tree.
clean_tree
cat >"$ROOT/README.md" <<'EOF'
# Agent Skills

The command surface is authored
against `bzr` 0.6.1-dev. The installer copies whole skill folders.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "wrapped stale claim fails" "1" "$rc"
assert_contains "wrapped claim names the file" "$out" "README.md"

# 5. Every stale claim is reported, not just the first -- a check that stops at
#    one finding sends an author back for a second round per site.
clean_tree
printf '0.6.1-dev\n' >"$ROOT/VERSION"
cat >"$ROOT/skills/bzr-reference/reference/commands.md" <<'EOF'
# bzr command surface (authored against bzr 0.7.0)
EOF
cat >"$ROOT/skills/bzr-reference/reference/commands.yml" <<'EOF'
# bzr command manifest — authored against bzr 0.8.0.
bug: list view
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "multiple stale sites fail" "1" "$rc"
assert_contains "VERSION reported" "$out" "VERSION"
assert_contains "first stale claim reported" "$out" "0.7.0"
assert_contains "second stale claim reported" "$out" "0.8.0"

# 6. A claim in a skill other than bzr-reference is covered too: the scan
#    discovers claims rather than reading a hardcoded path list.
clean_tree
mkdir -p "$ROOT/skills/bzr-setup"
cat >"$ROOT/skills/bzr-setup/SKILL.md" <<'EOF'
This skill is authored against bzr 0.6.1-dev.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "stale claim in another skill fails" "1" "$rc"
assert_contains "other skill named" "$out" "bzr-setup"

# 7. Zero claims is an error, not a pass: without it, deleting or rewording
#    every claim would turn the check green while enforcing nothing.
clean_tree
printf '# Agent Skills\n' >"$ROOT/README.md"
rm -f "$ROOT/skills/bzr-reference/SKILL.md" \
  "$ROOT/skills/bzr-reference/reference/commands.md" \
  "$ROOT/skills/bzr-reference/reference/commands.yml"
out=$(run_check) && rc=0 || rc=$?
assert_eq "zero claims fails" "1" "$rc"
assert_contains "zero claims explained" "$out" "no 'authored against"

# 8. A missing VERSION file is an error, not a silent pass.
clean_tree
rm -f "$ROOT/VERSION"
out=$(run_check) && rc=0 || rc=$?
assert_eq "missing VERSION fails" "1" "$rc"
assert_contains "missing VERSION named" "$out" "VERSION file not found"

# 9. A missing Cargo.toml is an error.
clean_tree
out=$("$CHECK" "$WORK/nosuch.toml" "$ROOT" 2>&1) && rc=0 || rc=$?
assert_eq "missing Cargo.toml fails" "1" "$rc"
assert_contains "missing Cargo.toml named" "$out" "Cargo.toml not found"

# 10. A Cargo.toml with no [package] version is an error rather than a pass on
#     an empty comparison against every site.
clean_tree
printf '[dependencies]\nreqwest = { version = "9.9.9" }\n' >"$WORK/nopkg.toml"
out=$("$CHECK" "$WORK/nopkg.toml" "$ROOT" 2>&1) && rc=0 || rc=$?
assert_eq "no [package] version fails" "1" "$rc"

report "version-check-test"
