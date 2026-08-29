#!/usr/bin/env bash
# shellcheck disable=SC2016 # Backticks below are literal Markdown delimiters.
set -euo pipefail

server=${1:?usage: run-release-readiness-demo.sh SERVER MARKER ROOT PRODUCT OUTPUT [TRACE]}
marker=${2:?usage: run-release-readiness-demo.sh SERVER MARKER ROOT PRODUCT OUTPUT [TRACE]}
root=${3:?usage: run-release-readiness-demo.sh SERVER MARKER ROOT PRODUCT OUTPUT [TRACE]}
product=${4:?usage: run-release-readiness-demo.sh SERVER MARKER ROOT PRODUCT OUTPUT [TRACE]}
output=${5:?usage: run-release-readiness-demo.sh SERVER MARKER ROOT PRODUCT OUTPUT [TRACE]}
BZR_BIN=${BZR_BIN:-bzr}
saved_query=release-readiness-demo
url_query=release-readiness-demo-url
fields=id,summary,status,priority,depends_on,last_change_time,whiteboard
discovery_fields=id,product,version,target_milestone,whiteboard
workdir=$(mktemp -d)
trace=${6:-$workdir/source-command-argv.jsonl}
trap 'rm -r "$workdir"' EXIT

[[ $root =~ ^[1-9][0-9]*$ ]] || {
  echo "ERROR: demo fixture root must be a positive bug ID" >&2
  exit 1
}
[[ $product =~ ^[A-Za-z0-9._-]+$ ]] || {
  echo "ERROR: demo fixture product is not portable" >&2
  exit 1
}

for tool in "$BZR_BIN" jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "ERROR: $tool not found" >&2
    exit 1
  }
done

: >"$trace"

run_evidence() {
  local label=$1
  local destination=$2
  local argument
  local -a normalized=(bzr --server '<server-profile>' --json)
  shift 2

  for argument in "$@"; do
    if [[ -n ${custom_url:-} && $argument == "$custom_url" ]]; then
      normalized+=("<credential-free-url>")
    else
      normalized+=("$argument")
    fi
  done
  jq -cn --arg label "$label" --args \
    '$ARGS.positional as $argv | {label: $label, argv: $argv}' -- \
    "${normalized[@]}" >>"$trace"
  "$BZR_BIN" --server "$server" --json "$@" >"$destination"
}

as_of=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
collection_start=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
stale_cutoff=$(jq -nr --arg as_of "$as_of" \
  '$as_of | fromdateiso8601 - (30 * 24 * 60 * 60) | todateiso8601')

