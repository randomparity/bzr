#!/usr/bin/env bash
set -euo pipefail

skill_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
skill="$skill_root/SKILL.md"
skill_text=$(tr '\n' ' ' <"$skill")
repo_root=$(cd "$skill_root/../../.." && pwd -P)
documentation="$repo_root/docs/bzr-dependency-analysis.md"
recorder="$repo_root/tools/record-demo.sh"

normalized_commands=$(awk '
  /^```sh[[:space:]]*$/ { in_block = 1; next }
  in_block && /^```[[:space:]]*$/ { in_block = 0; next }
  in_block {
    line = $0
    sub(/^[[:space:]]+/, "", line)
    sub(/[[:space:]]+$/, "", line)
    if (line ~ /\\$/) {
      sub(/[[:space:]]*\\$/, "", line)
      command = command line " "
    } else if (line != "") {
      print command line
      command = ""
    }
  }
' "$skill")

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

require_file_literal() {
  local path=$1
  local value=$2
  rg -Fq -- "$value" "$path" || fail "missing from $path: $value"
}

expected_helper_commands=$(
  cat <<'EOF'
python3 "$SKILL_ROOT/scripts/collect.py" --policy "$POLICY" --output "$COLLECTION"
python3 "$SKILL_ROOT/scripts/analyze.py" --input "$COLLECTION" --output "$ANALYSIS"
python3 "$SKILL_ROOT/scripts/render.py" --input "$ANALYSIS" --format markdown --output "$REPORT"
python3 "$SKILL_ROOT/scripts/render.py" --input "$ANALYSIS" --format mermaid --output "$DIAGRAM"
python3 "$SKILL_ROOT/scripts/analyze.py" --input "$SKILL_ROOT/tests/fixtures/cycle.collection.json" --allow-partial --output "$ANALYSIS"
EOF
)
actual_helper_commands=$(printf '%s\n' "$normalized_commands" |
  awk '/^python3 "\$SKILL_ROOT\/scripts\//')
[[ "$actual_helper_commands" == "$expected_helper_commands" ]] ||
  fail 'collect/analyze/Markdown/Mermaid command blocks are not exact'

first_collect=$(rg -n -m1 'scripts/collect\.py' "$skill" | cut -d: -f1)
depth_default=$(rg -n -m1 'depth 5' "$skill" | cut -d: -f1)
node_default=$(rg -n -m1 '200 nodes' "$skill" | cut -d: -f1)
[[ -n "$first_collect" && -n "$depth_default" && -n "$node_default" ]] ||
  fail 'DA-02 defaults or collection command are absent'
((depth_default < first_collect && node_default < first_collect)) ||
  fail 'DA-02 defaults must precede the first collection command'

for command in 'bug view' 'bug list' 'bug search' 'query run'; do
  require_literal "$command"
done

resource_pattern='attachment|bug|classification|comment|component|config|field|group'
resource_pattern+='|product|query|server|template|user|whoami'
command_pattern="\`(?:bzr )?(?:${resource_pattern})"
command_pattern+="(?: [a-z][a-z-]*)?\`"
command_pattern+="|\\bbzr (?:${resource_pattern})"
command_pattern+='(?: [a-z][a-z-]*)?'
while IFS= read -r documented; do
  documented=${documented#\`}
  documented=${documented%\`}
  documented=${documented#bzr }
  case "$documented" in
  'bug view' | 'bug list' | 'bug search' | 'query run') ;;
  *) fail "non-allowlisted bzr command: $documented" ;;
  esac
done < <(rg -o "$command_pattern" "$skill")

require_words 'server alias, scope kind, saved-query name, allowlisted parameter names, and collection command name'
require_words 'never includes parameter values, a literal Custom Search URL, credentials, raw server errors, or a full command line'
require_words 'Unknown and boundary nodes remain visible'
require_words 'longest dependency chain by edge count'
require_words 'Weighted critical-path analysis and delivery-date prediction are unsupported'
require_words 'refuse the request rather than mutate Bugzilla'
require_literal 'cycle.collection.json'
require_literal 'fixture-only cycle proof'

require_file_literal "$recorder" '"${1:-}" == "dependency-analysis"'
require_file_literal "$recorder" \
  '[.data[] | select(.whiteboard == $marker) | .id] | max // empty'
require_file_literal "$recorder" \
  'dependency_workdir=$(cd "$dependency_workdir" && pwd -P)'
[[ -f "$documentation" ]] || fail "missing documentation: $documentation"
require_file_literal "$documentation" \
  '![Dependency analysis demo](assets/bzr-dependency-analysis-demo.gif)'
require_file_literal "$documentation" \
  '[Download the asciinema cast](assets/bzr-dependency-analysis-demo.cast)'

printf 'dependency-analysis skill contract: ok\n'
