#!/bin/sh
set -eu
current=$1
runs=$2
required_fields=$3
best_path=
best_key=
for candidate in "$runs"/*/snapshot.json; do
	[ -f "$candidate" ] || continue
	[ "$candidate" != "$current" ] || continue
	if jq -e --slurpfile current "$current" --argjson required "$required_fields" '
    .format_version == $current[0].format_version and .server == $current[0].server and
    .scope_fingerprint == $current[0].scope_fingerprint and
    ((.rules | if (.terminal_statuses | type) == "array" then .terminal_statuses |= (unique | sort) else . end) ==
     ($current[0].rules | if (.terminal_statuses | type) == "array" then .terminal_statuses |= (unique | sort) else . end)) and
    (($required - .fields) | length) == 0
  ' "$candidate" >/dev/null; then
		key=$(jq -r '.created_at' "$candidate")/$(dirname "$candidate" | sed 's|.*/||')
		newer=$(jq -nr --arg candidate "$key" --arg current "$best_key" '$candidate > $current')
		if [ -z "$best_key" ] || [ "$newer" = true ]; then
			best_key=$key
			best_path=$candidate
		fi
	fi
done
[ -n "$best_path" ] && printf '%s\n' "$best_path"
