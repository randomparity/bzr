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

# The installer replaces every skill in the embedded payload, so this list must
# track content/skills/ exactly (build.rs embeds it sorted by relative path,
# which is also the emitted order of `.destinations[*].installed`; that order
# equals plain lexical name order only while no skill name is a hyphen-prefix
# of another). Drift trips src/skills/embedded_tests.rs
# (embeds_all_current_skills_in_lexical_order) under make test-fast first,
# then fails here on count or fixture mismatch — update all three lists
# together.
SKILLS_EXPECTED=(
  bzr-bulk-triage
  bzr-dependency-analysis
  bzr-dry-run-confirm
  bzr-file-bug
  bzr-project-manager-reporting
  bzr-reference
  bzr-release-readiness
  bzr-release-tracking
  bzr-search-report
  bzr-setup
  bzr-triage-bug
  bzr-weekly-status
)

DEPENDENCY_ANALYSIS_PAYLOAD=(
  SKILL.md
  scripts/analyze.py
  scripts/collect.py
  scripts/render.py
  tests/fixtures/alias-collapse.expected.json
  tests/fixtures/alias-collapse.policy.json
  tests/fixtures/branch.analysis.json
  tests/fixtures/branch.collection.json
  tests/fixtures/cross-server.analysis.json
  tests/fixtures/cross-server.collection.json
  tests/fixtures/cycle.analysis.json
  tests/fixtures/cycle.collection.json
  tests/fixtures/diamond.analysis.json
  tests/fixtures/diamond.collection.json
  tests/fixtures/empty-partial.analysis.json
  tests/fixtures/empty-partial.collection.json
  tests/fixtures/hostile.analysis.json
  tests/fixtures/hostile.expected.md
  tests/fixtures/hostile.expected.mmd
  tests/fixtures/inaccessible.analysis.json
  tests/fixtures/inaccessible.collection.json
  tests/fixtures/missing.analysis.json
  tests/fixtures/missing.collection.json
  tests/fixtures/recording_runner.py
  tests/fixtures/resolved.analysis.json
  tests/fixtures/resolved.collection.json
  tests/fixtures/stale.analysis.json
  tests/fixtures/stale.collection.json
  tests/skill-contract.sh
  tests/test_analyze.py
  tests/test_collect.py
  tests/test_render.py
)

RELEASE_READINESS_PAYLOAD=(
  SKILL.md
  reference/eval-cases.md
  reference/report-template.md
  tests/fixtures/release-bugs.json
  tests/fixtures/release-report.expected.md
  tests/run.sh
)

PROJECT_MANAGER_REPORTING_PAYLOAD=(
  SKILL.md
  assets/demo-prompt.txt
  assets/demo-report.md
  reference/artifact-safety.md
  reference/report-template.md
  tests/run.sh
)

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

test_begin "123i. dependency-analysis installation contains the complete payload"
_DA_INSTALLED_ROOT="$SKILLS_PROJECT/.agents/skills/bzr-dependency-analysis"
_DA_INSTALLED_EXPECTED="$FUNC_CONFIG_DIR/dependency-analysis-installed-expected.txt"
printf '%s\n' "${DEPENDENCY_ANALYSIS_PAYLOAD[@]}" | LC_ALL=C sort \
  >"$_DA_INSTALLED_EXPECTED"
_DA_INSTALLED_OK=1
for _DA_LAYOUT in .agents .claude; do
  _DA_LAYOUT_ROOT="$SKILLS_PROJECT/$_DA_LAYOUT/skills/bzr-dependency-analysis"
  _DA_INSTALLED_PATHS="$FUNC_CONFIG_DIR/dependency-analysis-${_DA_LAYOUT#*.}-paths.txt"
  (
    cd "$_DA_LAYOUT_ROOT" || exit 1
    find . -type f ! -name .bzr-skill-managed -print |
      sed 's#^\./##' | LC_ALL=C sort
  ) >"$_DA_INSTALLED_PATHS"
  cmp -s "$_DA_INSTALLED_EXPECTED" "$_DA_INSTALLED_PATHS" || _DA_INSTALLED_OK=0
