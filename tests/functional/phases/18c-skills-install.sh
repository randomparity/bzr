# 18c-skills-install
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble. Exercises the offline bundled-skill installer.
# shellcheck shell=bash

echo "── Phase 18c: Bundled skill installation ────────────────"

# Every fixture root in this phase lives under FUNC_CONFIG_DIR, which the
# orchestrator registered with its EXIT cleanup before sourcing this file.
SKILLS_PROJECT="$FUNC_CONFIG_DIR/skills-project"
mkdir "$SKILLS_PROJECT"
SKILLS_PROJECT_CANONICAL=$(cd "$SKILLS_PROJECT" && pwd -P)

test_begin "123a. skills install populates both project layouts"
run_bzr skills install --agent all --project "$SKILLS_PROJECT"
if assert_success &&
  assert_json '.action' "install" &&
  assert_json '.agent' "all" &&
  assert_json '.scope' "project" &&
  assert_json '.project' "$SKILLS_PROJECT_CANONICAL" &&
  assert_json '.destinations | length' "2" &&
  assert_json '.destinations[0].layout' "agents" &&
  assert_json '.destinations[1].layout' "claude" &&
  [[ -f "$SKILLS_PROJECT/.agents/skills/bzr-reference/reference/commands.md" ]] &&
  [[ -f "$SKILLS_PROJECT/.claude/skills/bzr-reference/reference/commands.md" ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "nested bundled files were not installed"
fi

test_begin "123b. skills install idempotently replaces an owned skill"
printf '\nlocal stale marker\n' >>"$SKILLS_PROJECT/.agents/skills/bzr-reference/SKILL.md"
run_bzr skills install --agent all --project "$SKILLS_PROJECT"
if assert_success &&
  ! grep -q "local stale marker" \
    "$SKILLS_PROJECT/.agents/skills/bzr-reference/SKILL.md" &&
  [[ $(jq -r '.destinations[0].installed | length' "$BZR_STDOUT") == 6 ]] &&
  [[ $(jq -r '.destinations[1].installed | length' "$BZR_STDOUT") == 6 ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "owned skill was not replaced from embedded payload"
fi

SKILLS_HOME="$FUNC_CONFIG_DIR/skills-home"
mkdir "$SKILLS_HOME"
SKILLS_HOME_CANONICAL=$(cd "$SKILLS_HOME" && pwd -P)
ORIGINAL_HOME=$HOME
export HOME="$SKILLS_HOME"
test_begin "123c. skills install populates isolated global layouts"
run_bzr skills install --agent all --global
export HOME="$ORIGINAL_HOME"
if assert_success &&
  assert_json '.scope' "global" &&
  assert_json '.project == null' "true" &&
  assert_json '.destinations[0].path' "$SKILLS_HOME_CANONICAL/.agents/skills" &&
  assert_json '.destinations[1].path' "$SKILLS_HOME_CANONICAL/.claude/skills" &&
  [[ -f "$SKILLS_HOME/.agents/skills/bzr-reference/reference/commands.md" ]] &&
  [[ -f "$SKILLS_HOME/.claude/skills/bzr-reference/reference/commands.md" ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "global nested bundled files were not installed"
fi

SKILLS_NDJSON="$FUNC_CONFIG_DIR/skills-ndjson"
SKILLS_NDJSON_EXPECTED="$FUNC_CONFIG_DIR/skills-ndjson-expected.json"
mkdir "$SKILLS_NDJSON"
SKILLS_NDJSON_CANONICAL=$(cd "$SKILLS_NDJSON" && pwd -P)
jq -n --arg project "$SKILLS_NDJSON_CANONICAL" '
  {
    action: "install",
    agent: "all",
    scope: "project",
    project: $project,
    destinations: [
      {
        layout: "agents",
        path: ($project + "/.agents/skills"),
        installed: [
          "bzr-bulk-triage",
          "bzr-file-bug",
          "bzr-reference",
          "bzr-search-report",
          "bzr-setup",
          "bzr-triage-bug"
        ]
      },
      {
        layout: "claude",
        path: ($project + "/.claude/skills"),
        installed: [
          "bzr-bulk-triage",
          "bzr-file-bug",
          "bzr-reference",
          "bzr-search-report",
          "bzr-setup",
          "bzr-triage-bug"
        ]
      }
    ]
  }
' >"$SKILLS_NDJSON_EXPECTED"
test_begin "123d. skills install emits one bare, complete NDJSON object"
run_bzr_raw --output ndjson skills install --agent all --project "$SKILLS_NDJSON"
if assert_success &&
  assert_ndjson_line_count 1 &&
  jq -e --slurpfile expected "$SKILLS_NDJSON_EXPECTED" \
    '. == $expected[0]' "$BZR_STDOUT" >/dev/null; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "NDJSON skill result did not match the contract"
fi

SKILLS_FOREIGN="$FUNC_CONFIG_DIR/skills-foreign"
SKILLS_FOREIGN_BEFORE="$FUNC_CONFIG_DIR/skills-foreign-before"
mkdir -p "$SKILLS_FOREIGN/.agents/skills/bzr-reference"
printf 'foreign bytes\n' >"$SKILLS_FOREIGN/.agents/skills/bzr-reference/keep.txt"
cat >"$SKILLS_FOREIGN/.agents/skills/bzr-reference/.bzr-skill-managed" <<'EOF'
managed-by: somebody-else
installed-skill: bzr-reference
source-version: foreign
source-commit: foreign
EOF
cp -R "$SKILLS_FOREIGN/.agents" "$SKILLS_FOREIGN_BEFORE"
test_begin "123e. skills install refuses and preserves a foreign skill"
run_bzr skills install --agent codex --project "$SKILLS_FOREIGN"
if assert_failure &&
  [[ ! -s "$BZR_STDOUT_RAW" ]] &&
  diff -r "$SKILLS_FOREIGN_BEFORE" "$SKILLS_FOREIGN/.agents" >/dev/null; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "foreign skill changed during refusal"
fi

SKILLS_SYMLINK="$FUNC_CONFIG_DIR/skills-symlink"
SKILLS_SYMLINK_TARGET="$FUNC_CONFIG_DIR/skills-symlink-target"
mkdir "$SKILLS_SYMLINK" "$SKILLS_SYMLINK_TARGET"
ln -s "$SKILLS_SYMLINK_TARGET" "$SKILLS_SYMLINK/.agents"
test_begin "123f. skills install refuses a symlinked destination component"
run_bzr skills install --agent codex --project "$SKILLS_SYMLINK"
if assert_failure &&
  [[ ! -s "$BZR_STDOUT_RAW" ]] &&
  [[ -L "$SKILLS_SYMLINK/.agents" ]] &&
  [[ -z $(ls -A "$SKILLS_SYMLINK_TARGET") ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "symlink destination was followed or modified"
fi

SKILLS_NO_CONFIG="$FUNC_CONFIG_DIR/skills-no-config"
SKILLS_BAD_CONFIG="$FUNC_CONFIG_DIR/skills-malformed-config.toml"
SKILLS_BAD_CONFIG_BEFORE="$FUNC_CONFIG_DIR/skills-malformed-config-before.toml"
mkdir "$SKILLS_NO_CONFIG"
printf 'not = [valid toml\n' >"$SKILLS_BAD_CONFIG"
cp "$SKILLS_BAD_CONFIG" "$SKILLS_BAD_CONFIG_BEFORE"
test_begin "123g. skills install ignores and preserves malformed Bugzilla config"
run_bzr --config "$SKILLS_BAD_CONFIG" skills install \
  --agent standard --project "$SKILLS_NO_CONFIG"
if assert_success &&
  cmp -s "$SKILLS_BAD_CONFIG_BEFORE" "$SKILLS_BAD_CONFIG" &&
  [[ -f "$SKILLS_NO_CONFIG/.agents/skills/bzr-reference/SKILL.md" ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "local install consulted or changed Bugzilla config"
fi

test_begin "123h. skills install refuses an omitted scope without stdout"
run_bzr skills install --agent all
if assert_exit_code 7 && [[ ! -s "$BZR_STDOUT_RAW" ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "missing scope emitted stdout"
fi

echo ""
