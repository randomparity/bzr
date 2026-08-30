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
_DA_CONFIG="$XDG_CONFIG_HOME/bzr/config.toml"
_DA_REJECTED_KEY="FuncTestRejected0123456789abcdef01234567"
printf '\n[servers.dependency-rest-public]\nurl = "%s"\napi_mode = "rest"\n' \
  "$BZ_URL" >>"$_DA_CONFIG"
printf '\n[servers.dependency-xmlrpc-public]\nurl = "%s"\napi_mode = "xmlrpc"\n' \
  "$BZ_URL" >>"$_DA_CONFIG"
printf '\n[servers.dependency-rest-rejected]\nurl = "%s"\napi_key = "%s"\nauth_method = "query_param"\napi_mode = "rest"\n' \
  "$BZ_URL" "$_DA_REJECTED_KEY" >>"$_DA_CONFIG"
printf '\n[servers.dependency-xmlrpc-rejected]\nurl = "%s"\napi_key = "%s"\nauth_method = "query_param"\napi_mode = "xmlrpc"\n' \
  "$BZ_URL" "$_DA_REJECTED_KEY" >>"$_DA_CONFIG"

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
_DA_ALIAS="adj/$BZ_VERSION-$$"
_DA_MISSING_ALIAS="missing/$BZ_VERSION-$$"
_DA_CREATE=(--product FuncTestProd --component Backend --op-sys Linux
  --rep-platform PC --description "dependency analysis fixture")
_DA_HOSTILE_SUMMARY='<script>alert(1)</script> [click](https://evil.invalid)'
_DA_HOSTILE_SUMMARY+=' ``` %%{init: {"theme":"dark"}}%%'
_DA_BASE=$(make_bug "${_DA_CREATE[@]}" --summary "$_DA_HOSTILE_SUMMARY")
_DA_LEFT=$(make_bug "${_DA_CREATE[@]}" --summary "dependency branch left" \
  --alias "$_DA_ALIAS")
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
  _DA_TEST_DEFAULT_ASSIGNEE=$(jq -r '.assigned_to // empty' "$BZR_STDOUT")
  if assert_success && assert_json '.whiteboard' "$_DA_MARKER" &&
    assert_json '.depends_on | sort | join(",")' \
      "$(printf '%s\n' "$_DA_LEFT" "$_DA_RESOLVED" "$_DA_RIGHT" | sort -n | paste -sd, -)"; then
    run_bzr --server public bug view "$_DA_ROOT"
    _DA_PUBLIC_DEFAULT_ASSIGNEE=$(jq -r '.assigned_to // empty' "$BZR_STDOUT")
    if assert_success && [[ -n $_DA_PUBLIC_DEFAULT_ASSIGNEE ]] &&
      [[ -n $_DA_TEST_DEFAULT_ASSIGNEE ]]; then
      test_pass
    fi
  fi
else
  test_fail "could not provision the dependency-analysis fixture"
fi

test_begin "123l1. slash alias persists through bug list search"
if [[ $_DA_FIXTURE_OK -eq 1 ]]; then
  run_bzr bug list --alias "$_DA_ALIAS"
  if assert_success && assert_json_array_length '.' 1 &&
    assert_json '.[0].id' "$_DA_LEFT"; then
    test_pass
  fi
else
  test_skip "no dependency-analysis fixture"
fi