done
if [[ $_DA_INSTALLED_OK -eq 1 ]]; then
  test_pass
else
  test_fail "installed dependency-analysis payload did not match the embedded contract"
fi

test_begin "123k. release-readiness installation contains the complete payload"
_RR_INSTALLED_EXPECTED="$FUNC_CONFIG_DIR/release-readiness-installed-expected.txt"
printf '%s\n' "${RELEASE_READINESS_PAYLOAD[@]}" | LC_ALL=C sort \
  >"$_RR_INSTALLED_EXPECTED"
_RR_INSTALLED_OK=1
for _RR_LAYOUT in .agents .claude; do
  _RR_LAYOUT_ROOT="$SKILLS_PROJECT/$_RR_LAYOUT/skills/bzr-release-readiness"
  _RR_INSTALLED_PATHS="$FUNC_CONFIG_DIR/release-readiness-${_RR_LAYOUT#*.}-paths.txt"
  (
    cd "$_RR_LAYOUT_ROOT" || exit 1
    find . -type f ! -name .bzr-skill-managed -print |
      sed 's#^\./##' | LC_ALL=C sort
  ) >"$_RR_INSTALLED_PATHS"
  cmp -s "$_RR_INSTALLED_EXPECTED" "$_RR_INSTALLED_PATHS" || _RR_INSTALLED_OK=0
done
if [[ $_RR_INSTALLED_OK -eq 1 ]]; then
  test_pass
else
  test_fail "installed release-readiness payload did not match the embedded contract"
fi

test_begin "123l. project-manager reporting installation contains the complete payload"
_PM_INSTALLED_EXPECTED="$FUNC_CONFIG_DIR/project-manager-reporting-installed-expected.txt"
printf '%s\n' "${PROJECT_MANAGER_REPORTING_PAYLOAD[@]}" | LC_ALL=C sort \
  >"$_PM_INSTALLED_EXPECTED"
_PM_INSTALLED_OK=1
for _PM_LAYOUT in .agents .claude; do
  _PM_LAYOUT_ROOT="$SKILLS_PROJECT/$_PM_LAYOUT/skills/bzr-project-manager-reporting"
  _PM_INSTALLED_PATHS="$FUNC_CONFIG_DIR/project-manager-reporting-${_PM_LAYOUT#*.}-paths.txt"
  (
    cd "$_PM_LAYOUT_ROOT" || exit 1
    find . -type f ! -name .bzr-skill-managed -print |
      sed 's#^\./##' | LC_ALL=C sort
  ) >"$_PM_INSTALLED_PATHS"
  cmp -s "$_PM_INSTALLED_EXPECTED" "$_PM_INSTALLED_PATHS" || _PM_INSTALLED_OK=0
done
if [[ $_PM_INSTALLED_OK -eq 1 ]]; then
  test_pass
else
  test_fail "installed project-manager reporting payload did not match the embedded contract"
fi

