#!/usr/bin/env bash
# shellcheck disable=SC2016 # Backticks below are literal Markdown delimiters.
set -euo pipefail

server=${1:?usage: run-release-readiness-demo.sh SERVER MARKER OUTPUT}
marker=${2:?usage: run-release-readiness-demo.sh SERVER MARKER OUTPUT}
output=${3:?usage: run-release-readiness-demo.sh SERVER MARKER OUTPUT}
BZR_BIN=${BZR_BIN:-bzr}
saved_query=release-readiness-demo
url_query=release-readiness-demo-url
fields=id,summary,status,priority,severity,assigned_to,target_milestone,version,deadline,last_change_time,whiteboard,depends_on
discovery_fields=$fields,product
workdir=$(mktemp -d)
trap 'rm -r "$workdir"' EXIT

for tool in "$BZR_BIN" jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "ERROR: $tool not found" >&2
    exit 1
  }
done

bzr_json=("$BZR_BIN" --server "$server" --json)
"${bzr_json[@]}" bug list --whiteboard "$marker" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$discovery_fields" >"$workdir/discovery.json"
root=$(jq -er --arg marker "$marker" '
  [.data[] | select(.whiteboard == ($marker + " release-blocker")) | .id] |
  max
' "$workdir/discovery.json")

"${bzr_json[@]}" bug view "$root" --fields "$discovery_fields" >"$workdir/root.json"
product=$(jq -er '.data.product' "$workdir/root.json")
version=$(jq -er '.data.version' "$workdir/root.json")
milestone=$(jq -er '.data.target_milestone' "$workdir/root.json")
fixture_ids=$(jq -cer --arg marker "$marker" --arg product "$product" '
  [.data[] | select(
    .product == $product and (.whiteboard | startswith($marker + " "))
  ) | .id] | sort | select(length == 3)
' "$workdir/discovery.json")
for value in "$product" "$version" "$milestone"; do
  [[ $value =~ ^[A-Za-z0-9._-]+$ ]] || {
    echo "ERROR: demo fixture contains a non-portable scope value" >&2
    exit 1
  }
done

"${bzr_json[@]}" query show "$url_query" >"$workdir/url-query.json"
custom_url=$(jq -er '.data.source_url' "$workdir/url-query.json")
"${bzr_json[@]}" query show "$saved_query" >"$workdir/query.json"
"${bzr_json[@]}" bug search --from-url "$custom_url" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$fields" >"$workdir/custom.json"
"${bzr_json[@]}" query run "$saved_query" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$fields" >"$workdir/saved.json"
"${bzr_json[@]}" bug list --product "$product" "--target-milestone=$milestone" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$fields" \
  >"$workdir/milestone.json"
"${bzr_json[@]}" bug list --version "$version" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$fields" >"$workdir/version.json"
"${bzr_json[@]}" bug list --product "$product" --limit 100 --paginate \
  --sort bug_id --order asc --fields "$fields" >"$workdir/product.json"

for scope in custom saved milestone version product; do
  jq -e --argjson expected "$fixture_ids" '[.data[].id] == $expected' \
    "$workdir/$scope.json" >/dev/null
done

"${bzr_json[@]}" bug history "$root" --since 2020-01-01 >"$workdir/history.json"
"${bzr_json[@]}" bug links "$root" --relation depends_on >"$workdir/links.json"
"${bzr_json[@]}" field list status >"$workdir/status-field.json"
"${bzr_json[@]}" server capabilities >"$workdir/capabilities.json"
"${bzr_json[@]}" schema bug >"$workdir/schema.json"

visible_count=$(jq -er '.data | length' "$workdir/product.json")
visible_ids=$(jq -er '[.data[].id] | join(", #")' "$workdir/product.json")
blocker_count=$(jq -er '[.data[] | select(
  (.status != "RESOLVED" and .status != "CLOSED") and
  (.priority == "Highest" or (.whiteboard | contains("release-blocker")))
)] | length' "$workdir/product.json")
blocker_ids=$(jq -er '[.data[] | select(
  (.status != "RESOLVED" and .status != "CLOSED") and
  (.priority == "Highest" or (.whiteboard | contains("release-blocker")))
) | .id] | join(", #")' "$workdir/product.json")
dependency_count=$(jq -er '[.data[] | select(
  .relation == "depends_on" and .status != "RESOLVED" and .status != "CLOSED"
)] | length' "$workdir/links.json")
dependency_ids=$(jq -er '[.data[] | select(
  .relation == "depends_on" and .status != "RESOLVED" and .status != "CLOSED"
) | .id] | join(", #")' "$workdir/links.json")
stale_count=$(jq -er '[.data[] | select(
  (.status != "RESOLVED" and .status != "CLOSED") and
  .last_change_time < "2026-07-30T16:00:00Z"
)] | length' "$workdir/product.json")
unowned_count=$(jq -er '[.data[] | select(
  (.status != "RESOLVED" and .status != "CLOSED") and
  (.assigned_to == null or .assigned_to == "")
)] | length' "$workdir/product.json")
custom_field_count=$(jq -er '.data.custom_fields | length' "$workdir/capabilities.json")
history_count=$(jq -er '[.data[] | select(.field == "status")] | length' \
  "$workdir/history.json")