run_evidence marker-discovery "$workdir/discovery.json" bug list --whiteboard "$marker" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$discovery_fields"
discovered_root=$(jq -er --arg marker "$marker" '
  [.data[] | select(.whiteboard == ($marker + " release-blocker")) | .id] |
  max
' "$workdir/discovery.json")
discovered_product=$(jq -er --arg marker "$marker" --argjson root "$discovered_root" '
  [.data[] | select(
    .id == $root and .whiteboard == ($marker + " release-blocker")
  ) | .product] | unique | select(length == 1) | .[0]
' "$workdir/discovery.json")
if [[ $discovered_root != "$root" || $discovered_product != "$product" ]]; then
  echo "ERROR: demo fixture identity changed after the visible scope was resolved" >&2
  exit 1
fi

run_evidence root-detail "$workdir/root.json" bug view "$root" --fields "$discovery_fields"
jq -e --argjson root "$root" --arg product "$product" '
  .data.id == $root and .data.product == $product
' "$workdir/root.json" >/dev/null || {
  echo "ERROR: demo root detail no longer matches the resolved fixture identity" >&2
  exit 1
}
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

run_evidence url-query-preflight "$workdir/url-query.json" query show "$url_query"
custom_url=$(jq -er '.data.source_url' "$workdir/url-query.json")
run_evidence saved-query-preflight "$workdir/query.json" query show "$saved_query"
run_evidence custom-scope-check "$workdir/custom.json" bug search --from-url "$custom_url" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$fields"
run_evidence saved-scope-check "$workdir/saved.json" query run "$saved_query" --limit 100 \
  --paginate --sort bug_id --order asc --fields "$fields"
run_evidence milestone-scope-check "$workdir/milestone.json" bug list --product "$product" \
  "--target-milestone=$milestone" --limit 100 --paginate --sort bug_id \
  --order asc --fields "$fields"
run_evidence version-scope-check "$workdir/version.json" bug list --version "$version" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$fields"
run_evidence product-scope "$workdir/product.json" bug list --product "$product" \
  --limit 100 --paginate --sort bug_id --order asc --fields "$fields"

for scope in custom saved milestone version product; do
  jq -e --argjson expected "$fixture_ids" '[.data[].id] == $expected' \
    "$workdir/$scope.json" >/dev/null
done

run_evidence dependency-links "$workdir/links.json" bug links "$root" \
  --relation depends_on
collection_end=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

summary="$workdir/summary.json"
jq -cer --arg cutoff "$stale_cutoff" --slurpfile links "$workdir/links.json" '
  def nonempty_string: type == "string" and length > 0;
  def status_known: (.status? | nonempty_string);
  def complete:
    status_known and (.status == "RESOLVED" or .status == "CLOSED");
  def open: status_known and (complete | not);
  def priority_known: (.priority? | nonempty_string);
  def whiteboard_known: (.whiteboard? | type == "string");
  def blocker_match:
    if open then
      (.priority? == "Highest") or
      (if whiteboard_known then .whiteboard | contains("release-blocker") else false end)
    else false end;
  def blocker_unknown:
    if (status_known | not) then true
    elif complete then false
    else ((priority_known and whiteboard_known) | not) end;
  def timestamp_epoch: try (.last_change_time? | fromdateiso8601) catch null;
  def stale_match($cutoff_epoch):
    if open then
      timestamp_epoch as $changed |
      if ($changed | type) == "number" then $changed < $cutoff_epoch else false end
    else false end;
  def stale_unknown:
    if (status_known | not) then true
    elif complete then false
    else (timestamp_epoch | type) != "number" end;
  def dependency_status_known: (.status? | nonempty_string);
  def dependency_unresolved:
    dependency_status_known and .status != "RESOLVED" and .status != "CLOSED";
  def format_ids:
    if length == 0 then "(none)" else map("#" + tostring) | join(", ") end;
  ($cutoff | fromdateiso8601) as $cutoff_epoch |
  .data as $bugs |
  ($links[0].data | map(select(.relation == "depends_on"))) as $dependencies |
  ($bugs | map(select(blocker_match)) | map(.id)) as $blockers |
  ($bugs | map(select(blocker_unknown)) | map(.id)) as $unknown_blockers |
  ($bugs | map(select(stale_match($cutoff_epoch))) | map(.id)) as $stale |
  ($bugs | map(select(stale_unknown)) | map(.id)) as $unknown_stale |
  ($dependencies | map(select(dependency_unresolved)) | map(.id)) as $unresolved |
  ($dependencies | map(select(dependency_status_known | not)) | map(.id)) as $unknown_dependencies |
  {
    visible_count: ($bugs | length),
    visible_ids: ($bugs | map(.id) | format_ids),
    visible_sample: ($bugs | map(.id)[0:5] | format_ids),
    blocker_count: ($blockers | length),
    blocker_ids: ($blockers | format_ids),
    blocker_sample: ($blockers[0:5] | format_ids),
    blocker_unknown_count: ($unknown_blockers | length),
    blocker_unknown_ids: ($unknown_blockers | format_ids),
    blocker_unknown_sample: ($unknown_blockers[0:5] | format_ids),
    dependency_total: ($dependencies | length),
    dependency_count: ($unresolved | length),
    dependency_ids: ($unresolved | format_ids),
    dependency_sample: ($unresolved[0:5] | format_ids),
    dependency_unknown_count: ($unknown_dependencies | length),
    dependency_unknown_ids: ($unknown_dependencies | format_ids),
    dependency_unknown_sample: ($unknown_dependencies[0:5] | format_ids),
    stale_count: ($stale | length),
    stale_ids: ($stale | format_ids),
    stale_sample: ($stale[0:5] | format_ids),
    stale_unknown_count: ($unknown_stale | length),
    stale_unknown_ids: ($unknown_stale | format_ids),
    stale_unknown_sample: ($unknown_stale[0:5] | format_ids)
  }
' "$workdir/product.json" >"$summary"
IFS=$'\t' read -r visible_count visible_ids visible_sample \
  blocker_count blocker_ids blocker_sample blocker_unknown_count \
  blocker_unknown_ids blocker_unknown_sample dependency_total dependency_count \
  dependency_ids dependency_sample dependency_unknown_count dependency_unknown_ids \
  dependency_unknown_sample stale_count stale_ids stale_sample stale_unknown_count \
  stale_unknown_ids stale_unknown_sample < <(jq -r '[
    .visible_count, .visible_ids, .visible_sample,
    .blocker_count, .blocker_ids, .blocker_sample,
    .blocker_unknown_count, .blocker_unknown_ids, .blocker_unknown_sample,
    .dependency_total, .dependency_count, .dependency_ids, .dependency_sample,
    .dependency_unknown_count, .dependency_unknown_ids, .dependency_unknown_sample,
    .stale_count, .stale_ids, .stale_sample,
    .stale_unknown_count, .stale_unknown_ids, .stale_unknown_sample
  ] | @tsv' "$summary")
if [[ $blocker_count -gt 0 ]]; then
  assessment='not ready'
elif [[ $blocker_unknown_count -gt 0 ]]; then
  assessment='indeterminate'
else
  assessment='no configured blocker observed'
fi

{
  printf '# Release readiness: product `%s`\n\n' "$product"
  printf 'Generated: %s\n\n' "$as_of"
  printf '## Scope and rules\n\n'
  printf -- '- **Fact:** Product scope; collection started %s and ended %s; ' \
    "$collection_start" "$collection_end"
  printf '%s visible bugs. Bounded sample: %s. Source: `product-scope`; ' \
    "$visible_count" "$visible_sample"
  printf 'bounded rolling collection.\n'
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
  printf -- '- **Fact:** %s/%s visible bugs are known to match a configured blocker. ' \
    "$blocker_count" "$visible_count"
  printf 'Bounded sample: %s. Source: `product-scope`.\n' "$blocker_sample"
  printf -- '- **Fact:** %s/%s visible bugs have unknown blocker evidence. ' \
    "$blocker_unknown_count" "$visible_count"
  printf 'Bounded sample: %s. Source: `product-scope`.\n\n' \
    "$blocker_unknown_sample"
  printf '## Blockers\n\n'
  if [[ $blocker_count -gt 0 ]]; then
    printf -- '- **Fact:** Known blocker IDs %s are open and match at least one configured blocker rule. Source: `product-scope`.\n\n' \
      "$blocker_ids"
  else
    printf -- '- **Fact:** No visible bug is known to match a configured blocker. Source: `product-scope`.\n\n'
  fi
  printf '## Dependency risks\n\n'
  printf -- '- **Fact:** %s/%s visible outgoing dependencies are known unresolved. ' \
    "$dependency_count" "$dependency_total"
  printf 'Bounded sample: %s. Source: `dependency-links`.' "$dependency_sample"
  if [[ $dependency_count -gt 0 ]]; then
    printf ' It affects #%s.\n' "$root"
  else
    printf '\n'
  fi
  printf -- '- **Fact:** %s/%s visible outgoing dependencies have unknown status. ' \
    "$dependency_unknown_count" "$dependency_total"
  printf 'Bounded sample: %s. Source: `dependency-links`.\n' \
    "$dependency_unknown_sample"
  if [[ $dependency_count -eq 0 ]]; then
    printf -- '- **Fact:** No visible outgoing dependency is known unresolved. Source: `dependency-links`.\n'
  fi
  printf '\n'
  printf '## Stale or unowned work\n\n'
  printf -- '- **Fact:** %s/%s visible bugs are known stale under the stated assumptions. ' \
    "$stale_count" "$visible_count"
  printf 'Bounded sample: %s. Source: `product-scope`.\n' "$stale_sample"
  printf -- '- **Fact:** %s/%s visible bugs have unknown stale evidence. ' \
    "$stale_unknown_count" "$visible_count"
  printf 'Bounded sample: %s. Source: `product-scope`.\n' "$stale_unknown_sample"
  printf -- '- **Fact:** Ownership check: N/A (not selected).\n\n'
  printf '## Recent adverse changes\n\n'
  printf -- '- **Fact:** History/regression check: N/A (not selected); no history read was issued.\n\n'
  printf '## Decisions needed\n\n'
  if [[ $blocker_count -gt 0 ]]; then
    printf -- '- **Assessment:** Decide whether blocker %s can be cleared before the release proceeds.\n' \
      "$blocker_ids"
  fi
  if [[ $dependency_count -gt 0 ]]; then
    printf -- '- **Assessment:** Decide whether dependency %s must close before the release proceeds.\n' \
      "$dependency_ids"
  fi
  if [[ $blocker_unknown_count -gt 0 || $dependency_unknown_count -gt 0 ||
    $stale_unknown_count -gt 0 ]]; then
    printf -- '- **Assessment:** Resolve the unknown selected evidence before relying on the affected checks.\n'
  elif [[ $blocker_count -eq 0 && $dependency_count -eq 0 ]]; then
    printf -- '- **Assessment:** No blocker or dependency decision is supported by known visible evidence.\n'
  fi
  printf '\n'
  printf '## Data limitations\n\n'
  printf -- '- **Fact:** Rolling snapshot, visible rows only. Deadline, ownership, milestone, '
  printf 'status/resolution, history/regression, and custom-field evidence is N/A because '
  printf 'those checks were not selected. Alternate saved-query, URL, milestone, and version '
  printf 'reads verify the fixture only; the assessment uses the product scope alone.\n'
  if [[ $blocker_unknown_count -gt 0 || $dependency_unknown_count -gt 0 ||
    $stale_unknown_count -gt 0 ]]; then
    printf -- '- **Fact:** Required evidence is unknown for blocker IDs %s, stale IDs %s, and dependency IDs %s. Unknown rows were not classified as matches or non-matches for the affected checks.\n\n' \
      "$blocker_unknown_ids" "$stale_unknown_ids" "$dependency_unknown_ids"
  else
    printf -- '- **Fact:** No selected blocker, dependency, or stale evidence was unknown in visible rows.\n\n'
  fi
  printf '## Source commands\n\n```text\n'
  jq -r '"[\(.label)] \(.argv | join(" "))"' "$trace"
  printf '```\n\n'
  printf '## Evidence appendix\n\n'
  printf -- '- **Fact:** Blocker IDs: %s. Stale IDs: %s. Dependency-risk IDs: %s. Visible IDs: %s. Sources: `product-scope`, `dependency-links`.\n' \
    "$blocker_ids" "$stale_ids" "$dependency_ids" "$visible_ids"
  printf -- '- **Fact:** Unknown blocker IDs: %s. Unknown stale IDs: %s. Unknown dependency-risk IDs: %s. Sources: `product-scope`, `dependency-links`.\n' \
    "$blocker_unknown_ids" "$stale_unknown_ids" "$dependency_unknown_ids"
} >"$output"
