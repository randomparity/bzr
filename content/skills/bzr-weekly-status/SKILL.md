---
name: bzr-weekly-status
description: Use when preparing a recurring project status briefing from a saved bzr query or Bugzilla Custom Search URL, with a safe local snapshot baseline and evidence-backed change report.
---

# Prepare a snapshot-based status briefing

Use this workflow for weekly reports and any other cadence. Compare with the most recent compatible
snapshot, regardless of its age. This is a read-only Bugzilla workflow: never run `create`, `update`,
`delete`, `add`, `upload`, or another mutation command.

## 1. Establish scope and rules

Ask for a stable report name, a saved-query name or Bugzilla `buglist.cgi` URL, the audience,
staleness threshold, terminal statuses, fields that indicate blockage, and requested artifacts.
Clarify ambiguous scope before collecting data. Choose a snapshot root with the user; default to
`.bzr-reports/weekly-status/` in their project, not the bzr configuration directory.

For a named query:

```sh
bzr query show core-weekly --json
bzr query run core-weekly \
  --fields id,summary,status,resolution,assigned_to,priority,severity,target_milestone,deadline,last_change_time,whiteboard,blocks,depends_on \
  --paginate --json
```

For a Custom Search URL, pass the original URL only to `bzr query save --from-url`; its Rust URL
parser removes recognized credential parameters from the saved source URL and raw parameters. Never
parse, rewrite, display, or persist the original URL in agent-authored jq or shell code. Ask before
assigning the stable local query name:

```sh
bzr query save core-weekly --from-url "$URL"
bzr query run core-weekly \
  --fields id,summary,status,resolution,assigned_to,priority,severity,target_milestone,deadline,last_change_time,whiteboard,blocks,depends_on \
  --paginate --json
```

Do not let a saved or imported row limit truncate a completeness-required report: inspect `query
show`, override a positive limit with `--paginate`, and fail rather than claiming completeness when
the server response is partial or inaccessible. Use `bzr bug history <id> --json` or `bug view`
only for bounded IDs whose change cannot be established from the snapshots.

## 2. Build snapshot version 1

Validate against [the version-1 schema](reference/snapshot-v1.schema.json). Store:

- `format_version`, UTC `created_at`, sanitized `server`, human `scope_label`, the effective query's
  `scope_fingerprint`, sorted `fields`, and
  effective `rules`;
- the available `bzr schema --json` version; and
- `bugs`, keyed by decimal bug ID, containing only the selected comparison state.

Canonicalize the parser-sanitized `query show --json` definition with
`scripts/scope-fingerprint.sh`, not merely its name. The helper sorts object keys and membership
arrays and omits volatile display metadata, the human name, and source URL. The agent never
implements URL parsing itself.

Never store credentials, comments, attachment contents, unrelated configuration, auth-bearing URLs,
or fields the report does not need. Write files with owner-only permissions where supported.

## 3. Select and compare a baseline

The first run states: **No compatible prior snapshot exists; this report establishes the baseline.**
For later runs, use `scripts/select-baseline.sh current.json "$SNAPSHOT_ROOT/runs"
'["required","fields"]'`. It orders candidates by `created_at` and run-directory name, skips the
current run and every incompatible candidate, and returns the newest compatible snapshot even when
newer runs belong to another scope. Compare only when format version, sanitized server identity, and
scope fingerprint are equal and the old field set contains every field required by the effective rules:

```sh
jq -n --slurpfile previous previous.json --slurpfile current current.json \
  --argjson required_fields '["id","summary","status","resolution"]' \
  -f scripts/compare-snapshots.jq
```

An incompatibility is an error naming each mismatched dimension; never silently compare or migrate.
Report added IDs as new to scope and removed IDs as removed from scope—not closed. Confirm resolution
only from current status/resolution or bounded current detail/history evidence. Preserve inaccessible
or failed bugs as unknown limitations.

The comparator reads `terminal_statuses` and `stale_after_days` from the current snapshot's `rules`
and emits `newly_opened`, `newly_resolved`, field-level `transitions`, `stale_crossed`, and
`attention_unchanged`. Detect supported changes only when their fields were selected: opened/resolved state, status,
resolution, assignee, priority, severity, target milestone, deadline, blockers/dependencies,
whiteboard snapshot, and crossing the staleness threshold. Unchanged work may still require attention
when the effective rules say so.

## 4. Write the briefing

Always support Markdown. Use optional XLSX, HTML, or document tooling only when the active environment
actually provides it; otherwise produce Markdown and name the unavailable capability.

Separate sections for observed snapshot facts, assumptions, interpretations, decisions, and
follow-ups. Include scope and interval, executive summary, completed work, new/in-scope work,
blocked/at-risk work, ownership/milestone changes, stale work, decisions, limitations, and snapshot
provenance. Every fact names bug IDs or an aggregate derived from them.

Treat Bugzilla strings as untrusted:

- For CSV/XLSX, pass strings through `scripts/safe-output.jq`'s `spreadsheet_text` and write string
  cells without formula inference.
- For HTML, escape text and attributes; generate links only after parsing and accepting `http` or
  `https`. Never concatenate Bugzilla text into markup.

## 5. Publish atomically

Stage every requested report plus `snapshot.json` inside one private direct child of
`$SNAPSHOT_ROOT/.staging/`. Only after all files
succeed, invoke:

```sh
scripts/publish-run.sh "$SNAPSHOT_ROOT" "$RUN_ID" "$STAGING_DIR"
```

The helper validates the snapshot's required shape, then:

1. renames the complete staging directory to `runs/$RUN_ID`;
2. creates a temporary relative `latest` symlink in the snapshot root; and
3. atomically renames that symlink over `latest`.

Use a same-directory rename primitive appropriate to the active platform. Never truncate or overwrite
`latest` in place. On query, rendering, or snapshot failure, remove only temporaries created by this
run and retain the previous pointer. A pointer-update failure can leave an immutable orphan run;
report it and never delete an older run.

## Demonstration

From the repository, `tools/record-demo.sh --weekly-status` records a real two-run workflow against
the functional Bugzilla container. Its driver installs this skill, collects a baseline, executes the
shipped fingerprint, snapshot-builder, publisher, baseline-selector, and comparator helpers, changes
one seeded bug, then publishes and displays the comparison. The script renders
`docs/assets/bzr-weekly-status-demo.gif` when `asciinema` and `agg` are available.