if [[ $blocker_count -gt 0 ]]; then
  assessment='not ready'
else
  assessment='no configured blocker observed'
fi

{
  printf '# Release readiness: product `%s`\n\n' "$product"
  printf 'Generated: 2026-08-29T16:00:00Z\n\n'
  printf '## Scope and rules\n\n'
  printf -- '- **Fact:** Product scope; %s visible bugs (#%s); bounded rolling collection.\n' \
    "$visible_count" "$visible_ids"
  printf -- '- **Assumption:** `RESOLVED` and `CLOSED` are complete; `Highest` priority or '
  printf '`release-blocker` whiteboard text is blocking; stale means 30 days; time zone is UTC.\n'
  printf -- '- **Fact:** Authorization can hide bugs, so this report does not claim an unobservable total.\n\n'
  printf '## Readiness assessment\n\n'
  printf -- '- **Assessment:** %s.\n' "$assessment"
  printf -- '- **Fact:** %s/%s visible bugs match a configured blocker (#%s).\n\n' \
    "$blocker_count" "$visible_count" "$blocker_ids"
  printf '## Blockers\n\n'
  printf -- '- **Fact:** #%s is open with `Highest` priority and the configured whiteboard marker.\n\n' \
    "$blocker_ids"
  printf '## Dependency risks\n\n'
  printf -- '- **Fact:** %s unresolved outgoing dependency (#%s) blocks #%s.\n\n' \
    "$dependency_count" "$dependency_ids" "$root"
  printf '## Stale or unowned work\n\n'
  printf -- '- **Fact:** %s stale and %s unowned open bugs under the stated assumptions.\n\n' \
    "$stale_count" "$unowned_count"
  printf '## Recent adverse changes\n\n'
  printf -- '- **Fact:** #%s has %s visible status-change record since 2020-01-01.\n\n' \
    "$root" "$history_count"
  printf '## Decisions needed\n\n'
  printf -- '- **Assessment:** Decide whether #%s can be cleared and whether dependency #%s ' \
    "$root" "$dependency_ids"
  printf 'must close before the release proceeds.\n\n'
  printf '## Data limitations\n\n'
  printf -- '- **Fact:** Rolling snapshot, visible rows only; custom fields observed: %s; ' \
    "$custom_field_count"
  printf 'no custom-field rule was assumed.\n\n'
  printf '## Source commands\n\n```text\n'
  printf 'bzr --server <server-profile> bug search --from-url <credential-free-url> --limit 100 --paginate --json --sort bug_id --order asc --fields %s\n' "$fields"
  printf 'bzr --server <server-profile> query show %s --json\n' "$saved_query"
  printf 'bzr --server <server-profile> query run %s --limit 100 --paginate --json --sort bug_id --order asc --fields %s\n' "$saved_query" "$fields"
  printf 'bzr --server <server-profile> bug list --product %s --target-milestone %s --limit 100 --paginate --json --sort bug_id --order asc --fields %s\n' "$product" "$milestone" "$fields"
  printf 'bzr --server <server-profile> bug list --version %s --limit 100 --paginate --json --sort bug_id --order asc --fields %s\n' "$version" "$fields"
  printf 'bzr --server <server-profile> bug list --product %s --limit 100 --paginate --json --sort bug_id --order asc --fields %s\n' "$product" "$fields"
  printf 'bzr --server <server-profile> bug view %s --json --fields %s\n' "$root" "$fields"
  printf 'bzr --server <server-profile> bug history %s --since 2020-01-01 --json\n' "$root"
  printf 'bzr --server <server-profile> bug links %s --relation depends_on --json\n' "$root"
  printf 'bzr --server <server-profile> field list status --json\n'
  printf 'bzr --server <server-profile> server capabilities --json\n'
  printf 'bzr --server <server-profile> schema bug\n```\n\n'
  printf '## Evidence appendix\n\n'
  printf -- '- **Fact:** Blocker IDs: #%s. Dependency-risk IDs: #%s. Visible IDs: #%s.\n' \
    "$blocker_ids" "$dependency_ids" "$visible_ids"
} >"$output"