_DA_POLICY="$FUNC_CONFIG_DIR/dependency-live.policy.json"
_DA_COLLECTION="$FUNC_CONFIG_DIR/dependency-live.collection.json"
_DA_ANALYSIS="$FUNC_CONFIG_DIR/dependency-live.analysis.json"
_DA_REPORT="$FUNC_CONFIG_DIR/dependency-live.md"
_DA_DIAGRAM="$FUNC_CONFIG_DIR/dependency-live.mmd"
jq -n \
  --arg bzr "$_DA_BZR_CANONICAL" \
  --argjson root "${_DA_ROOT:-0}" \
  --arg public_default_assignee "${_DA_PUBLIC_DEFAULT_ASSIGNEE:-}" \
  --arg test_default_assignee "${_DA_TEST_DEFAULT_ASSIGNEE:-}" \
  '{
    bounds: {max_depth: 5, max_nodes: 20, max_relationships: 40},
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
    stale_after_days: 14,
    unassigned_assignees: {
      public: [$public_default_assignee],
      test: [$test_default_assignee]
    }
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
    --argjson left "$_DA_LEFT" --argjson right "$_DA_RIGHT" \
    --argjson resolved "$_DA_RESOLVED" --argjson skipped "$_DA_RESOLVED_PARENT" \
    --arg public_default_assignee "$_DA_PUBLIC_DEFAULT_ASSIGNEE" \
    --arg test_default_assignee "$_DA_TEST_DEFAULT_ASSIGNEE" '
      def edge($server; $predecessor; $successor; $observations): {
        observations: $observations,
        predecessor: {id: $predecessor, server: $server},
        successor: {id: $successor, server: $server}
      };
      def inventory($server): [
        edge($server; $base; $left; ["blocks", "depends_on"]),
        edge($server; $base; $right; ["blocks", "depends_on"]),
        edge($server; $left; $root; ["blocks", "depends_on"]),
        edge($server; $right; $root; ["blocks", "depends_on"]),
        edge($server; $resolved; $root; ["depends_on"])
      ];
      .schema == "bzr-dependency-analysis/v1" and
      .status == "complete" and
      .bounds == {"max_depth": 5, "max_nodes": 20, "max_relationships": 40} and
      .cap == {
        "graph_cap_reached": false,
        "omitted_discovered_identities": 0,
        "omitted_relationships_lower_bound": 0,
        "relationship_cap_reached": false,
        "scope_truncated": false
      } and
      .policy.unassigned_assignees == {
        "public": [$public_default_assignee],
        "test": [$test_default_assignee]
      } and
      ([.nodes[] | select(.id == $root) | .server] | sort) == ["public", "test"] and
      .edges == (inventory("public") + inventory("test")) and
      all(.edges[];
        .predecessor.server == .successor.server and
        (.predecessor.server == "public" or .predecessor.server == "test")) and
      ([.nodes[] | select(.id == $resolved and .status == "RESOLVED")] | length) == 2 and
      ([.nodes[] | select(.id == $skipped)] | length) == 0 and
      ([.findings.bottlenecks[] |
        select(.node.id == $base and .fan_out == 2)] | length) == 2 and
      ([.findings.unassigned_blockers[] |
        select(.id == $base and
          (.server == "public" or .server == "test"))] | length) == 2 and
      (.findings.execution_order.assumptions |
        index("resolved-include-no-traverse")) != null and
      .longest_chain.kind == "edge_count" and
      .longest_chain.length == 2
    ' "$_DA_ANALYSIS" >/dev/null &&
  grep -q '^````text$' "$_DA_REPORT" &&
  grep -q '<script>alert(1)</script>' "$_DA_REPORT" &&
  grep -Fq -- '- Graph cap reached: false' "$_DA_REPORT" &&
  grep -Fq -- '- Omitted discovered identities: 0' "$_DA_REPORT" &&
  grep -Fq -- '- Relationship cap reached: false' "$_DA_REPORT" &&
  grep -Fq -- '- Omitted relationships (lower bound): 0' "$_DA_REPORT" &&
  grep -Fq -- '- Traversal direction: both' "$_DA_REPORT" &&
  grep -Fq -- '- Unassigned-assignee policy: public=' "$_DA_REPORT" &&
  grep -Fq -- '- Longest dependency chain components:' "$_DA_REPORT" &&
  grep -Fq -- '- Bottlenecks:' "$_DA_REPORT" &&
  grep -Fq -- '- Execution assumptions:' "$_DA_REPORT" &&
  grep -Fq -- '- Execution component order:' "$_DA_REPORT" &&
  grep -Fq -- '- Incomplete boundaries:' "$_DA_REPORT" &&
  grep -Fq -- '- Analysis warnings:' "$_DA_REPORT" &&
  ! grep -q '<script>' "$_DA_DIAGRAM" &&
  ! grep -q '%%{init:' "$_DA_DIAGRAM" &&
  ! grep -Fq '\"' "$_DA_DIAGRAM" &&
  grep -q '&lt;script&gt;' "$_DA_DIAGRAM" &&
  grep -q '#quot;theme#quot;' "$_DA_DIAGRAM" &&
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
      bounds: {max_depth: 5, max_nodes: 30, max_relationships: 60},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [
        {ids: [$root, $missing], kind: "bug-ids", server: "dependency-rest-public"},
        {ids: [$root, $missing], kind: "bug-ids", server: "dependency-xmlrpc-public"}
      ],
      servers: ["dependency-rest-public", "dependency-xmlrpc-public"],
      stale_after_days: 14
    }
  ' >"$_DA_MISSING_POLICY"
_DA_MISSING_OK=1
python3 "$_DA_COLLECT" --policy "$_DA_MISSING_POLICY" \
  --output "$_DA_MISSING_COLLECTION" || _DA_MISSING_OK=0
python3 "$_DA_ANALYZE" --input "$_DA_MISSING_COLLECTION" \
  --output "$_DA_MISSING_ANALYSIS" || _DA_MISSING_OK=0

