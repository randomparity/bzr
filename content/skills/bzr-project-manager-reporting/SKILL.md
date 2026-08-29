---
name: bzr-project-manager-reporting
description: Create a decision-ready project-management report from a Bugzilla Custom Search URL or saved bzr query, choosing safe CSV, XLSX, self-contained HTML, or Markdown output according to the artifact capabilities currently available.
---

# Create a project-manager report

Turn Bugzilla evidence into an artifact the audience can use. Do not add output formats to `bzr` or
promise a format before checking the active environment's artifact capabilities.

## Define the report

Clarify the audience, decision or meeting, saved-query name or Custom Search URL, completeness,
grouping dimensions, stale threshold, and requested artifact. Ask before saving a URL because
`query save` changes local configuration.

For an existing saved query, inspect it before collection:

```sh
bzr query show pm-status --json
bzr query run pm-status \
  --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
  --paginate --json
```

For a Bugzilla `buglist.cgi` Custom Search URL, either collect directly or—with approval—give it a
local name:

```sh
bzr bug search --from-url "$URL" \
  --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
  --paginate --output ndjson
bzr query save pm-status --from-url "$URL"
```

Prefer `--fields`; add only fields used by the report. Use `--paginate` whenever the report claims
all matches. The `--json` result is a versioned envelope whose rows are in `.data`; NDJSON stays
bare and emits one compact bug object per line for list results. Obtain the published contract
version with `bzr schema --json` when provenance requires it. Never reproduce the original URL:
bzr may remove credentials while importing it.

Status Whiteboard is a standard Bugzilla field that an installation may disable. Its value is a
mutable current snapshot. Use `bzr comment list <bug-id> --json` when the report needs the durable
update history. Never present whiteboard as an activity log.

## Choose an honest artifact

Inspect the active agent's installed skills and tools before committing to a format:

- Use the spreadsheet capability for XLSX and its workbook verification workflow.
- Use a document/site or direct file capability able to create escaped, self-contained HTML.
- Use a CSV library or writer that supports explicit quoting and text preservation.
- Produce Markdown directly for a portable report and a terminal summary when requested.

CSV, XLSX, HTML, and Markdown are supported only when the corresponding active capability is
available. If the requested capability is unavailable, say which capability is missing, produce a
Markdown report, and offer CSV only when a safe CSV writer is available. A renamed CSV is not XLSX.

Before generating CSV, XLSX, or HTML, read
[artifact safety](reference/artifact-safety.md). For report structure, read the
[PM report template](reference/report-template.md). Open and verify the final artifact with its
own capability; a successful write alone is not proof that a workbook or page is usable.

## Analyze for the decision

Compute groupings only from collected fields and cite contributing bug IDs. Separate observed facts,
interpretation, decisions, and follow-ups. Call out partial visibility, inaccessible rows, missing
fields, and unknown totals. Do not infer that removal from scope means resolution.

Compose another bundled skill only when it matches the request:

- `bzr-weekly-status` for snapshot-to-snapshot changes;
- `bzr-dependency-analysis` for blockers and dependency topology; or
- `bzr-release-readiness` for an evidence-bounded release decision.

These are optional collaborators, not a required pipeline.

## Demonstration

From the bzr repository, `tools/record-demo.sh project-manager-reporting` records a workflow whose
visible narrative begins with an agent prompt and ends with the complete PM-ready report. It uses a
real local Bugzilla server and the installed skill; setup and helper plumbing stay outside the cast.
