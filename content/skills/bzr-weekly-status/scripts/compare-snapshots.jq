def sorted_unique: unique | sort;
def incompatibilities($old; $new; $required):
  ([if $old.format_version != $new.format_version then "format_version" else empty end,
    if $old.server != $new.server then "server" else empty end,
    if $old.scope_fingerprint != $new.scope_fingerprint then "scope" else empty end,
    if $old.rules != $new.rules then "rules" else empty end,
    if (($required - $old.fields) | length) > 0 then "fields" else empty end] | sorted_unique);
def ids($snapshot): ($snapshot.bugs | keys | sort);
def terminal($bug; $rules): (($rules.terminal_statuses // []) | index($bug.status)) != null;
def changed_field($id; $before; $after; $field):
  select($before[$field] != $after[$field]) |
  {id: $id, field: $field, before: $before[$field], after: $after[$field]};
def age_days($at; $changed): (($at | fromdateiso8601) - ($changed | fromdateiso8601)) / 86400;

($previous[0] // null) as $old |
($current[0] // error("current snapshot is required")) as $new |
if $old == null then
  {baseline: true, message: "No compatible prior snapshot exists; this report establishes the baseline.", added: [], removed_from_scope: [], newly_opened: [], newly_resolved: [], transitions: [], stale_crossed: [], attention_unchanged: [], limitations: $new.limitations}
else
  incompatibilities($old; $new; $required_fields) as $bad |
  if ($bad | length) > 0 then error("incompatible snapshot: " + ($bad | join(", ")))
  else
    (ids($old)) as $old_ids | (ids($new)) as $new_ids |
    (($old_ids + $new_ids) | unique) as $all_ids |
    ($new.rules // {}) as $rules |
    ($rules.stale_after_days // null) as $stale_days |
    {baseline: false,
     added: ($new_ids - $old_ids),
     removed_from_scope: ($old_ids - $new_ids),
     newly_opened: [$all_ids[] as $id | select($old.bugs[$id] and $new.bugs[$id]) |
       select(terminal($old.bugs[$id]; $rules) and (terminal($new.bugs[$id]; $rules) | not)) | $id],
     newly_resolved: [$all_ids[] as $id | select($old.bugs[$id] and $new.bugs[$id]) |
       select((terminal($old.bugs[$id]; $rules) | not) and terminal($new.bugs[$id]; $rules)) | $id],
     transitions: [$all_ids[] as $id | select($old.bugs[$id] and $new.bugs[$id]) |
       $old.bugs[$id] as $before | $new.bugs[$id] as $after |
       ["status", "resolution", "assigned_to", "priority", "severity", "target_milestone",
        "deadline", "blocks", "depends_on", "whiteboard"][] as $field |
       changed_field($id; $before; $after; $field)],
     stale_crossed: [if $stale_days == null then empty else $all_ids[] as $id |
       select($old.bugs[$id].last_change_time and $new.bugs[$id].last_change_time) |
       select(age_days($old.created_at; $old.bugs[$id].last_change_time) < $stale_days and
              age_days($new.created_at; $new.bugs[$id].last_change_time) >= $stale_days) | $id end],
     attention_unchanged: [if $stale_days == null then empty else $all_ids[] as $id |
       select($old.bugs[$id] == $new.bugs[$id] and $new.bugs[$id].last_change_time) |
       select(age_days($new.created_at; $new.bugs[$id].last_change_time) >= $stale_days) | $id end],
     limitations: $new.limitations}
  end
end