test_begin "123n. REST and XML-RPC missing roots are sanitized and nonfatal"
if [[ $_DA_MISSING_OK -eq 1 ]] &&
  jq -e --argjson root "$_DA_ROOT" --argjson missing "$_DA_MISSING" '
      ([.nodes[] | select(
        .id == $missing and .state == "unknown" and
        .error_type == "not_found" and .summary == null)] | length) == 2 and
      ([.nodes[] | select(.id == $root and .state == "known")] | length) == 2 and
      ([.. | objects | select(has("message"))] | length) == 0
    ' "$_DA_MISSING_COLLECTION" >/dev/null &&
  jq -e --argjson missing "$_DA_MISSING" '
      [.findings.execution_order.incomplete_boundaries[] |
        select(.id == $missing)] | length == 2
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
      bounds: {max_depth: 5, max_nodes: 12, max_relationships: 24},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{ids: [$root, $restricted], kind: "bug-ids", server: "dependency-rest-public"}],
      servers: ["dependency-rest-public"],
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
        select(.id == $restricted and .server == "dependency-rest-public")] | length == 1
    ' "$_DA_INACCESSIBLE_ANALYSIS" >/dev/null &&
  ! grep -Eiq 'does not exist|not authorized|not permitted' \
    "$_DA_INACCESSIBLE_COLLECTION"; then
  test_pass
else
  test_fail "inaccessible root was conflated, unsanitized, or stopped visible traversal"
fi

_DA_XMLRPC_INACCESSIBLE_POLICY="$FUNC_CONFIG_DIR/dependency-xmlrpc-inaccessible.policy.json"
_DA_XMLRPC_INACCESSIBLE_COLLECTION="$FUNC_CONFIG_DIR/dependency-xmlrpc-inaccessible.collection.json"
jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" \
  --argjson restricted "${RESTRICTED_BUG:-0}" '
    {
      bounds: {max_depth: 5, max_nodes: 12, max_relationships: 24},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{ids: [$root, $restricted], kind: "bug-ids", server: "dependency-xmlrpc-public"}],
      servers: ["dependency-xmlrpc-public"],
      stale_after_days: 14
    }
  ' >"$_DA_XMLRPC_INACCESSIBLE_POLICY"
_DA_XMLRPC_INACCESSIBLE_OK=1
python3 "$_DA_COLLECT" --policy "$_DA_XMLRPC_INACCESSIBLE_POLICY" \
  --output "$_DA_XMLRPC_INACCESSIBLE_COLLECTION" || _DA_XMLRPC_INACCESSIBLE_OK=0

test_begin "123o2. XML-RPC inaccessible 102 is resource-scoped after preflight"
if [[ $_DA_XMLRPC_INACCESSIBLE_OK -eq 1 ]] &&
  jq -e --argjson root "$_DA_ROOT" --argjson restricted "$RESTRICTED_BUG" '
      ([.nodes[] | select(
        .id == $restricted and .state == "unknown" and
        .error_type == "inaccessible" and .summary == null)] | length) == 1 and
      ([.nodes[] | select(.id == $root and .state == "known")] | length) == 1 and
      ([.. | objects | select(has("message"))] | length) == 0
    ' "$_DA_XMLRPC_INACCESSIBLE_COLLECTION" >/dev/null &&
  ! grep -Eiq 'does not exist|not authorized|not permitted' \
    "$_DA_XMLRPC_INACCESSIBLE_COLLECTION"; then
  test_pass
else
  test_fail "XML-RPC did not expose inaccessible 102 after a successful preflight"
fi

_DA_REJECTED_REST_POLICY="$FUNC_CONFIG_DIR/dependency-rest-rejected.policy.json"
_DA_REJECTED_REST_COLLECTION="$FUNC_CONFIG_DIR/dependency-rest-rejected.collection.json"
_DA_REJECTED_REST_ERROR="$FUNC_CONFIG_DIR/dependency-rest-rejected.stderr"
_DA_REJECTED_XMLRPC_POLICY="$FUNC_CONFIG_DIR/dependency-xmlrpc-rejected.policy.json"
_DA_REJECTED_XMLRPC_COLLECTION="$FUNC_CONFIG_DIR/dependency-xmlrpc-rejected.collection.json"
_DA_REJECTED_XMLRPC_ERROR="$FUNC_CONFIG_DIR/dependency-xmlrpc-rejected.stderr"
for _DA_REJECTED_MODE in rest xmlrpc; do
  if [[ $_DA_REJECTED_MODE == rest ]]; then
    _DA_REJECTED_POLICY=$_DA_REJECTED_REST_POLICY
  else
    _DA_REJECTED_POLICY=$_DA_REJECTED_XMLRPC_POLICY
  fi
  jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" \
    --arg mode "$_DA_REJECTED_MODE" '
      {
        bounds: {max_depth: 5, max_nodes: 12, max_relationships: 24},
        bzr: $bzr,
        direction: "both",
        resolved_mode: "include-no-traverse",
        resolved_statuses: ["RESOLVED"],
        restriction: null,
        scopes: [{ids: [$root], kind: "bug-ids", server: ("dependency-" + $mode + "-rejected")}],
        servers: [("dependency-" + $mode + "-rejected")],
        stale_after_days: 14
      }
    ' >"$_DA_REJECTED_POLICY"
done
if RUST_LOG=bzr=debug python3 "$_DA_COLLECT" --policy "$_DA_REJECTED_REST_POLICY" \
  --output "$_DA_REJECTED_REST_COLLECTION" 2>"$_DA_REJECTED_REST_ERROR"; then
  _DA_REJECTED_REST_EXIT=0
