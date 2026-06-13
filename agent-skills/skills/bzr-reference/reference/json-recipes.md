# bzr --json + jq recipes

All read paths support `--json`. Pipe to `jq`.

```
# Bug ids from a search
bzr bug search "memory leak" --json | jq -r '.[].id'

# A single bug's assignee and status
bzr bug view 12345 --json | jq -r '.assigned_to, .status'

# Attachment file names on a bug
bzr attachment list 12345 --json | jq -r '.[].file_name'

# Component names of a product
bzr product view Fedora --json | jq -r '.components[].name'

# Which status values a bug can move to
bzr field list status --json | jq '.[] | select(.name=="NEW") | .can_change_to'

# Count of your open bugs
bzr bug my --status \!CLOSED --json | jq 'length'
```

Note: multi-id `bug view --json` may wrap results as `{ "bugs": [ ... ] }`;
single-id output is the object directly. Check with `jq 'has("bugs")'` if unsure.
