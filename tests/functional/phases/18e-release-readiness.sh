# 18e-release-readiness
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble. Fixture mutations finish before the review segment.
# shellcheck shell=bash

echo "── Phase 18e: Release-readiness workflow ────────────"

_RR_MARKER="bzr-release-readiness-demo-v1"
_RR_PRODUCT=$(unique_name rr-product)
_RR_VERSION=$(unique_name rr-version)
_RR_QUERY="release-readiness-demo"
_RR_URL_QUERY="release-readiness-demo-url"
_RR_FIELDS="id,summary,status,priority,severity,assigned_to,target_milestone,version,deadline,last_change_time,whiteboard,depends_on"
_RR_CREATE=(--product "$_RR_PRODUCT" --component Release --version "$_RR_VERSION"
  --op-sys Linux --rep-platform PC --description "release-readiness fixture")

_RR_FIXTURE_OK=1
run_bzr product create --name "$_RR_PRODUCT" \
  --description "Release-readiness functional fixture" --version "$_RR_VERSION"
[[ $BZR_EXIT -eq 0 ]] || _RR_FIXTURE_OK=0
run_bzr component create --product "$_RR_PRODUCT" --name Release \
  --description "Release delivery" --default-assignee "$ADMIN_EMAIL"
[[ $BZR_EXIT -eq 0 ]] || _RR_FIXTURE_OK=0

_RR_DEPENDENCY=$(make_bug "${_RR_CREATE[@]}" \
  --summary "Release dependency remains open" --priority High \
  --deadline 2027-12-31 --whiteboard "$_RR_MARKER release-risk")
_RR_COMPLETE=$(make_bug "${_RR_CREATE[@]}" \
  --summary "Completed release validation" --priority Highest \
  --deadline 2026-01-15 --whiteboard "$_RR_MARKER release-complete")
_RR_ROOT=$(make_bug "${_RR_CREATE[@]}" \
  --summary "Release candidate requires approval" --priority Highest \
  --deadline 2026-01-15 --whiteboard "$_RR_MARKER release-blocker" \
  --depends-on "$_RR_DEPENDENCY")
if [[ -z $_RR_DEPENDENCY || -z $_RR_COMPLETE || -z $_RR_ROOT ]]; then
  _RR_FIXTURE_OK=0
else
  run_bzr bug update "$_RR_ROOT" --status IN_PROGRESS --assignee "$ADMIN_EMAIL"
  [[ $BZR_EXIT -eq 0 ]] || _RR_FIXTURE_OK=0
  run_bzr bug update "$_RR_COMPLETE" --status RESOLVED --resolution FIXED
  [[ $BZR_EXIT -eq 0 ]] || _RR_FIXTURE_OK=0
fi

_RR_URL="${BZ_URL}/buglist.cgi?product=${_RR_PRODUCT}&query_format=advanced&limit=1&order=changeddate%20DESC"
run_bzr query save "$_RR_QUERY" --product "$_RR_PRODUCT" --limit 1 \
  --sort last_change_time --order desc
[[ $BZR_EXIT -eq 0 ]] || _RR_FIXTURE_OK=0
run_bzr query save "$_RR_URL_QUERY" --from-url "$_RR_URL" --limit 1
[[ $BZR_EXIT -eq 0 ]] || _RR_FIXTURE_OK=0

test_begin "123r. release fixture carries deadline, owner, milestone, history, and dependency"
if [[ $_RR_FIXTURE_OK -eq 1 ]]; then
  run_bzr bug view "$_RR_ROOT"
  if assert_success && assert_json '.priority' "Highest" &&
    assert_json '.assigned_to' "$ADMIN_EMAIL" &&
    assert_json '.target_milestone' "---" &&
    assert_json '.version' "$_RR_VERSION" &&
    assert_json '.deadline' "2026-01-15" &&
    assert_json '.whiteboard' "$_RR_MARKER release-blocker" &&
    assert_json '.depends_on[0]' "$_RR_DEPENDENCY"; then
    run_bzr bug history "$_RR_ROOT"
    if assert_success &&
      assert_json '[.[] | select(.field == "status" and .new_value == "IN_PROGRESS")] | length' 1; then
      test_pass
    fi
  fi
