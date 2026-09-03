# bzr --json + jq recipes

All read paths support `--json`. Output is wrapped in a versioned envelope —
`{"schema_version": "3.0.0", "data": <payload>}` — so read fields under `.data`.
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

# Which server a key actually resolved against, before a write
bzr whoami --json | jq -r '.data.server_name, .data.auth_mode'

# Everything blocking a bug, two hops out
bzr bug links 12345 --recursive --depth 2 --json \
  | jq -r '.data[] | select(.relation=="depends_on") | "\(.id)\t\(.status)\t\(.summary)"'

# Preserve every requested ID/alias outcome while reading complete adjacency
bzr bug adjacency 00123 release/2026 missing-alias --json \
  | jq '{requests: .data.requests, bugs: [.data.bugs[] | {id, blocks, depends_on}]}'

# Who last touched the status field (history records are flattened, one per field)
bzr bug history 12345 --json \
  | jq -r '.data[] | select(.field=="status") | "\(.when) \(.who) \(.old_value)->\(.new_value)"'

# What the server will let you do, before planning a mutation
bzr server capabilities --json | jq '.data.status_transitions'
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

## Project to cut tokens with `--fields`

Every list/view read verb accepts `--fields <a,b,c>` / `--exclude-fields <a,b,c>`
to trim the JSON object to the keys you actually need. Field names are the verb's
`--json` keys (top-level only). This cuts tokens and latency on large results —
a 200-comment thread fetched for a tag index, an attachment list fetched for file
names and sizes — without post-filtering in `jq`.

```
# Comment thread as a lightweight index (no bodies)
bzr comment list 12345 --json --fields id,creator,creation_time | jq '.data'

# Attachment metadata only, streamed
bzr attachment list 12345 --output ndjson --fields file_name,size

# Drop noisy nested keys instead of listing every key you want
bzr product view Fedora --json --exclude-fields components,versions,milestones | jq '.data'
```

Rules: an unknown field name (or a selection that removes every key) exits 7;
selecting a key a record does not carry yields a sparse object (e.g.
`--fields data` on `attachment list`); table output ignores the flags with a
warning, so always pair them with `--json` or `--output ndjson`. The `bug`
verbs and `query run` support the same flags with alias-aware field names.
