#!/usr/bin/env bash
# shellcheck disable=SC2016 # Backticks below are literal Markdown delimiters.
set -euo pipefail

server=${1:?usage: run-release-readiness-demo.sh SERVER MARKER OUTPUT [TRACE]}
marker=${2:?usage: run-release-readiness-demo.sh SERVER MARKER OUTPUT [TRACE]}
output=${3:?usage: run-release-readiness-demo.sh SERVER MARKER OUTPUT [TRACE]}
BZR_BIN=${BZR_BIN:-bzr}
saved_query=release-readiness-demo
url_query=release-readiness-demo-url
fields=id,summary,status,priority,severity,keywords,flags,depends_on,last_change_time,whiteboard
discovery_fields=id,product,version,target_milestone,whiteboard
workdir=$(mktemp -d)
trace=${4:-$workdir/source-command-argv.jsonl}
trap 'rm -r "$workdir"' EXIT

for tool in "$BZR_BIN" jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "ERROR: $tool not found" >&2
    exit 1
  }
done

: >"$trace"

run_evidence() {
  local destination=$1
  local argument
  local -a normalized=(bzr --server '<server-profile>' --json)
  shift

  for argument in "$@"; do
    if [[ -n ${custom_url:-} && $argument == "$custom_url" ]]; then
      normalized+=("<credential-free-url>")
    else
      normalized+=("$argument")
    fi
  done
  jq -cn --args '$ARGS.positional' -- "${normalized[@]}" >>"$trace"
  "$BZR_BIN" --server "$server" --json "$@" >"$destination"
}

as_of=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
collection_start=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
stale_cutoff=$(jq -nr --arg as_of "$as_of" \
  '$as_of | fromdateiso8601 - (30 * 24 * 60 * 60) | todateiso8601')

run_evidence "$workdir/discovery.json" bug list --whiteboard "$marker" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$discovery_fields"
root=$(jq -er --arg marker "$marker" '
  [.data[] | select(.whiteboard == ($marker + " release-blocker")) | .id] |
  max
' "$workdir/discovery.json")

run_evidence "$workdir/root.json" bug view "$root" --fields "$discovery_fields"
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

run_evidence "$workdir/url-query.json" query show "$url_query"
custom_url=$(jq -er '.data.source_url' "$workdir/url-query.json")
run_evidence "$workdir/query.json" query show "$saved_query"
run_evidence "$workdir/custom.json" bug search --from-url "$custom_url" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$fields"
run_evidence "$workdir/saved.json" query run "$saved_query" --limit 100 \
  --paginate --sort bug_id --order asc --fields "$fields"
run_evidence "$workdir/milestone.json" bug list --product "$product" \
  "--target-milestone=$milestone" --limit 100 --paginate --sort bug_id \
  --order asc --fields "$fields"
run_evidence "$workdir/version.json" bug list --version "$version" --limit 100 \
  --paginate --sort bug_id --order asc --fields "$fields"
run_evidence "$workdir/product.json" bug list --product "$product" --limit 100 \
  --paginate --sort bug_id --order asc --fields "$fields"

for scope in custom saved milestone version product; do
  jq -e --argjson expected "$fixture_ids" '[.data[].id] == $expected' \
    "$workdir/$scope.json" >/dev/null
done

run_evidence "$workdir/links.json" bug links "$root" --relation depends_on
collection_end=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

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
  .last_change_time < $cutoff
)] | length' --arg cutoff "$stale_cutoff" "$workdir/product.json")
if [[ $blocker_count -gt 0 ]]; then
  assessment='not ready'
else
  assessment='no configured blocker observed'
fi

{
  printf '# Release readiness: product `%s`\n\n' "$product"
  printf 'Generated: %s\n\n' "$as_of"
  printf '## Scope and rules\n\n'
  printf -- '- **Fact:** Product scope; collection started %s and ended %s; ' \
    "$collection_start" "$collection_end"
  printf '%s visible bugs (#%s); bounded rolling collection.\n' \
    "$visible_count" "$visible_ids"
  printf -- '- **Assumption:** `RESOLVED` and `CLOSED` are complete. `Highest` priority or '
  printf 'literal `release-blocker` whiteboard text is release-blocking; there are no '
  printf 'severity, keyword, flag, or custom-field blocker rules.\n'
  printf -- '- **Assumption:** Dependency and stale checks are selected as non-blocking risks. '
  printf 'Dependency targets in `RESOLVED` or `CLOSED` are complete with no resolved-target '
  printf 'override. Stale means changed before %s (30 days before `as-of`); UTC is the ' \
    "$stale_cutoff"
  printf 'release-policy time zone.\n'
  printf -- '- **Assumption:** Deadline, ownership, milestone, status/resolution, and '
  printf 'history/regression checks are not selected. This is a complete zero-offset review, '
  printf 'the root cap is 100, and Markdown is the requested artifact.\n'
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
  printf -- '- **Fact:** %s stale open bugs under the stated assumptions.\n' "$stale_count"
  printf -- '- **Fact:** Ownership check: N/A (not selected).\n\n'
  printf '## Recent adverse changes\n\n'
  printf -- '- **Fact:** History/regression check: N/A (not selected); no history read was issued.\n\n'
  printf '## Decisions needed\n\n'
  printf -- '- **Assessment:** Decide whether #%s can be cleared and whether dependency #%s ' \
    "$root" "$dependency_ids"
  printf 'must close before the release proceeds.\n\n'
  printf '## Data limitations\n\n'
  printf -- '- **Fact:** Rolling snapshot, visible rows only. Deadline, ownership, milestone, '
  printf 'status/resolution, history/regression, and custom-field evidence is N/A because '
  printf 'those checks were not selected. Alternate saved-query, URL, milestone, and version '
  printf 'reads verify the fixture only; the assessment uses the product scope alone.\n\n'
  printf '## Source commands\n\n```text\n'
  jq -r 'join(" ")' "$trace"
  printf '```\n\n'
  printf '## Evidence appendix\n\n'
  printf -- '- **Fact:** Blocker IDs: #%s. Dependency-risk IDs: #%s. Visible IDs: #%s.\n' \
    "$blocker_ids" "$dependency_ids" "$visible_ids"
} >"$output"
