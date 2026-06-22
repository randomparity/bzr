---
name: bzr-search-report
description: Use when searching Bugzilla or producing a bug report/digest with bzr — run saved or ad-hoc queries, extract fields with --json and jq, and summarize results (e.g. "my open bugs", a weekly triage list).
---

# Search and report with bzr

## Ad-hoc search

```
# Public read-only server, no config or API key
bzr --server-url https://bugzilla.example.com bug search "crash on startup" --json \
  | jq -r '.[] | "\(.id)\t\(.status)\t\(.summary)"'

bzr bug search "crash on startup" --json | jq -r '.[] | "\(.id)\t\(.status)\t\(.summary)"'
bzr bug list --product Foo --status NEW --json | jq -r '.[].id'
```

### Sort, page, count, and filter

`bzr bug list` accepts a rich filter and ordering set:

```
# Order and paginate
bzr bug list --product Foo --sort priority --order desc --limit 25
bzr bug list --product Foo --offset 50 --limit 25     # one page
bzr bug list --product Foo --paginate --json          # fetch every page

# Just the count (cheaper than fetching rows)
bzr bug list --product Foo --status NEW --count --json | jq '.count'
```

Extra filters beyond product/component/status/assignee: `--resolution`,
`--version`, `--op-sys`, `--platform`, `--whiteboard`, `--target-milestone`,
`--qa-contact`, `--url`, `--created-since`, `--changed-since`.

## Saved queries

Save a reusable query once, run it by name:

```
bzr query save my-open --assignee you@example.com --status NEW --status ASSIGNED
bzr query run my-open --json
bzr query list
```

Save with a persisted order (`--sort`/`--order`), and edit a saved query in
place rather than re-saving it:

```
bzr query save my-open --status NEW --status ASSIGNED --sort changed --order desc
bzr query update my-open --status ASSIGNED --clear assignee   # --clear drops a field
bzr query update imported --from-url 'https://bz/buglist.cgi?product=Foo'
```

`query update --from-url` refreshes a saved Bugzilla URL import without losing
allowed overrides such as `--limit`, `--fields`, date filters, and sort order.

## Your own bugs

```
bzr bug my --status \!CLOSED --product Foo --json | jq 'length'      # count
bzr bug my --status \!CLOSED --component Bar --changed-since 2026-01-01 --json \
  | jq -r '.[] | "\(.id)\t\(.summary)"'
```

`bug my` supports the same product/component/status/date/metadata filters as
`bug list`, plus `--all`, `--created`, `--cc`, `--count`, `--fields`,
`--sort`, paging, and `--limit` per category.

## Build a digest

Combine a query with jq to produce a readable list to drop into a report:

```
bzr query run my-open --json \
  | jq -r 'sort_by(.status)[] | "- #\(.id) [\(.status)] \(.summary)"'
```

For large or streaming digests, use `--output ndjson` (one compact record per
line) and process rows as they arrive:

```
bzr query run my-open --output ndjson \
  | jq -rc '"- #\(.id) [\(.status)] \(.summary)"'
```

For more extraction patterns see `bzr-reference` (`reference/json-recipes.md`).