else
  test_fail "could not provision release-readiness fixture"
fi

# Everything below is the read-only review segment. Preserve the local profile,
# one complete bug record, and its history to prove the segment made no change.
_RR_CONFIG="$XDG_CONFIG_HOME/bzr/config.toml"
_RR_CONFIG_BEFORE="$FUNC_CONFIG_DIR/release-readiness-config.before"
_RR_BUG_BEFORE="$FUNC_CONFIG_DIR/release-readiness-bug.before.json"
_RR_HISTORY_BEFORE="$FUNC_CONFIG_DIR/release-readiness-history.before.json"
cp "$_RR_CONFIG" "$_RR_CONFIG_BEFORE"
run_bzr bug view "$_RR_ROOT"
cp "$BZR_STDOUT" "$_RR_BUG_BEFORE"
run_bzr bug history "$_RR_ROOT"
cp "$BZR_STDOUT" "$_RR_HISTORY_BEFORE"

test_begin "123s. five release scope forms use bounded complete reads"
_RR_SCOPES_OK=1
run_bzr --server test bug search --from-url "$_RR_URL" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$_RR_FIELDS"
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e --argjson root "$_RR_ROOT" 'any(.[]; .id == $root)' "$BZR_STDOUT" >/dev/null ||
  _RR_SCOPES_OK=0
run_bzr query show "$_RR_QUERY"
[[ $BZR_EXIT -eq 0 ]] && [[ $(jq -r '.limit' "$BZR_STDOUT") == 1 ]] ||
  _RR_SCOPES_OK=0
run_bzr query run "$_RR_QUERY" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$_RR_FIELDS"
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e --argjson root "$_RR_ROOT" 'any(.[]; .id == $root)' "$BZR_STDOUT" >/dev/null ||
  _RR_SCOPES_OK=0
run_bzr bug list --target-milestone=--- --limit 100 --paginate \
  --sort bug_id --order asc --fields "$_RR_FIELDS"
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e --argjson root "$_RR_ROOT" 'any(.[]; .id == $root)' "$BZR_STDOUT" >/dev/null ||
  _RR_SCOPES_OK=0
run_bzr bug list --version "$_RR_VERSION" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$_RR_FIELDS"
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e --argjson root "$_RR_ROOT" 'any(.[]; .id == $root)' "$BZR_STDOUT" >/dev/null ||
  _RR_SCOPES_OK=0
run_bzr bug list --product "$_RR_PRODUCT" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$_RR_FIELDS"
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e --argjson root "$_RR_ROOT" --argjson complete "$_RR_COMPLETE" '
    (map(.id) | index($root)) != null and
    (map(.id) | index($complete)) != null and
    ([.[].id] | . == sort)
  ' "$BZR_STDOUT" >/dev/null || _RR_SCOPES_OK=0
if [[ $_RR_SCOPES_OK -eq 1 ]]; then
  test_pass
else
  test_fail "a release scope failed, omitted the fixture, or lost stable ordering"
fi

test_begin "123t. supplementary release evidence is structured and bounded"
_RR_SUPPLEMENT_OK=1
run_bzr bug view "$_RR_ROOT" --fields "$_RR_FIELDS"
[[ $BZR_EXIT -eq 0 ]] && [[ $(jq -r '.id' "$BZR_STDOUT") == "$_RR_ROOT" ]] ||
  _RR_SUPPLEMENT_OK=0
run_bzr bug history "$_RR_ROOT" --since 2020-01-01
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e 'any(.[]; .field == "status")' "$BZR_STDOUT" >/dev/null ||
  _RR_SUPPLEMENT_OK=0
