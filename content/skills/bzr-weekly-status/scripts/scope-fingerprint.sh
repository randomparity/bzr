#!/bin/sh
set -eu
canonical=$(jq -cS '
  del(.name, .source_url, .created_at, .updated_at) |
  ["product", "component", "status", "assignee", "creator", "priority", "severity",
   "whiteboard", "target_milestone", "version", "op_sys", "platform", "resolution",
   "qa_contact", "url"] as $sets |
  reduce $sets[] as $key (.; if (.[$key] | type) == "array" then .[$key] |= sort else . end) |
  if (.raw_params | type) == "array" then .raw_params |= sort_by(.[0], .[1]) else . end
')
if command -v sha256sum >/dev/null 2>&1; then
  printf '%s' "$canonical" | sha256sum | awk '{print $1}'
else
  printf '%s' "$canonical" | shasum -a 256 | awk '{print $1}'
fi
