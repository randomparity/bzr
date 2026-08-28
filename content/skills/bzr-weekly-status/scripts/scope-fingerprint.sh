#!/bin/sh
set -eu
canonical=$(jq -cS 'del(.name, .source_url, .created_at, .updated_at) | walk(if type == "array" then sort_by(tostring) else . end)')
if command -v sha256sum >/dev/null 2>&1; then
  printf '%s' "$canonical" | sha256sum | awk '{print $1}'
else
  printf '%s' "$canonical" | shasum -a 256 | awk '{print $1}'
fi