else
  _DA_REJECTED_REST_EXIT=$?
fi
if RUST_LOG=bzr=debug python3 "$_DA_COLLECT" --policy "$_DA_REJECTED_XMLRPC_POLICY" \
  --output "$_DA_REJECTED_XMLRPC_COLLECTION" 2>"$_DA_REJECTED_XMLRPC_ERROR"; then
  _DA_REJECTED_XMLRPC_EXIT=0
else
  _DA_REJECTED_XMLRPC_EXIT=$?
fi

test_begin "123o3. rejected credentials fail sanitized preflight before resource reads"
if [[ $_DA_REJECTED_REST_EXIT -eq 1 ]] && [[ $_DA_REJECTED_XMLRPC_EXIT -eq 1 ]] &&
  jq -e '
      .status == "partial" and .limitations == ["collection-api"] and
      .nodes == [] and .roots == [] and
      .cap == {
        "graph_cap_reached": false,
        "omitted_discovered_identities": 0,
        "omitted_relationships_lower_bound": 0,
        "relationship_cap_reached": false,
        "scope_truncated": false
      }
    ' "$_DA_REJECTED_REST_COLLECTION" >/dev/null &&
  jq -e '
      .status == "partial" and .limitations == ["collection-api"] and
      .nodes == [] and .roots == []
    ' "$_DA_REJECTED_XMLRPC_COLLECTION" >/dev/null &&
  grep -Fxq 'collection failed: api' "$_DA_REJECTED_REST_ERROR" &&
  grep -Fxq 'collection failed: api' "$_DA_REJECTED_XMLRPC_ERROR" &&
  ! grep -Fq "$_DA_REJECTED_KEY" "$_DA_REJECTED_REST_ERROR" \
    "$_DA_REJECTED_XMLRPC_ERROR" "$_DA_REJECTED_REST_COLLECTION" \
    "$_DA_REJECTED_XMLRPC_COLLECTION"; then
  test_pass
else
  test_fail "rejected credential did not stop at a sanitized command-fatal preflight"
fi

_DA_RELATIONSHIP_POLICY="$FUNC_CONFIG_DIR/dependency-relationship-cap.policy.json"
_DA_RELATIONSHIP_COLLECTION="$FUNC_CONFIG_DIR/dependency-relationship-cap.collection.json"
_DA_RELATIONSHIP_ANALYSIS="$FUNC_CONFIG_DIR/dependency-relationship-cap.analysis.json"
jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" '
  {
    bounds: {max_depth: 5, max_nodes: 20, max_relationships: 1},
    bzr: $bzr,
    direction: "both",
    resolved_mode: "include-no-traverse",
    resolved_statuses: ["RESOLVED"],
    restriction: null,
    scopes: [{ids: [$root], kind: "bug-ids", server: "test"}],
    servers: ["test"],
    stale_after_days: 14
  }
' >"$_DA_RELATIONSHIP_POLICY"
_DA_RELATIONSHIP_OK=1
python3 "$_DA_COLLECT" --policy "$_DA_RELATIONSHIP_POLICY" \
  --output "$_DA_RELATIONSHIP_COLLECTION" || _DA_RELATIONSHIP_OK=0
python3 "$_DA_ANALYZE" --input "$_DA_RELATIONSHIP_COLLECTION" --allow-partial \
  --output "$_DA_RELATIONSHIP_ANALYSIS" || _DA_RELATIONSHIP_OK=0

test_begin "123p. installed live relationship cap retains a bounded partial prefix"
if [[ $_DA_RELATIONSHIP_OK -eq 1 ]] &&
  jq -e --argjson root "$_DA_ROOT" '
      .status == "partial" and
      .limitations == ["relationship_cap"] and
      .bounds.max_relationships == 1 and
      .cap.relationship_cap_reached == true and
      .cap.omitted_relationships_lower_bound >= 2 and
      ([.nodes[] | select(.id == $root and .state == "known")] | length) == 1 and
      (.observations | length) == 1 and
      (.nodes | length) == 2
    ' "$_DA_RELATIONSHIP_COLLECTION" >/dev/null &&
  jq -e '
      .status == "partial" and
      .limitations == ["relationship_cap"] and
      .cap.relationship_cap_reached == true and
      (.edges | length) == 1
    ' "$_DA_RELATIONSHIP_ANALYSIS" >/dev/null; then
  test_pass
else
  test_fail "relationship cap did not retain a deterministic bounded partial prefix"
fi

