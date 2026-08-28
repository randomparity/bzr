#!/usr/bin/env bash
set -euo pipefail

skill_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
skill="$skill_root/SKILL.md"
skill_text=$(tr '\n' ' ' < "$skill")

fail() {
  printf 'skill contract failure: %s\n' "$1" >&2
  exit 1
}

require_literal() {
  local value=$1
  rg -Fq -- "$value" "$skill" || fail "missing: $value"
}

require_words() {
  local value=$1
  [[ "$skill_text" == *"$value"* ]] || fail "missing: $value"
}

first_collect=$(rg -n -m1 'scripts/collect\.py' "$skill" | cut -d: -f1)
depth_default=$(rg -n -m1 'depth 5' "$skill" | cut -d: -f1)
node_default=$(rg -n -m1 '200 nodes' "$skill" | cut -d: -f1)
[[ -n "$first_collect" && -n "$depth_default" && -n "$node_default" ]] \
  || fail 'DA-02 defaults or collection command are absent'
(( depth_default < first_collect && node_default < first_collect )) \
  || fail 'DA-02 defaults must precede the first collection command'

require_literal 'python3 "$SKILL_ROOT/scripts/collect.py" \'
require_literal '  --policy "$POLICY" \'
require_literal '  --output "$COLLECTION"'
require_literal 'python3 "$SKILL_ROOT/scripts/analyze.py" \'
require_literal '  --input "$COLLECTION" \'
require_literal '  --output "$ANALYSIS"'
require_literal 'python3 "$SKILL_ROOT/scripts/render.py" \'
require_literal '  --input "$ANALYSIS" \'
require_literal '  --format markdown \'
require_literal '  --format mermaid \'
require_literal '  --output "$REPORT"'

for command in 'bug view' 'bug list' 'bug search' 'query run'; do
  require_literal "$command"
done
for forbidden in 'bug links' '--paginate' '--count' '--permissive' \
  'bug create' 'bug update' 'bug clone' 'bug resolve' 'comment add' \
  'attachment upload' 'query save' 'query update'; do
  if rg -Fq -- "$forbidden" "$skill"; then
    fail "phantom or mutating command: $forbidden"
  fi
done

require_words 'server alias, scope kind, saved-query name, allowlisted parameter names, and collection command name'
require_words 'never includes parameter values, a literal Custom Search URL, credentials, raw server errors, or a full command line'
require_words 'Unknown and boundary nodes remain visible'
require_words 'longest dependency chain by edge count'
require_words 'Weighted critical-path analysis and delivery-date prediction are unsupported'
require_words 'refuse the request rather than mutate Bugzilla'
require_literal 'cycle.collection.json'
require_literal 'fixture-only cycle proof'

printf 'dependency-analysis skill contract: ok\n'
