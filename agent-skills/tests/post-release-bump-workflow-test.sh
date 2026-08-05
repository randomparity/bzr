#!/bin/sh
set -eu
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck disable=SC1091  # lib.sh path is dynamic; resolved at runtime
. "$HERE/lib.sh"
# shellcheck disable=SC1007  # CDPATH= is a per-command env var, not an assignment
ROOT=$(CDPATH= cd -- "$HERE/../.." && pwd)
WORKFLOW="$ROOT/.github/workflows/release.yml"
VERSION_CHECK="$HERE/version-check.sh"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

extract_step() {
  awk -v name="$1" '
    $0 == "      - name: " name { found = 1; next }
    found && $0 == "        run: |" { in_run = 1; next }
    in_run && /^      - / { exit }
    in_run { sub(/^          /, ""); print }
    END { if (!in_run) exit 1 }
  ' "$WORKFLOW"
}

make_fixture() {
  fixture=$1
  mkdir -p "$fixture/agent-skills/skills/bzr-reference/reference"
  cat >"$fixture/Cargo.toml" <<'EOF'
[package]
name = "fixture"
version = "9.8.7-dev"
EOF
  printf '%s\n' '0.8.2-dev' >"$fixture/agent-skills/VERSION"
  # shellcheck disable=SC2016  # Literal backticks document the authored-against claim.
  printf '%s\n' 'The surface is authored against `bzr` 0.8.2-dev.' \
    >"$fixture/agent-skills/README.md"
  printf '%s\n' 'This reference is authored against **bzr 0.8.2-dev**.' \
    >"$fixture/agent-skills/skills/bzr-reference/SKILL.md"
  printf '%s\n' '# commands (authored against bzr 0.8.2-dev)' \
    >"$fixture/agent-skills/skills/bzr-reference/reference/commands.md"
  printf '%s\n' '# manifest — authored against bzr 0.8.2-dev.' \
    >"$fixture/agent-skills/skills/bzr-reference/reference/commands.yml"
}

UPDATE="$WORK/update.sh"
if extract_step 'Update bundled-skill versions' >"$UPDATE"; then
  update_rc=0
else
  update_rc=$?
fi
assert_eq "update step exists" "0" "$update_rc"

if [ "$update_rc" -eq 0 ]; then
  chmod +x "$UPDATE"

  normal="$WORK/normal"
  make_fixture "$normal"
  out=$(cd "$normal" && NEXT=9.8.7-dev bash "$UPDATE" 2>&1) && rc=0 || rc=$?
  assert_eq "normal update exits 0" "0" "$rc"
  assert_eq "VERSION updated" "9.8.7-dev" "$(cat "$normal/agent-skills/VERSION")"
  claim_count=$(grep -E -i -l 'authored against.*bzr.*9\.8\.7-dev' \
    "$normal/agent-skills/README.md" \
    "$normal/agent-skills/skills/bzr-reference/SKILL.md" \
    "$normal/agent-skills/skills/bzr-reference/reference/commands.md" \
    "$normal/agent-skills/skills/bzr-reference/reference/commands.yml" | wc -l | tr -d ' ')
  assert_eq "all four claims updated" "4" "$claim_count"
  out=$("$VERSION_CHECK" "$normal/Cargo.toml" "$normal/agent-skills" 2>&1) && rc=0 || rc=$?
  assert_eq "updated fixture satisfies version contract" "0" "$rc"

  missing="$WORK/missing"
  make_fixture "$missing"
  printf '%s\n' '# no version claim' \
    >"$missing/agent-skills/skills/bzr-reference/reference/commands.md"
  out=$(cd "$missing" && NEXT=9.8.7-dev bash "$UPDATE" 2>&1) && rc=0 || rc=$?
  assert_eq "missing claim fails" "1" "$rc"
  assert_contains "missing claim names file" "$out" "commands.md"

  duplicate="$WORK/duplicate"
  make_fixture "$duplicate"
  printf '%s\n' '# second claim authored against bzr 0.8.2-dev' \
    >>"$duplicate/agent-skills/skills/bzr-reference/reference/commands.yml"
  out=$(cd "$duplicate" && NEXT=9.8.7-dev bash "$UPDATE" 2>&1) && rc=0 || rc=$?
  assert_eq "duplicate claim fails" "1" "$rc"
  assert_contains "duplicate claim names file" "$out" "commands.yml"
fi

commit_step=$(extract_step 'Commit and push branch')
for path in \
  agent-skills/VERSION \
  agent-skills/README.md \
  agent-skills/skills/bzr-reference/SKILL.md \
  agent-skills/skills/bzr-reference/reference/commands.md \
  agent-skills/skills/bzr-reference/reference/commands.yml; do
  assert_contains "commit stages $path" "$commit_step" "$path"
done

update_line=$(grep -nF -- '- name: Update bundled-skill versions' "$WORKFLOW" | cut -d: -f1)
verify_line=$(grep -nF -- '- name: Verify bundled-skill version contract' "$WORKFLOW" | cut -d: -f1)
commit_line=$(grep -nF -- '- name: Commit and push branch' "$WORKFLOW" | cut -d: -f1)
if [ -n "$update_line" ] && [ -n "$verify_line" ] &&
  [ "$update_line" -lt "$verify_line" ] && [ "$verify_line" -lt "$commit_line" ]; then
  _pass "version contract is verified before commit and push"
else
  _fail "version contract is verified before commit and push" \
    "step lines: update=$update_line verify=$verify_line commit=$commit_line"
fi

report "post-release-bump-workflow-test"