_DA_POLICY_SECRET="FuncPolicySecret0123456789abcdef"
_DA_CREDENTIAL_URL_POLICY="$FUNC_CONFIG_DIR/dependency-credential-url.policy.json"
_DA_CREDENTIAL_URL_COLLECTION="$FUNC_CONFIG_DIR/dependency-credential-url.collection.json"
_DA_CREDENTIAL_URL_ERROR="$FUNC_CONFIG_DIR/dependency-credential-url.stderr"
_DA_EXTRA_SERVER_POLICY="$FUNC_CONFIG_DIR/dependency-extra-server.policy.json"
_DA_EXTRA_SERVER_COLLECTION="$FUNC_CONFIG_DIR/dependency-extra-server.collection.json"
_DA_EXTRA_SERVER_ERROR="$FUNC_CONFIG_DIR/dependency-extra-server.stderr"
jq -n --arg bzr "$_DA_BZR_CANONICAL" \
  --arg url "$BZ_URL/buglist.cgi?product=FuncTestProd&Bugzilla_API_Key=$_DA_POLICY_SECRET" '
    {
      bounds: {max_depth: 5, max_nodes: 20, max_relationships: 20},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{kind: "custom-search", parameter_names: ["product"], server: "test", url: $url}],
      servers: ["test"],
      stale_after_days: 14
    }
  ' >"$_DA_CREDENTIAL_URL_POLICY"
jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" '
    {
      bounds: {max_depth: 5, max_nodes: 20, max_relationships: 20},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{ids: [$root], kind: "bug-ids", server: "test"}],
      servers: ["dependency-rest-public", "test"],
      stale_after_days: 14
    }
  ' >"$_DA_EXTRA_SERVER_POLICY"
if python3 "$_DA_COLLECT" --policy "$_DA_CREDENTIAL_URL_POLICY" \
  --output "$_DA_CREDENTIAL_URL_COLLECTION" 2>"$_DA_CREDENTIAL_URL_ERROR"; then
  _DA_CREDENTIAL_URL_EXIT=0
else
  _DA_CREDENTIAL_URL_EXIT=$?
fi
if python3 "$_DA_COLLECT" --policy "$_DA_EXTRA_SERVER_POLICY" \
  --output "$_DA_EXTRA_SERVER_COLLECTION" 2>"$_DA_EXTRA_SERVER_ERROR"; then
  _DA_EXTRA_SERVER_EXIT=0
else
  _DA_EXTRA_SERVER_EXIT=$?
fi

test_begin "123q. installed policy rejects credential URLs and unused servers before retrieval"
if [[ $_DA_CREDENTIAL_URL_EXIT -eq 2 ]] && [[ $_DA_EXTRA_SERVER_EXIT -eq 2 ]] &&
  [[ ! -e $_DA_CREDENTIAL_URL_COLLECTION ]] && [[ ! -e $_DA_EXTRA_SERVER_COLLECTION ]] &&
  grep -Fxq 'policy error: scopes[0].url must not include credentials' \
    "$_DA_CREDENTIAL_URL_ERROR" &&
  grep -Fxq 'policy error: servers must match scope and restriction servers' \
    "$_DA_EXTRA_SERVER_ERROR" &&
  ! grep -Fq "$_DA_POLICY_SECRET" "$_DA_CREDENTIAL_URL_ERROR" \
    "$_DA_EXTRA_SERVER_ERROR"; then
  test_pass
else
  test_fail "installed policy crossed a credential or server-universe trust boundary"
fi

# The collector checks above intentionally exercise its unchanged traversal
# contract before these extra relationships are added. Adjacency then reuses
# the same live public bugs to prove complete, sorted arrays in both directions
# without changing the collector's expected graph.
_DA_ADJ_FIXTURE_OK=1
run_bzr bug update "$_DA_LEFT" --depends-on-add "$_DA_RESOLVED_PARENT"
[[ $BZR_EXIT -eq 0 ]] || _DA_ADJ_FIXTURE_OK=0
run_bzr bug update "$_DA_LEFT" --blocks-add "$_DA_RIGHT"
[[ $BZR_EXIT -eq 0 ]] || _DA_ADJ_FIXTURE_OK=0

test_begin "123r. live adjacency fixture has two complete directions"
if [[ $_DA_ADJ_FIXTURE_OK -eq 1 ]]; then
  test_pass
else
  test_fail "could not extend the live graph for adjacency coverage"
fi

