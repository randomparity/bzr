# Project-manager reporting skill design

Issue #567 adds one embedded `bzr-project-manager-reporting` skill. It teaches an agent to collect
the smallest complete Bugzilla dataset needed for a project-management question and turn it into a
requested artifact using capabilities that are actually present in the active agent environment.
It does not add report formats to `bzr` itself.

## Workflow contract

1. Clarify audience, decision, scope, freshness/completeness, grouping, staleness rule, and artifact.
2. Resolve either an existing saved query or a Bugzilla `buglist.cgi` Custom Search URL. A URL may
   be saved locally only with user approval because that mutates bzr configuration.
3. Select only fields needed by the report and use `--paginate` whenever completeness is claimed.
   Consume the versioned JSON envelope for whole-result work or NDJSON for streaming rows.
4. Inspect available artifact capabilities before promising a format. Produce CSV, XLSX,
   self-contained HTML, or Markdown when the environment has the relevant capability. If it does
   not, name the missing capability, produce Markdown, and offer portable CSV when a safe CSV writer
   is available. Never claim that renaming a text file creates XLSX.
5. Present facts separately from interpretations and cite bug IDs behind aggregates. Whiteboard is
   an optional standard Bugzilla field: its current value is a mutable snapshot; comments are the
   durable update history.

The skill may compose the merged weekly-status, dependency-analysis, and release-readiness skills
when the request actually needs comparison, dependency topology, or a release decision. Those
skills are optional collaborators, not prerequisites or a fixed pipeline.

## Safety

Bugzilla fields are untrusted. CSV and spreadsheet cells whose first non-whitespace character is
`=`, `+`, `-`, or `@` must be emitted as text rather than formulas. XLSX writers must create string
cells with formula inference disabled. HTML text and attributes must be escaped, and generated bug
links are allowed only when the parsed base URL uses HTTP or HTTPS. The original Custom Search URL
must not be reproduced because it may have contained credentials removed by bzr while importing.

## Demonstration and proof

The asciinema demo visibly starts with a project manager's agent prompt, then shows the installed
skill collecting a saved Custom Search query through the real CLI, and ends by displaying a useful
Markdown status report with an executive summary, grouped results, stale/attention items, current
whiteboard snapshots, limitations, and provenance. Helper commands remain hidden from the recorded
narrative.

Contract tests verify skill routing, every documented bzr command against the compiled CLI help,
capability/fallback language, format safety, and the prompt-to-artifact demo fixture. The functional
skills phase verifies installation. A live Bugzilla phase imports and saves a Custom Search URL,
updates and retrieves whiteboard and comments, collects the projected complete result through both
JSON and NDJSON, and checks that the demonstrated fields have the documented semantics.

## Smallest viable surface

- `content/skills/bzr-project-manager-reporting/` with `SKILL.md`, a format-safety reference, a PM
  report template, and contract fixtures/tests.
- Embedded-skill inventory and installer functional expectations.
- One live functional phase for the demonstrated CLI workflow.
- A named `tools/record-demo.sh project-manager-reporting` mode and public demo documentation/assets.

No renderer or report engine is shipped: the active agent's artifact tools own artifact creation.
This keeps capability claims honest and avoids duplicating the spreadsheet, document, and site
capabilities the skill is meant to route to.
