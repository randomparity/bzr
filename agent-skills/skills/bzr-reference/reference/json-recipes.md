# bzr --json + jq recipes

All read paths support `--json`. Pipe to `jq`.

```
# Public read-only Bugzilla, no config or API key
bzr --server-url https://bugzilla.example.com server info --json | jq -r '.version'

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

# Count of your open bugs (client side)
bzr bug my --status \!CLOSED --json | jq 'length'

# Match count without fetching the rows (server side, cheaper)
bzr bug list --product Foo --status NEW --count --json | jq '.count'
```

## Streaming with `--output ndjson`

`--output ndjson` emits one compact JSON record per line for list results — ideal
for large result sets and line-oriented tooling (`jq -c`, `grep`, `while read`):

```
# One bug per line; pull two fields from each
bzr bug list --product Foo --status NEW --output ndjson | jq -c '{id, summary}'

# Stream a saved query and act per row as it arrives
bzr query run my-open --output ndjson | while IFS= read -r line; do
  echo "$line" | jq -r '"#\(.id) \(.status)"'
done
```

Note: multi-id `bug view --json` may wrap results as `{ "bugs": [ ... ] }`;
single-id output is the object directly. Check with `jq 'has("bugs")'` if unsure.
