# bzr --json + jq recipes

All read paths support `--json`. Output is wrapped in a versioned envelope —
`{"schema_version": "0.6.0", "data": <payload>}` — so read fields under `.data`.
Pipe to `jq`. (`--output ndjson` records stay bare; see below.)

```
# The contract version itself
bzr --server-url https://bugzilla.example.com schema --json | jq -r '.schema_version'

# Public read-only Bugzilla, no config or API key
bzr --server-url https://bugzilla.example.com server info --json | jq -r '.data.version'

# Bug ids from a search
bzr bug search "memory leak" --json | jq -r '.data[].id'

# A single bug's assignee and status
bzr bug view 12345 --json | jq -r '.data.assigned_to, .data.status'

# Attachment file names on a bug
bzr attachment list 12345 --json | jq -r '.data[].file_name'

# Component names of a product
bzr product view Fedora --json | jq -r '.data.components[].name'

# Which status values a bug can move to
bzr field list status --json | jq '.data[] | select(.name=="NEW") | .can_change_to'

# Count of your open bugs (client side)
bzr bug my --status \!CLOSED --json | jq '.data | length'

# Match count without fetching the rows (server side, cheaper)
bzr bug list --product Foo --status NEW --count --json | jq '.data.count'
```

## Streaming with `--output ndjson`

`--output ndjson` emits one compact JSON record per line for list results — ideal
for large result sets and line-oriented tooling (`jq -c`, `grep`, `while read`).
ndjson records are **bare** (no `schema_version` envelope and no `.data`
wrapper), so read fields directly; it is the stable shape for pinned automation.
Read the contract version out of band via `bzr schema --json` (`.schema_version`)
or `bzr --version`.

```
# One bug per line; pull two fields from each (bare records — no .data)
bzr bug list --product Foo --status NEW --output ndjson | jq -c '{id, summary}'

# Stream a saved query and act per row as it arrives
bzr query run my-open --output ndjson | while IFS= read -r line; do
  echo "$line" | jq -r '"#\(.id) \(.status)"'
done
```

Note: under `--json`, multi-id `bug view` puts results at `.data.bugs[]`;
single-id output is the object at `.data`. Check with `jq '.data | has("bugs")'`
if unsure.