run_bzr bug links "$_RR_ROOT" --relation depends_on
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e --argjson dependency "$_RR_DEPENDENCY" '
    any(.[]; .id == $dependency and .relation == "depends_on")
  ' "$BZR_STDOUT" >/dev/null || _RR_SUPPLEMENT_OK=0
run_bzr field list status
[[ $BZR_EXIT -eq 0 ]] && jq -e 'length > 0' "$BZR_STDOUT" >/dev/null ||
  _RR_SUPPLEMENT_OK=0
run_bzr server capabilities
_RR_CUSTOM_FIELD_COUNT=$(jq -r '.custom_fields | length' "$BZR_STDOUT" 2>/dev/null || true)
[[ $BZR_EXIT -eq 0 ]] && [[ $_RR_CUSTOM_FIELD_COUNT =~ ^[0-9]+$ ]] ||
  _RR_SUPPLEMENT_OK=0
run_bzr schema bug
[[ $BZR_EXIT -eq 0 ]] &&
  jq -e '."$schema" == "https://json-schema.org/draft/2020-12/schema"' \
    "$BZR_STDOUT" >/dev/null || _RR_SUPPLEMENT_OK=0
if [[ $_RR_SUPPLEMENT_OK -eq 1 ]]; then
  echo "    observed custom fields: $_RR_CUSTOM_FIELD_COUNT"
  test_pass
else
  test_fail "supplementary evidence was missing or malformed"
fi

_RR_REPORT="$FUNC_CONFIG_DIR/release-readiness-demo.md"
test_begin "123u. demo helper turns live evidence into a PM report"
if [[ -x "$REPO_ROOT/tools/run-release-readiness-demo.sh" ]] &&
  BZR_BIN="$BZR_BIN" "$REPO_ROOT/tools/run-release-readiness-demo.sh" \
    test "$_RR_MARKER" "$_RR_REPORT" &&
  grep -Fq '# Release readiness:' "$_RR_REPORT" &&
  grep -Fq '**Fact:**' "$_RR_REPORT" &&
  grep -Fq '**Assumption:**' "$_RR_REPORT" &&
  grep -Fq '**Assessment:** not ready' "$_RR_REPORT" &&
  grep -Fq '## Data limitations' "$_RR_REPORT" &&
  grep -Fq "#$_RR_ROOT" "$_RR_REPORT" &&
  grep -Fq "#$_RR_DEPENDENCY" "$_RR_REPORT"; then
  test_pass
else
  test_fail "demo helper did not produce the release-readiness report contract"
fi

test_begin "123v. release review leaves Bugzilla and local configuration unchanged"
run_bzr bug view "$_RR_ROOT"
cp "$BZR_STDOUT" "$FUNC_CONFIG_DIR/release-readiness-bug.after.json"
run_bzr bug history "$_RR_ROOT"
cp "$BZR_STDOUT" "$FUNC_CONFIG_DIR/release-readiness-history.after.json"
if cmp -s "$_RR_CONFIG_BEFORE" "$_RR_CONFIG" &&
  cmp -s "$_RR_BUG_BEFORE" "$FUNC_CONFIG_DIR/release-readiness-bug.after.json" &&
  cmp -s "$_RR_HISTORY_BEFORE" "$FUNC_CONFIG_DIR/release-readiness-history.after.json"; then
  test_pass
else
  test_fail "read-only release review changed config or Bugzilla evidence"
fi

unset _RR_MARKER _RR_PRODUCT _RR_VERSION _RR_QUERY _RR_URL_QUERY _RR_FIELDS _RR_CREATE
unset _RR_FIXTURE_OK _RR_DEPENDENCY _RR_COMPLETE _RR_ROOT _RR_URL
unset _RR_CONFIG _RR_CONFIG_BEFORE _RR_BUG_BEFORE _RR_HISTORY_BEFORE
unset _RR_SCOPES_OK _RR_SUPPLEMENT_OK _RR_CUSTOM_FIELD_COUNT _RR_REPORT
echo ""
