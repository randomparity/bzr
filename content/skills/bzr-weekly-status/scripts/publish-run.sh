#!/bin/sh
set -eu

root=$1
run_id=$2
staging=$3

case "$run_id" in
  ''|*[!A-Za-z0-9._-]*) echo "publish-run: invalid run id" >&2; exit 2 ;;
esac
mkdir -p "$root/.staging" "$root/runs"
root=$(CDPATH= cd -- "$root" && pwd -P)
[ -d "$staging" ] || { echo "publish-run: staging directory not found" >&2; exit 2; }
staging=$(CDPATH= cd -- "$staging" && pwd -P)
[ "$(dirname "$staging")" = "$root/.staging" ] || { echo "publish-run: staging must be below $root/.staging" >&2; exit 2; }
[ -f "$staging/snapshot.json" ] || { echo "publish-run: snapshot.json not found" >&2; exit 2; }
jq -e '
  .format_version == 1 and
  (.created_at | type == "string") and
  (.server | type == "string" and length > 0) and
  (.scope_label | type == "string" and length > 0) and
  (.scope_fingerprint | test("^[0-9a-f]{64}$")) and
  (.fields | type == "array") and (.rules | type == "object") and
  (.bugs | type == "object") and (.limitations | type == "array")
' "$staging/snapshot.json" >/dev/null

final="$root/runs/$run_id"
[ ! -e "$final" ] || { echo "publish-run: run already exists: $run_id" >&2; exit 2; }
chmod -R go-rwx "$staging"
mv "$staging" "$final"

tmp_link="$root/.latest.$run_id.$$"
trap 'rm -f "$tmp_link"' EXIT HUP INT TERM
[ ! -d "$root/latest" ] || { echo "publish-run: latest is a directory" >&2; exit 2; }
ln -s "runs/$run_id" "$tmp_link"
mv -f "$tmp_link" "$root/latest"
trap - EXIT HUP INT TERM
printf '%s\n' "$final"