_DA_ADJ_REST="$FUNC_CONFIG_DIR/dependency-adjacency-rest.json"
_DA_ADJ_XMLRPC="$FUNC_CONFIG_DIR/dependency-adjacency-xmlrpc.json"
_DA_ADJ_PARITY_OK=1
for _DA_ADJ_MODE in rest xmlrpc; do
  _DA_ADJ_SERVER="dependency-$_DA_ADJ_MODE-public"

  test_begin "123s. $_DA_ADJ_MODE numeric and slash-alias requests both resolve"
  run_bzr_raw --json --server "$_DA_ADJ_SERVER" \
    bug adjacency "$_DA_ROOT" "$_DA_ALIAS"
  if assert_exit_code 0 &&
    assert_raw_json '.schema_version' '1.0.0' &&
    assert_json '.requests == [
      {requested: "'"$_DA_ROOT"'", bug_id: '"$_DA_ROOT"'},
      {requested: "'"$_DA_ALIAS"'", bug_id: '"$_DA_LEFT"'}
    ]' 'true' &&
    assert_json '[.bugs[].id] == (['"$_DA_ROOT"', '"$_DA_LEFT"'] | sort)' 'true'; then
    test_pass
  fi

  test_begin "123t. $_DA_ADJ_MODE alias and numeric identities converge once"
  run_bzr_raw --json --server "$_DA_ADJ_SERVER" \
    bug adjacency "$_DA_ALIAS" "$_DA_LEFT"
  if assert_exit_code 0 &&
    assert_json '.requests == [
      {requested: "'"$_DA_ALIAS"'", bug_id: '"$_DA_LEFT"'},
      {requested: "'"$_DA_LEFT"'", bug_id: '"$_DA_LEFT"'}
    ]' 'true' &&
    assert_json '.bugs | length' '1' &&
    assert_json '.bugs[0].id' "$_DA_LEFT" &&
    assert_json '.bugs[0].blocks == (['"$_DA_ROOT"', '"$_DA_RIGHT"'] | sort)' 'true' &&
    assert_json '.bugs[0].depends_on == (['"$_DA_BASE"', '"$_DA_RESOLVED_PARENT"'] | sort)' 'true'; then
    test_pass
  fi

  test_begin "123u. $_DA_ADJ_MODE mixed resource failures retain typed identities and exit zero"
  run_bzr_raw --json --server "$_DA_ADJ_SERVER" \
    bug adjacency "$_DA_ROOT" "$_DA_MISSING" "$_DA_MISSING_ALIAS"
  if assert_exit_code 0 &&
    assert_json '.requests == [
      {requested: "'"$_DA_ROOT"'", bug_id: '"$_DA_ROOT"'},
      {requested: "'"$_DA_MISSING"'", error: {type: "not_found", api_code: 101}},
      {requested: "'"$_DA_MISSING_ALIAS"'", error: {type: "not_found", api_code: 100}}
    ]' 'true' &&
    assert_json '[.bugs[].id] == ['"$_DA_ROOT"']' 'true'; then
    test_pass
  fi

  test_begin "123v. $_DA_ADJ_MODE all-failure result is closed and exits zero"
  run_bzr_raw --json --server "$_DA_ADJ_SERVER" \
    bug adjacency "$_DA_MISSING" "$_DA_MISSING_ALIAS"
  if assert_exit_code 0 &&
    assert_json '. == {
      requests: [
        {requested: "'"$_DA_MISSING"'", error: {type: "not_found", api_code: 101}},
        {requested: "'"$_DA_MISSING_ALIAS"'", error: {type: "not_found", api_code: 100}}
      ],
      bugs: []
    }' 'true'; then
    test_pass
  fi

  test_begin "123w. $_DA_ADJ_MODE anonymous restricted bug is typed inaccessible"
  run_bzr_raw --json --server "$_DA_ADJ_SERVER" bug adjacency "$RESTRICTED_BUG"
  if assert_exit_code 0 &&
    assert_json '. == {
      requests: [{
        requested: "'"$RESTRICTED_BUG"'",
        error: {type: "inaccessible", api_code: 102}
      }],
      bugs: []
    }' 'true'; then
    test_pass
  fi

  run_bzr_raw --json --server "$_DA_ADJ_SERVER" \
    bug adjacency "$_DA_ROOT" "$_DA_ALIAS" "$_DA_MISSING" \
    "$_DA_MISSING_ALIAS" "$RESTRICTED_BUG"
  if [[ $BZR_EXIT -ne 0 ]]; then
    _DA_ADJ_PARITY_OK=0
  elif [[ $_DA_ADJ_MODE == rest ]]; then
    # REST uses RFC 3339 while XML-RPC uses Bugzilla's compact ISO-8601
    # spelling for the same instant. Normalize only that wire-level spelling
    # so every adjacency field and ordering decision remains compared.
    jq -S '(.bugs[].last_change_time) |=
      (if type == "string" then
         if test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$") then
           . as $timestamp |
           (try ($timestamp | strptime("%Y-%m-%dT%H:%M:%SZ") |
             mktime | gmtime | strftime("%Y-%m-%dT%H:%M:%SZ")) catch "") as $round_trip |
           if $round_trip == $timestamp then
             capture("^(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})T(?<hour>[0-9]{2}):(?<minute>[0-9]{2}):(?<second>[0-9]{2})Z$") |
             "\(.year)\(.month)\(.day)T\(.hour):\(.minute):\(.second)"
           else
             error("REST last_change_time must be a valid RFC3339 UTC timestamp")
           end
         else
           error("REST last_change_time must use RFC3339 UTC format")
         end
       else
         error("REST last_change_time must be a string")
       end)' \
      "$BZR_STDOUT" >"$_DA_ADJ_REST" || _DA_ADJ_PARITY_OK=0
  else
    jq -S '(.bugs[].last_change_time) |=
      (if type == "string" then
         if test("^[0-9]{8}T[0-9]{2}:[0-9]{2}:[0-9]{2}$") then
           . as $timestamp |
           (try ($timestamp | strptime("%Y%m%dT%H:%M:%S") |
             mktime | gmtime | strftime("%Y%m%dT%H:%M:%S")) catch "") as $round_trip |
           if $round_trip == $timestamp then .
           else error("XML-RPC last_change_time must be a valid compact ISO-8601 timestamp")
           end
         else error("XML-RPC last_change_time must use compact ISO-8601 format")
         end
       else error("XML-RPC last_change_time must be a string")
       end)' \
      "$BZR_STDOUT" >"$_DA_ADJ_XMLRPC" || _DA_ADJ_PARITY_OK=0
  fi
