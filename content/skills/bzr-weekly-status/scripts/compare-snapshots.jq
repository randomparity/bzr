def sorted_unique: unique | sort;
def incompatibilities($old; $new; $required):
  ([if $old.format_version != $new.format_version then "format_version" else empty end,
    if $old.server != $new.server then "server" else empty end,
    if $old.scope_fingerprint != $new.scope_fingerprint then "scope" else empty end,
    if (($required - $old.fields) | length) > 0 then "fields" else empty end] | sorted_unique);
def ids($snapshot): ($snapshot.bugs | keys | sort);

($previous[0] // null) as $old |
($current[0] // error("current snapshot is required")) as $new |
if $old == null then
  {baseline: true, message: "No compatible prior snapshot exists; this report establishes the baseline.", added: [], removed_from_scope: [], changed: [], limitations: $new.limitations}
else
  incompatibilities($old; $new; $required_fields) as $bad |
  if ($bad | length) > 0 then error("incompatible snapshot: " + ($bad | join(", ")))
  else
    (ids($old)) as $old_ids | (ids($new)) as $new_ids |
    {baseline: false,
     added: ($new_ids - $old_ids),
     removed_from_scope: ($old_ids - $new_ids),
     changed: [($old_ids - ($old_ids - $new_ids))[] as $id |
       (($old.bugs[$id] | to_entries | from_entries) as $before |
        ($new.bugs[$id] | to_entries | from_entries) as $after |
        select($before != $after) | {id: $id, before: $before, after: $after})],
     limitations: $new.limitations}
  end
end
