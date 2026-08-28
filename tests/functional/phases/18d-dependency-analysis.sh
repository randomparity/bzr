# 18d-dependency-analysis
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble plus SKILLS_PROJECT from phase 18c.
# shellcheck shell=bash

echo "── Phase 18d: Installed dependency analysis ──────────────"

_DA_SKILL_ROOT="$SKILLS_PROJECT/.agents/skills/bzr-dependency-analysis"
_DA_SKILL_ROOT_CANONICAL=$(cd "$_DA_SKILL_ROOT" && pwd -P)
_DA_COLLECT="$_DA_SKILL_ROOT/scripts/collect.py"
_DA_ANALYZE="$_DA_SKILL_ROOT/scripts/analyze.py"
_DA_RENDER="$_DA_SKILL_ROOT/scripts/render.py"
_DA_CYCLE="$_DA_SKILL_ROOT/tests/fixtures/cycle.collection.json"
_DA_PATHS_OK=1
for _DA_PATH in "$_DA_COLLECT" "$_DA_ANALYZE" "$_DA_RENDER" "$_DA_CYCLE"; do
  _DA_PATH_CANONICAL=$(cd "$(dirname "$_DA_PATH")" && pwd -P)/$(basename "$_DA_PATH")
  case "$_DA_PATH_CANONICAL" in
  "$_DA_SKILL_ROOT_CANONICAL"/*) ;;
  *) _DA_PATHS_OK=0 ;;
  esac
done
_DA_BZR_CANONICAL=$(cd "$(dirname "$BZR_BIN")" && pwd -P)/$(basename "$BZR_BIN")

test_begin "123k. live pipeline resolves only installed helpers and release bzr"
if [[ $_DA_PATHS_OK -eq 1 ]] && [[ -f "$_DA_COLLECT" ]] &&
  [[ -f "$_DA_ANALYZE" ]] && [[ -f "$_DA_RENDER" ]] &&
  [[ -f "$_DA_CYCLE" ]] && [[ -x "$_DA_BZR_CANONICAL" ]] &&
  [[ $_DA_BZR_CANONICAL == "$REPO_ROOT/target/release/bzr" ]]; then
  test_pass
else
  test_fail "dependency-analysis stage escaped its installed root or release binary"
fi

# Fixture mutations are confined to this disposable functional phase. The
# stable marker is intentionally on the root only: a later read-only recorder
# can discover the root, then follow its dependency fields without creating or
# updating anything.
_DA_MARKER="bzr-dependency-analysis-demo-v1"
_DA_CREATE=(--product FuncTestProd --component Backend --op-sys Linux
  --rep-platform PC --description "dependency analysis fixture")
_DA_HOSTILE_SUMMARY='<script>alert(1)</script> [click](https://evil.invalid)'
_DA_HOSTILE_SUMMARY+=' ``` %%{init: {"theme":"dark"}}%%'
_DA_BASE=$(make_bug "${_DA_CREATE[@]}" --summary "$_DA_HOSTILE_SUMMARY")
_DA_LEFT=$(make_bug "${_DA_CREATE[@]}" --summary "dependency branch left")
_DA_RIGHT=$(make_bug "${_DA_CREATE[@]}" --summary "dependency branch right")
_DA_RESOLVED_PARENT=$(make_bug "${_DA_CREATE[@]}" --summary "resolved blocker parent")
_DA_RESOLVED=$(make_bug "${_DA_CREATE[@]}" --summary "resolved blocker")
_DA_ROOT=$(make_bug --marker "$_DA_MARKER" "${_DA_CREATE[@]}" \
  --summary "dependency analysis delivery root")
_DA_FIXTURE_OK=1
if [[ -z $_DA_BASE || -z $_DA_LEFT || -z $_DA_RIGHT ||
  -z $_DA_RESOLVED_PARENT || -z $_DA_RESOLVED || -z $_DA_ROOT ]]; then
  _DA_FIXTURE_OK=0
else
  run_bzr bug update "$_DA_LEFT" --depends-on-add "$_DA_BASE"
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
  run_bzr bug update "$_DA_RIGHT" --depends-on-add "$_DA_BASE"
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
  run_bzr bug update "$_DA_RESOLVED" --depends-on-add "$_DA_RESOLVED_PARENT"
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
  run_bzr bug update "$_DA_RESOLVED" --status RESOLVED --resolution FIXED
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
  run_bzr bug update "$_DA_ROOT" --depends-on-add "$_DA_LEFT"
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
  run_bzr bug update "$_DA_ROOT" --depends-on-add "$_DA_RIGHT"
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
  run_bzr bug update "$_DA_ROOT" --depends-on-add "$_DA_RESOLVED"
  [[ $BZR_EXIT -eq 0 ]] || _DA_FIXTURE_OK=0
fi

test_begin "123l. disposable live fixture forms a marked diamond and resolved blocker"
if [[ $_DA_FIXTURE_OK -eq 1 ]] && [[ -n $RESTRICTED_BUG ]]; then
  run_bzr bug view "$_DA_ROOT"
  if assert_success && assert_json '.whiteboard' "$_DA_MARKER" &&
    assert_json '.depends_on | sort | join(",")' \
      "$(printf '%s\n' "$_DA_LEFT" "$_DA_RESOLVED" "$_DA_RIGHT" | sort -n | paste -sd, -)"; then
    test_pass
  fi
else
  test_fail "could not provision the dependency-analysis fixture"
fi

_DA_POLICY="$FUNC_CONFIG_DIR/dependency-live.policy.json"
_DA_COLLECTION="$FUNC_CONFIG_DIR/dependency-live.collection.json"
_DA_ANALYSIS="$FUNC_CONFIG_DIR/dependency-live.analysis.json"
_DA_REPORT="$FUNC_CONFIG_DIR/dependency-live.md"
_DA_DIAGRAM="$FUNC_CONFIG_DIR/dependency-live.mmd"
jq -n \
  --arg bzr "$_DA_BZR_CANONICAL" \
  --argjson root "${_DA_ROOT:-0}" \
  '{
    bounds: {max_depth: 5, max_nodes: 20},
    bzr: $bzr,
    direction: "both",
    resolved_mode: "include-no-traverse",
    resolved_statuses: ["RESOLVED"],
    restriction: null,
    scopes: [
      {ids: [$root], kind: "bug-ids", server: "public"},
      {ids: [$root], kind: "bug-ids", server: "test"}
    ],
    servers: ["public", "test"],
    stale_after_days: 14
  }' >"$_DA_POLICY"

_DA_PIPELINE_OK=1
python3 "$_DA_COLLECT" --policy "$_DA_POLICY" --output "$_DA_COLLECTION" ||
  _DA_PIPELINE_OK=0
python3 "$_DA_ANALYZE" --input "$_DA_COLLECTION" --output "$_DA_ANALYSIS" ||
  _DA_PIPELINE_OK=0
python3 "$_DA_RENDER" --input "$_DA_ANALYSIS" --format markdown \
  --output "$_DA_REPORT" || _DA_PIPELINE_OK=0
python3 "$_DA_RENDER" --input "$_DA_ANALYSIS" --format mermaid \
  --output "$_DA_DIAGRAM" || _DA_PIPELINE_OK=0

test_begin "123m. installed live pipeline preserves identities, bounds, and resolved policy"
if [[ $_DA_PIPELINE_OK -eq 1 ]] &&
  jq -e --argjson root "$_DA_ROOT" --argjson base "$_DA_BASE" \
    --argjson resolved "$_DA_RESOLVED" --argjson skipped "$_DA_RESOLVED_PARENT" '
      .schema == "bzr-dependency-analysis/v1" and
      .status == "complete" and
      .bounds == {"max_depth": 5, "max_nodes": 20} and
      .cap == {
        "graph_cap_reached": false,
        "omitted_discovered_identities": 0,
        "scope_truncated": false
      } and
      ([.nodes[] | select(.id == $root) | .server] | sort) == ["public", "test"] and
      ([.nodes[] | select(.id == $resolved and .status == "RESOLVED")] | length) == 2 and
      ([.nodes[] | select(.id == $skipped)] | length) == 0 and
      ([.findings.bottlenecks[] |
        select(.node.id == $base and .fan_out == 2)] | length) == 2 and
      (.findings.execution_order.assumptions |
        index("resolved-include-no-traverse")) != null and
      .longest_chain.kind == "edge_count" and
      .longest_chain.length == 2
    ' "$_DA_ANALYSIS" >/dev/null &&
  grep -q '^````text$' "$_DA_REPORT" &&
  grep -q '<script>alert(1)</script>' "$_DA_REPORT" &&
  ! grep -q '<script>' "$_DA_DIAGRAM" &&
  ! grep -q '%%{init:' "$_DA_DIAGRAM" &&
  grep -q '&lt;script&gt;' "$_DA_DIAGRAM" &&
  grep -q '&#37;&#37;&#123;init:' "$_DA_DIAGRAM"; then
  test_pass
else
  test_fail "live installed pipeline lost graph, policy, cap, or inert rendering evidence"
fi

_DA_MISSING=999999999
_DA_MISSING_POLICY="$FUNC_CONFIG_DIR/dependency-missing.policy.json"
_DA_MISSING_COLLECTION="$FUNC_CONFIG_DIR/dependency-missing.collection.json"
_DA_MISSING_ANALYSIS="$FUNC_CONFIG_DIR/dependency-missing.analysis.json"
jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" \
  --argjson missing "$_DA_MISSING" '
    {
      bounds: {max_depth: 5, max_nodes: 12},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{ids: [$root, $missing], kind: "bug-ids", server: "public"}],
      servers: ["public"],
      stale_after_days: 14
    }
  ' >"$_DA_MISSING_POLICY"
_DA_MISSING_OK=1
python3 "$_DA_COLLECT" --policy "$_DA_MISSING_POLICY" \
  --output "$_DA_MISSING_COLLECTION" || _DA_MISSING_OK=0
python3 "$_DA_ANALYZE" --input "$_DA_MISSING_COLLECTION" \
  --output "$_DA_MISSING_ANALYSIS" || _DA_MISSING_OK=0

test_begin "123n. nonexistent root is sanitized and does not stop visible collection"
if [[ $_DA_MISSING_OK -eq 1 ]] &&
  jq -e --argjson root "$_DA_ROOT" --argjson missing "$_DA_MISSING" '
      ([.nodes[] | select(
        .id == $missing and .state == "unknown" and
        .error_type == "not_found" and .summary == null)] | length) == 1 and
      ([.nodes[] | select(.id == $root and .state == "known")] | length) == 1 and
      ([.. | objects | select(has("message"))] | length) == 0
    ' "$_DA_MISSING_COLLECTION" >/dev/null &&
  jq -e --argjson missing "$_DA_MISSING" '
      [.findings.execution_order.incomplete_boundaries[] |
        select(.id == $missing and .server == "public")] | length == 1
    ' "$_DA_MISSING_ANALYSIS" >/dev/null &&
  ! grep -Eiq 'does not exist|not authorized|not permitted' \
    "$_DA_MISSING_COLLECTION"; then
  test_pass
else
  test_fail "missing root was fatal, unsanitized, or stopped visible traversal"
fi

_DA_INACCESSIBLE_POLICY="$FUNC_CONFIG_DIR/dependency-inaccessible.policy.json"
_DA_INACCESSIBLE_COLLECTION="$FUNC_CONFIG_DIR/dependency-inaccessible.collection.json"
_DA_INACCESSIBLE_ANALYSIS="$FUNC_CONFIG_DIR/dependency-inaccessible.analysis.json"
jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" \
  --argjson restricted "${RESTRICTED_BUG:-0}" '
    {
      bounds: {max_depth: 5, max_nodes: 12},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{ids: [$root, $restricted], kind: "bug-ids", server: "public"}],
      servers: ["public"],
      stale_after_days: 14
    }
  ' >"$_DA_INACCESSIBLE_POLICY"
_DA_INACCESSIBLE_OK=1
python3 "$_DA_COLLECT" --policy "$_DA_INACCESSIBLE_POLICY" \
  --output "$_DA_INACCESSIBLE_COLLECTION" || _DA_INACCESSIBLE_OK=0
python3 "$_DA_ANALYZE" --input "$_DA_INACCESSIBLE_COLLECTION" \
  --output "$_DA_INACCESSIBLE_ANALYSIS" || _DA_INACCESSIBLE_OK=0

test_begin "123o. credentialless inaccessible root is distinct, sanitized, and nonfatal"
if [[ $_DA_INACCESSIBLE_OK -eq 1 ]] &&
  jq -e --argjson root "$_DA_ROOT" --argjson restricted "$RESTRICTED_BUG" '
      ([.nodes[] | select(
        .id == $restricted and .state == "unknown" and
        .error_type == "inaccessible" and .summary == null)] | length) == 1 and
      ([.nodes[] | select(.id == $root and .state == "known")] | length) == 1 and
      ([.. | objects | select(has("message"))] | length) == 0
    ' "$_DA_INACCESSIBLE_COLLECTION" >/dev/null &&
  jq -e --argjson restricted "$RESTRICTED_BUG" '
      [.findings.execution_order.incomplete_boundaries[] |
        select(.id == $restricted and .server == "public")] | length == 1
    ' "$_DA_INACCESSIBLE_ANALYSIS" >/dev/null &&
  ! grep -Eiq 'does not exist|not authorized|not permitted' \
    "$_DA_INACCESSIBLE_COLLECTION"; then
  test_pass
else
  test_fail "inaccessible root was conflated, unsanitized, or stopped visible traversal"
fi

unset RESTRICTED_BUG
unset _DA_ANALYSIS _DA_ANALYZE _DA_BASE _DA_BZR_CANONICAL _DA_COLLECT
unset _DA_COLLECTION _DA_CREATE _DA_CYCLE _DA_DIAGRAM _DA_FIXTURE_OK
unset _DA_HOSTILE_SUMMARY _DA_INACCESSIBLE_ANALYSIS _DA_INACCESSIBLE_COLLECTION
unset _DA_INACCESSIBLE_OK _DA_INACCESSIBLE_POLICY _DA_LEFT _DA_MARKER
unset _DA_MISSING _DA_MISSING_ANALYSIS _DA_MISSING_COLLECTION _DA_MISSING_OK
unset _DA_MISSING_POLICY _DA_PATH _DA_PATH_CANONICAL _DA_PATHS_OK _DA_PIPELINE_OK
unset _DA_POLICY _DA_RENDER _DA_REPORT _DA_RESOLVED _DA_RESOLVED_PARENT
unset _DA_RIGHT _DA_ROOT _DA_SKILL_ROOT _DA_SKILL_ROOT_CANONICAL

echo ""