done

test_begin "123x. live REST and XML-RPC adjacency payloads have transport parity"
if [[ $_DA_ADJ_PARITY_OK -eq 1 ]] && [[ -f $_DA_ADJ_REST ]] &&
  [[ -f $_DA_ADJ_XMLRPC ]] && cmp -s "$_DA_ADJ_REST" "$_DA_ADJ_XMLRPC"; then
  test_pass
else
  test_fail "REST and XML-RPC adjacency payloads differ"
fi

_DA_PRODUCTION_POLICY_OK=1
_DA_PRODUCTION_POLICY_COLLECTION="$FUNC_CONFIG_DIR/dependency-production-policy.collection.json"
_DA_PRODUCTION_POLICY_FAILURE="$FUNC_CONFIG_DIR/dependency-production-policy-failure.collection.json"
_DA_PRODUCTION_POLICY_FAILURE_ERROR="$FUNC_CONFIG_DIR/dependency-production-policy-failure.stderr"
_DA_PRODUCTION_POLICY="$FUNC_CONFIG_DIR/dependency-production-policy.json"
_DA_PRODUCTION_POLICY_REJECT="$FUNC_CONFIG_DIR/dependency-production-policy-reject.json"
if redhat_shape_start "$BZ_PORT"; then
  trap 'cleanup; redhat_shape_stop' EXIT
  _DA_PRODUCTION_POLICY_URL="http://127.0.0.1:${REDHAT_SHAPE_PORT}"
  printf '\n[servers.dependency-production-policy]\nurl = "%s"\napi_mode = "rest"\n' \
    "$_DA_PRODUCTION_POLICY_URL" >>"$_DA_CONFIG"

  test_begin "123y. production-policy proxy rejects the legacy termless preflight"
  run_bzr_raw --json --server dependency-production-policy bug list \
    --limit 1 --offset 0 --fields id --sort bug_id --order asc
  if assert_exit_code 4 && jq -e '
      .error.type == "api" and .error.api_code == 1000
    ' "$BZR_STDERR" >/dev/null; then
    test_pass
  else
    _DA_PRODUCTION_POLICY_OK=0
  fi

  jq -n --arg bzr "$_DA_BZR_CANONICAL" --argjson root "${_DA_ROOT:-0}" '
    {
      bounds: {max_depth: 1, max_nodes: 2, max_relationships: 2},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{ids: [$root], kind: "bug-ids", server: "dependency-production-policy"}],
      servers: ["dependency-production-policy"],
      stale_after_days: 14
    }
  ' >"$_DA_PRODUCTION_POLICY"
  if ! python3 "$_DA_COLLECT" --policy "$_DA_PRODUCTION_POLICY" \
    --output "$_DA_PRODUCTION_POLICY_COLLECTION"; then
    _DA_PRODUCTION_POLICY_OK=0
  fi

  test_begin "123z. installed collector uses a scoped proof through the production-policy proxy"
  if [[ $_DA_PRODUCTION_POLICY_OK -eq 1 ]] &&
    jq -e --argjson root "$_DA_ROOT" '
      .status == "complete" and
      any(.nodes[]; .id == $root and .state == "known") and
      .limitations == []
    ' "$_DA_PRODUCTION_POLICY_COLLECTION" >/dev/null; then
    test_pass
  else
    test_fail "scope-qualified collection failed through production-policy proxy"
  fi

  jq -n --arg bzr "$_DA_BZR_CANONICAL" \
    --arg url "$_DA_PRODUCTION_POLICY_URL/buglist.cgi?product=" '
    {
      bounds: {max_depth: 1, max_nodes: 2, max_relationships: 2},
      bzr: $bzr,
      direction: "both",
      resolved_mode: "include-no-traverse",
      resolved_statuses: ["RESOLVED"],
      restriction: null,
      scopes: [{
        kind: "custom-search",
        parameter_names: ["product"],
        server: "dependency-production-policy",
        url: $url
      }],
      servers: ["dependency-production-policy"],
      stale_after_days: 14
    }
  ' >"$_DA_PRODUCTION_POLICY_REJECT"
  if python3 "$_DA_COLLECT" --policy "$_DA_PRODUCTION_POLICY_REJECT" \
    --output "$_DA_PRODUCTION_POLICY_FAILURE" 2>"$_DA_PRODUCTION_POLICY_FAILURE_ERROR"; then
    _DA_PRODUCTION_POLICY_FAILURE_EXIT=0
  else
    _DA_PRODUCTION_POLICY_FAILURE_EXIT=$?
  fi

  test_begin "123z1. installed collector preserves production code 1000 as API failure"
  if [[ $_DA_PRODUCTION_POLICY_FAILURE_EXIT -eq 1 ]] &&
    jq -e '
      .status == "partial" and .limitations == ["collection-api"] and
      .nodes == [] and .roots == []
    ' "$_DA_PRODUCTION_POLICY_FAILURE" >/dev/null &&
    grep -Fxq 'collection failed: api' "$_DA_PRODUCTION_POLICY_FAILURE_ERROR"; then
    test_pass
  else
    test_fail "installed collector did not preserve production code 1000 classification"
  fi

  redhat_shape_stop || _DA_PRODUCTION_POLICY_OK=0
  trap cleanup EXIT
