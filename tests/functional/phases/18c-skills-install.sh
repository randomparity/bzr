# 18c-skills-install
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble. Exercises the offline bundled-skill installer.
# shellcheck shell=bash

echo "── Phase 18c: Bundled skill installation ────────────────"

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

SKILLS_HOME="$FUNC_CONFIG_DIR/skills-home"
mkdir "$SKILLS_HOME"
ORIGINAL_HOME=$HOME
export HOME="$SKILLS_HOME"
test_begin "123b. skills install populates isolated global layouts"
run_bzr skills install --agent all --global
export HOME="$ORIGINAL_HOME"
if assert_success &&
  assert_json '.scope' "global" &&
  assert_json '.project == null' "true" &&
  assert_json '.destinations[0].path' "$SKILLS_HOME/.agents/skills" &&
  assert_json '.destinations[1].path' "$SKILLS_HOME/.claude/skills" &&
  [[ -f "$SKILLS_HOME/.agents/skills/bzr-reference/reference/commands.md" ]] &&
  [[ -f "$SKILLS_HOME/.claude/skills/bzr-reference/reference/commands.md" ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "global nested bundled files were not installed"
fi

test_begin "123c. skills install refuses an omitted scope without stdout"
run_bzr skills install --agent all
if assert_exit_code 7 && [[ ! -s "$BZR_STDOUT_RAW" ]]; then
  test_pass
else
  [[ $FAIL_COUNT -gt 0 ]] || test_fail "missing scope emitted stdout"
fi

echo ""
