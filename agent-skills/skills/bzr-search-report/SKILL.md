---
name: bzr-search-report
description: Use when searching Bugzilla or producing a bug report/digest with bzr — run saved or ad-hoc queries, extract fields with --json and jq, and summarize results (e.g. "my open bugs", a weekly triage list).
---

# Search and report with bzr

## Ad-hoc search

```
bzr bug search "crash on startup" --json | jq -r '.[] | "\(.id)\t\(.status)\t\(.summary)"'
bzr bug list --product Foo --status NEW --json | jq -r '.[].id'
```

## Saved queries

Save a reusable query once, run it by name:

```
bzr query save my-open --assignee you@example.com --status NEW --status ASSIGNED
bzr query run my-open --json
bzr query list
```

## Your own bugs

```
bzr bug my --status \!CLOSED --json | jq 'length'                 # count
bzr bug my --status \!CLOSED --json | jq -r '.[] | "\(.id)\t\(.summary)"'
```

## Build a digest

Combine a query with jq to produce a readable list to drop into a report:

```
bzr query run my-open --json \
  | jq -r 'sort_by(.status)[] | "- #\(.id) [\(.status)] \(.summary)"'
```

For more extraction patterns see `bzr-reference` (`reference/json-recipes.md`).