test_begin "123b. skills install idempotently replaces an owned skill"
printf '\nlocal stale marker\n' >>"$SKILLS_PROJECT/.agents/skills/bzr-reference/SKILL.md"
run_bzr skills install --agent all --project "$SKILLS_PROJECT"
if assert_success &&
  ! grep -q "local stale marker" \
    "$SKILLS_PROJECT/.agents/skills/bzr-reference/SKILL.md" &&
  [[ $(jq -r '.destinations[0].installed | length' "$BZR_STDOUT") == ${#SKILLS_EXPECTED[@]} ]] &&
  [[ $(jq -r '.destinations[1].installed | length' "$BZR_STDOUT") == ${#SKILLS_EXPECTED[@]} ]]; then
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
jq -n --arg project "$SKILLS_NDJSON_CANONICAL" --args '
  {
    action: "install",
    agent: "all",
    scope: "project",
    project: $project,
    destinations: [
      {
        layout: "agents",
        path: ($project + "/.agents/skills"),
        installed: $ARGS.positional
      },
      {
        layout: "claude",
        path: ($project + "/.claude/skills"),
        installed: $ARGS.positional
      }
    ]
  }
' "${SKILLS_EXPECTED[@]}" >"$SKILLS_NDJSON_EXPECTED"
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

test_begin "123j. installed cycle fixture analyzes and renders deterministically"
_DA_CYCLE_COLLECTION="$_DA_INSTALLED_ROOT/tests/fixtures/cycle.collection.json"
_DA_CYCLE_EXPECTED="$_DA_INSTALLED_ROOT/tests/fixtures/cycle.analysis.json"
_DA_CYCLE_ANALYSIS="$FUNC_CONFIG_DIR/dependency-cycle.analysis.json"
_DA_CYCLE_REPORT="$FUNC_CONFIG_DIR/dependency-cycle.md"
_DA_CYCLE_DIAGRAM="$FUNC_CONFIG_DIR/dependency-cycle.mmd"
if python3 "$_DA_INSTALLED_ROOT/scripts/analyze.py" \
  --input "$_DA_CYCLE_COLLECTION" --allow-partial \
  --output "$_DA_CYCLE_ANALYSIS" &&
  cmp -s "$_DA_CYCLE_EXPECTED" "$_DA_CYCLE_ANALYSIS" &&
  python3 "$_DA_INSTALLED_ROOT/scripts/render.py" \
    --input "$_DA_CYCLE_ANALYSIS" --format markdown \
    --output "$_DA_CYCLE_REPORT" &&
  python3 "$_DA_INSTALLED_ROOT/scripts/render.py" \
    --input "$_DA_CYCLE_ANALYSIS" --format mermaid \
    --output "$_DA_CYCLE_DIAGRAM" &&
  jq -e '.components | any(.cyclic == true)' "$_DA_CYCLE_ANALYSIS" >/dev/null &&
  grep -q 'Cycle impediments: c0001' "$_DA_CYCLE_REPORT" &&
  grep -q 'cyclic=true' "$_DA_CYCLE_DIAGRAM"; then
  test_pass
else
  test_fail "installed cycle fixture pipeline did not preserve cycle evidence"
fi

test_begin "123ja. installed collector replay feeds installed analyzer and renderers"
_DA_REPLAY_POLICY="$_DA_INSTALLED_ROOT/tests/fixtures/alias-collapse.policy.json"
_DA_REPLAY_EXPECTED="$_DA_INSTALLED_ROOT/tests/fixtures/alias-collapse.expected.json"
_DA_REPLAY_RUNNER="$_DA_INSTALLED_ROOT/tests/fixtures/recording_runner.py"
_DA_REPLAY_SCENARIO="$FUNC_CONFIG_DIR/dependency-replay.scenario.json"
_DA_REPLAY_LOG="$FUNC_CONFIG_DIR/dependency-replay.commands.ndjson"
_DA_REPLAY_COLLECTION="$FUNC_CONFIG_DIR/dependency-replay.collection.json"
_DA_REPLAY_ANALYSIS="$FUNC_CONFIG_DIR/dependency-replay.analysis.json"
_DA_REPLAY_REPORT="$FUNC_CONFIG_DIR/dependency-replay.md"
_DA_REPLAY_DIAGRAM="$FUNC_CONFIG_DIR/dependency-replay.mmd"
jq -n '{
  responses: [{
    argv: [
      "--server", "primary", "--json", "bug", "list",
      "--limit", "1", "--offset", "0", "--fields", "id",
      "--sort", "bug_id", "--order", "asc"
    ],
    exit_code: 0,
    stdout: {schema_version: "0.6.2", data: []}
  }, {
    argv: [
      "--server", "primary", "--json", "bug", "view", "delivery",
      "--fields",
      "id,summary,status,resolution,assigned_to,last_change_time,blocks,depends_on"
    ],
    exit_code: 0,
    stdout: {
      schema_version: "0.6.2",
      data: {
        assigned_to: null,
        blocks: [],
        depends_on: [11],
        id: 10,
        last_change_time: "2026-08-27T12:00:00Z",
        resolution: null,
        status: "NEW",
        summary: "Delivery"
      }
    }
  }]
}' >"$_DA_REPLAY_SCENARIO"
_DA_REPLAY_OK=1
chmod u+x "$_DA_REPLAY_RUNNER" || _DA_REPLAY_OK=0
BZR_DEPENDENCY_RUNNER_SCENARIO="$_DA_REPLAY_SCENARIO" \
  BZR_DEPENDENCY_RUNNER_LOG="$_DA_REPLAY_LOG" \
  python3 "$_DA_INSTALLED_ROOT/scripts/collect.py" \
  --policy "$_DA_REPLAY_POLICY" --output "$_DA_REPLAY_COLLECTION" \
  --runner "$_DA_REPLAY_RUNNER" \
  --analysis-timestamp "2026-08-28T12:00:00Z" || _DA_REPLAY_OK=0
python3 "$_DA_INSTALLED_ROOT/scripts/analyze.py" \
  --input "$_DA_REPLAY_COLLECTION" --allow-partial \
  --output "$_DA_REPLAY_ANALYSIS" || _DA_REPLAY_OK=0
python3 "$_DA_INSTALLED_ROOT/scripts/render.py" \
  --input "$_DA_REPLAY_ANALYSIS" --format markdown \
  --output "$_DA_REPLAY_REPORT" || _DA_REPLAY_OK=0
python3 "$_DA_INSTALLED_ROOT/scripts/render.py" \
  --input "$_DA_REPLAY_ANALYSIS" --format mermaid \
  --output "$_DA_REPLAY_DIAGRAM" || _DA_REPLAY_OK=0
if [[ $_DA_REPLAY_OK -eq 1 ]] &&
  cmp -s "$_DA_REPLAY_EXPECTED" "$_DA_REPLAY_COLLECTION" &&
  jq -s -e 'length == 2 and .[0] == [
      "--server", "primary", "--json", "bug", "list",
      "--limit", "1", "--offset", "0", "--fields", "id",
      "--sort", "bug_id", "--order", "asc"
    ] and .[1] == [
      "--server", "primary", "--json", "bug", "view", "delivery",
      "--fields",
      "id,summary,status,resolution,assigned_to,last_change_time,blocks,depends_on"
    ]' "$_DA_REPLAY_LOG" >/dev/null &&
  jq -e '
      .status == "partial" and .edges == [] and (.nodes | length) == 1 and
      .nodes[0].id == 10 and .nodes[0].server == "primary" and
      .nodes[0].requested_aliases == ["delivery"]
    ' "$_DA_REPLAY_ANALYSIS" >/dev/null &&
  grep -q 'primary#10' "$_DA_REPLAY_REPORT" &&
  grep -q 'primary&#35;10' "$_DA_REPLAY_DIAGRAM"; then
  test_pass
else
  test_fail "installed collect-to-render replay did not match its fixture oracle"
fi

unset DEPENDENCY_ANALYSIS_PAYLOAD _DA_CYCLE_ANALYSIS _DA_CYCLE_COLLECTION
unset _DA_CYCLE_DIAGRAM _DA_CYCLE_EXPECTED _DA_CYCLE_REPORT _DA_INSTALLED_EXPECTED
unset _DA_INSTALLED_OK _DA_INSTALLED_PATHS _DA_INSTALLED_ROOT _DA_LAYOUT _DA_LAYOUT_ROOT
unset _DA_REPLAY_ANALYSIS _DA_REPLAY_COLLECTION _DA_REPLAY_DIAGRAM
unset _DA_REPLAY_EXPECTED _DA_REPLAY_LOG _DA_REPLAY_OK _DA_REPLAY_POLICY
unset _DA_REPLAY_REPORT _DA_REPLAY_RUNNER _DA_REPLAY_SCENARIO
unset RELEASE_READINESS_PAYLOAD _RR_INSTALLED_EXPECTED _RR_INSTALLED_OK
unset _RR_INSTALLED_PATHS _RR_LAYOUT _RR_LAYOUT_ROOT

echo ""
