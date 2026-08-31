#!/bin/sh
set -eu

root=$1
run_id=$2
staging=$3

case "$run_id" in
'' | *[!A-Za-z0-9._-]*)
	echo "publish-run: invalid run id" >&2
	exit 2
	;;
esac
mkdir -p "$root/.staging" "$root/runs"
root=$(CDPATH='' cd -- "$root" && pwd -P)
[ -d "$staging" ] || {
	echo "publish-run: staging directory not found" >&2
	exit 2
}
staging=$(CDPATH='' cd -- "$staging" && pwd -P)
[ "$(dirname "$staging")" = "$root/.staging" ] || {
	echo "publish-run: staging must be below $root/.staging" >&2
	exit 2
}
[ -f "$staging/snapshot.json" ] || {
	echo "publish-run: snapshot.json not found" >&2
	exit 2
}
if find "$staging" -type l -print -quit | grep -q .; then
	echo "publish-run: staging directory must not contain symlinks" >&2
	exit 2
fi
if ! jq -e '
  .fields as $allowed |
  (keys - ["bugs", "bzr_schema_version", "created_at", "fields", "format_version",
           "limitations", "rules", "scope_fingerprint", "scope_label", "server"] | length) == 0 and
  .format_version == 1 and
  (.created_at | type == "string") and
  (.created_at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")) and
  (try (.created_at | fromdateiso8601 | type == "number") catch false) and
  (.server | type == "string" and length > 0) and
  (.scope_label | type == "string" and length > 0) and
  (.scope_fingerprint | test("^[0-9a-f]{64}$")) and
  (.fields | type == "array" and length > 0) and
  (all($allowed[]; type == "string" and length > 0)) and
  (all($allowed[]; test("^(id|summary|status|resolution|assigned_to|priority|severity|target_milestone|deadline|last_change_time|whiteboard|blocks|depends_on|cf_[A-Za-z0-9_]+)$"))) and
  (($allowed | unique | length) == ($allowed | length)) and
  (.rules | type == "object") and
  ((.rules | keys) - ["stale_after_days", "terminal_statuses"] | length) == 0 and
  ((.rules.terminal_statuses // []) | type == "array") and
  (all((.rules.terminal_statuses // [])[]; type == "string" and length > 0)) and
  (((.rules.terminal_statuses // []) | unique | length) == ((.rules.terminal_statuses // []) | length)) and
  ((.rules.stale_after_days // 1) | type == "number" and . >= 1 and floor == .) and
  ((.bzr_schema_version // null) | type == "null" or type == "string") and
  (.bugs | type == "object") and
  (all(.bugs | to_entries[];
    (.key | test("^[1-9][0-9]*$")) and
    (.value | type == "object") and
    (all(.value | keys[]; test("^(id|summary|status|resolution|assigned_to|priority|severity|target_milestone|deadline|last_change_time|whiteboard|blocks|depends_on|cf_[A-Za-z0-9_]+)$"))) and
    (.value.id | type == "number" and floor == . and . >= 1) and
    (.value.id | tostring) == .key)) and
  (.limitations | type == "array") and
  (all(.limitations[];
    (keys - ["id", "reason"] | length) == 0 and
    ((.id // null) | type == "null" or (type == "number" and floor == . and . >= 1)) and
    (.reason | type == "string" and length > 0))) and
  ([.. | objects | keys[] | ascii_downcase] |
    map(select(. == "api_key" or . == "bugzilla_api_key" or . == "token" or
               . == "password" or . == "comments" or . == "attachments")) | length) == 0
' "$staging/snapshot.json" >/dev/null; then
	echo "publish-run: snapshot.json does not match snapshot-v1.schema.json" >&2
	exit 2
fi

final="$root/runs/$run_id"
[ ! -e "$final" ] || {
	echo "publish-run: run already exists: $run_id" >&2
	exit 2
}
chmod -R go-rwx "$staging"
mv "$staging" "$final"

tmp_link="$root/.latest.$run_id.$$"
trap 'rm -f "$tmp_link"' EXIT HUP INT TERM
[ ! -d "$root/latest" ] || [ -L "$root/latest" ] || {
	echo "publish-run: latest is a directory; new run retained: $final" >&2
	exit 2
}
ln -s "runs/$run_id" "$tmp_link"
if ! mv -fh "$tmp_link" "$root/latest" 2>/dev/null &&
	! mv -fT "$tmp_link" "$root/latest"; then
	echo "publish-run: could not update latest; new run retained: $final" >&2
	exit 2
fi
trap - EXIT HUP INT TERM
printf '%s\n' "$final"
