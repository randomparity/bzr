def bug_map:
  map(select(.id != null) | {( .id | tostring): .}) | add // {};

{
  format_version: 1,
  created_at: $created_at,
  server: $server,
  scope_label: $scope_label,
  scope_fingerprint: $scope_fingerprint,
  fields: ($fields | unique | sort),
  rules: ($rules | if (.terminal_statuses | type) == "array" then .terminal_statuses |= (unique | sort) else . end),
  bugs: ((.data // .) | bug_map),
  limitations: []
}