else
  test_begin "123y. production-policy proxy rejects the legacy termless preflight"
  test_fail "production-policy proxy did not become ready: $REDHAT_SHAPE_LOG"
  test_begin "123z. installed collector uses a scoped proof through the production-policy proxy"
  test_skip "production-policy proxy unavailable"
  test_begin "123z1. installed collector preserves production code 1000 as API failure"
  test_skip "production-policy proxy unavailable"
fi

unset RESTRICTED_BUG
unset _DA_ADJ_FIXTURE_OK _DA_ADJ_MODE _DA_ADJ_PARITY_OK _DA_ADJ_REST
unset _DA_ADJ_SERVER _DA_ADJ_XMLRPC
unset _DA_ALIAS _DA_ANALYSIS _DA_ANALYZE _DA_BASE _DA_BZR_CANONICAL _DA_COLLECT _DA_CONFIG
unset _DA_COLLECTION _DA_CREATE _DA_CYCLE _DA_DIAGRAM
unset _DA_CREDENTIAL_URL_COLLECTION _DA_CREDENTIAL_URL_ERROR _DA_CREDENTIAL_URL_EXIT
unset _DA_CREDENTIAL_URL_POLICY
unset _DA_EXTRA_SERVER_COLLECTION _DA_EXTRA_SERVER_ERROR _DA_EXTRA_SERVER_EXIT
unset _DA_EXTRA_SERVER_POLICY
unset _DA_FIXTURE_OK
unset _DA_HOSTILE_SUMMARY _DA_INACCESSIBLE_ANALYSIS _DA_INACCESSIBLE_COLLECTION
unset _DA_INACCESSIBLE_OK _DA_INACCESSIBLE_POLICY _DA_LEFT _DA_MARKER
unset _DA_MISSING _DA_MISSING_ALIAS _DA_MISSING_ANALYSIS _DA_MISSING_COLLECTION _DA_MISSING_OK
unset _DA_MISSING_POLICY _DA_PATH _DA_PATH_CANONICAL _DA_PATHS_OK _DA_PIPELINE_OK
unset _DA_POLICY _DA_PUBLIC_DEFAULT_ASSIGNEE _DA_RENDER _DA_REPORT
unset _DA_POLICY_SECRET
unset _DA_PRODUCTION_POLICY _DA_PRODUCTION_POLICY_COLLECTION
unset _DA_PRODUCTION_POLICY_FAILURE _DA_PRODUCTION_POLICY_FAILURE_ERROR
unset _DA_PRODUCTION_POLICY_FAILURE_EXIT _DA_PRODUCTION_POLICY_OK
unset _DA_PRODUCTION_POLICY_REJECT _DA_PRODUCTION_POLICY_URL
unset _DA_RESOLVED _DA_RESOLVED_PARENT
unset _DA_RELATIONSHIP_ANALYSIS _DA_RELATIONSHIP_COLLECTION _DA_RELATIONSHIP_OK
unset _DA_RELATIONSHIP_POLICY
unset _DA_REJECTED_KEY _DA_REJECTED_MODE _DA_REJECTED_POLICY
unset _DA_REJECTED_REST_COLLECTION _DA_REJECTED_REST_ERROR _DA_REJECTED_REST_EXIT
unset _DA_REJECTED_REST_POLICY _DA_REJECTED_XMLRPC_COLLECTION
unset _DA_REJECTED_XMLRPC_ERROR _DA_REJECTED_XMLRPC_EXIT _DA_REJECTED_XMLRPC_POLICY
unset _DA_RIGHT _DA_ROOT _DA_SKILL_ROOT _DA_SKILL_ROOT_CANONICAL
unset _DA_TEST_DEFAULT_ASSIGNEE
unset _DA_XMLRPC_INACCESSIBLE_COLLECTION _DA_XMLRPC_INACCESSIBLE_OK
unset _DA_XMLRPC_INACCESSIBLE_POLICY

echo ""
