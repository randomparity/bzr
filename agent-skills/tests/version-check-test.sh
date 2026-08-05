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
PAYLOAD="$WORK/content/skills"
CARGO="$WORK/Cargo.toml"

# Other tables carry their own `version =`, so the parser must be bounded by
# [package]. One of them is placed *before* [package]: with both decoys after
# it, a naive "first version = anywhere" parser would still pass this fixture.
cat >"$CARGO" <<'EOF'
[workspace.package]
version = "9.9.9"

[package]
name = "bzr"
version = "1.2.3-dev"
edition = "2021"

[dependencies]
reqwest = { version = "8.8.8" }
EOF

# A tree where all five sites agree with the crate version. The README claim is
# split across a line break, matching the real file's wrap.
clean_tree() {
  rm -rf "$ROOT"
  rm -rf "$PAYLOAD"
  mkdir -p "$ROOT" "$PAYLOAD/bzr-reference/reference"
  printf '1.2.3-dev\n' >"$ROOT/VERSION"
  cat >"$ROOT/README.md" <<'EOF'
# Agent Skills

The command surface is authored
against `bzr` 1.2.3-dev. The installer copies whole skill folders.
EOF
  cat >"$PAYLOAD/bzr-reference/SKILL.md" <<'EOF'
This reference is authored against **bzr 1.2.3-dev**. Check `bzr --help`.
EOF
  cat >"$PAYLOAD/bzr-reference/reference/commands.md" <<'EOF'
# bzr command surface (authored against bzr 1.2.3-dev)
EOF
  cat >"$PAYLOAD/bzr-reference/reference/commands.yml" <<'EOF'
# bzr command manifest — authored against bzr 1.2.3-dev.
bug: list view
EOF
}

run_check() {
  "$CHECK" "$CARGO" "$ROOT" "$PAYLOAD" 2>&1
}

# 1. Every site agrees -> exit 0. This also proves the [package] version wins
#    over the decoys in [workspace.package] and [dependencies].
clean_tree
out=$(run_check) && rc=0 || rc=$?
assert_eq "agreeing tree exits 0" "0" "$rc"
assert_not_contains "decoy [workspace.package] version not used" "$out" "9.9.9"

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
cat >"$PAYLOAD/bzr-reference/reference/commands.md" <<'EOF'
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
cat >"$PAYLOAD/bzr-reference/reference/commands.md" <<'EOF'
# bzr command surface (authored against bzr 0.7.0)
EOF
cat >"$PAYLOAD/bzr-reference/reference/commands.yml" <<'EOF'
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
mkdir -p "$PAYLOAD/bzr-setup"
cat >"$PAYLOAD/bzr-setup/SKILL.md" <<'EOF'
This skill is authored against bzr 0.6.1-dev.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "stale claim in another skill fails" "1" "$rc"
assert_contains "other skill named" "$out" "bzr-setup"

# 6b. The same claim at the start of a sentence is the same claim. A
#     case-sensitive scan misses it silently -- and only outside the required
#     sites, where "no claim" would otherwise have caught it.
clean_tree
mkdir -p "$PAYLOAD/bzr-setup"
cat >"$PAYLOAD/bzr-setup/SKILL.md" <<'EOF'
Authored against `bzr` 0.6.1-dev. Run `bzr --version` to confirm.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "capitalized stale claim fails" "1" "$rc"
assert_contains "capitalized claim names the version" "$out" "0.6.1-dev"

# 6c. Wordier phrasings around the `bzr` token are still claims.
clean_tree
mkdir -p "$PAYLOAD/bzr-setup"
cat >"$PAYLOAD/bzr-setup/SKILL.md" <<'EOF'
These recipes are authored against the current `bzr` 0.6.1-dev surface, and
the flags below are authored against bzr version 0.7.0.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "wordy stale claims fail" "1" "$rc"
assert_contains "wordy claim before the token reported" "$out" "0.6.1-dev"
assert_contains "wordy claim after the token reported" "$out" "0.7.0"

# 7. Dropping the claim from ONE required site fails. A floor of "at least one
#    claim somewhere" would report this tree clean, which is the whole point of
#    naming the required sites.
clean_tree
printf 'bug: list view\n' >"$PAYLOAD/bzr-reference/reference/commands.yml"
out=$(run_check) && rc=0 || rc=$?
assert_eq "one required site without a claim fails" "1" "$rc"
assert_contains "missing site named" "$out" "commands.yml"

# 7b. ...and with every claim gone, all four are reported, not just the first.
clean_tree
printf '# Agent Skills\n' >"$ROOT/README.md"
printf 'no claim here\n' >"$PAYLOAD/bzr-reference/SKILL.md"
printf 'no claim here\n' >"$PAYLOAD/bzr-reference/reference/commands.md"
printf 'bug: list view\n' >"$PAYLOAD/bzr-reference/reference/commands.yml"
out=$(run_check) && rc=0 || rc=$?
assert_eq "zero claims fails" "1" "$rc"
assert_contains "missing README reported" "$out" "README.md"
assert_contains "missing SKILL.md reported" "$out" "SKILL.md"
assert_contains "missing commands.md reported" "$out" "commands.md"
assert_contains "missing commands.yml reported" "$out" "commands.yml"

# 7c. The gap between "against" and the version is bounded, so unrelated prose
#     cannot latch a version literal onto an "authored against" phrase far away.
#     An unbounded gap flags this tree, and flags this repo's own README.
clean_tree
mkdir -p "$PAYLOAD/bzr-x"
cat >"$PAYLOAD/bzr-x/SKILL.md" <<'EOF'
These recipes are authored against the current CLI surface; run `bzr --help`
to confirm. Requires a Bugzilla server of version 5.0.0 or newer.
EOF
out=$(run_check) && rc=0 || rc=$?
assert_eq "distant version literal is not a claim" "0" "$rc"
assert_not_contains "distant literal not reported" "$out" "5.0.0"

# 8. A missing VERSION file is an error, not a silent pass.
clean_tree
rm -f "$ROOT/VERSION"
out=$(run_check) && rc=0 || rc=$?
assert_eq "missing VERSION fails" "1" "$rc"
assert_contains "missing VERSION named" "$out" "VERSION file not found"

# 9. A missing Cargo.toml is an error.
clean_tree
out=$("$CHECK" "$WORK/nosuch.toml" "$ROOT" "$PAYLOAD" 2>&1) && rc=0 || rc=$?
assert_eq "missing Cargo.toml fails" "1" "$rc"
assert_contains "missing Cargo.toml named" "$out" "Cargo.toml not found"

# 10. A Cargo.toml with no [package] version is an error rather than a pass on
#     an empty comparison against every site.
clean_tree
printf '[dependencies]\nreqwest = { version = "9.9.9" }\n' >"$WORK/nopkg.toml"
out=$("$CHECK" "$WORK/nopkg.toml" "$ROOT" "$PAYLOAD" 2>&1) && rc=0 || rc=$?
assert_eq "no [package] version fails" "1" "$rc"
# Assert the guard is what fired. Without this the test passes either way: an
# empty crate version makes every site mismatch, which also exits 1.
assert_contains "no [package] version explained" "$out" "no [package] version in"

report "version-check-test"
