# Snapshot-based weekly status skill design

## Scope

Issue #569 adds one canonical `bzr-weekly-status` skill. It composes existing `bzr` query, search,
history, schema, and structured-output commands; it does not add a Rust command or persist reporting
state in `bzr` configuration. The snapshot protocol follows [ADR 0023](../../adr/0023-skill-owned-weekly-status-snapshots.md).

The skill supports a saved query or Bugzilla Custom Search URL, Markdown on every installation, and
XLSX, HTML, or document output only when the active agent environment provides a corresponding
artifact capability. It performs no Bugzilla mutation.

## Workflow and data flow

1. Clarify the report name, scope, audience, comparison rules, staleness threshold, output formats,
   and snapshot root. Default the root to `.bzr-reports/weekly-status/` under the user's chosen
   project directory, never the bzr config directory.
2. Resolve a named query with `bzr query show` then collect with `bzr query run`; resolve a Custom
   Search URL by importing it under a user-approved saved-query name with `bzr query save --from-url`
   and then running it. Request the explicit comparison field set with `--fields`, `--paginate`, and
   `--json`.
3. Build a format-version-1 JSON snapshot containing creation time, sanitized server identity,
   a human scope label, a SHA-256 fingerprint of the canonical effective query definition, effective
   fields and rules, optional `bzr schema --json` version, and sorted bug
   records. Exclude credentials, comments, attachment contents, and unrelated configuration.
4. The baseline selector scans immutable run snapshots, orders candidates by UTC `created_at` and
   run-directory name, skips the current and incompatible runs, and returns the newest compatible
   prior snapshot. Compatibility requires equal format, server, and scope fingerprint plus a prior
   field set containing every field needed by the requested rules. The first run says no comparison
   is available.
5. Compute added/removed scope membership and supported field transitions. Removal is never called
   closure; only current status/resolution or targeted detail/history evidence may establish that.
   Missing or inaccessible bugs remain limitations/unknowns.
6. Render the ten-section briefing requested by the issue. Label snapshot observations as facts and
   separate interpretations, assumptions, decisions, and follow-ups.
7. Stage all requested reports and `snapshot.json` in one private directory created directly under
   the report root's `.staging/`, keeping publication on one filesystem. Invoke the shipped
   publisher, which validates the snapshot, renames the whole directory into the report root, creates
   a temporary relative `latest` symlink, and atomically renames that symlink over the prior one. A
   failure before the directory rename removes staging; a failure afterward may leave one immutable
   orphan run directory but retains the prior `latest` baseline.

## Snapshot contract

The reference fixture defines required JSON keys and compatibility examples. Bug records are keyed
by decimal ID and carry only selected fields. The source URL is sanitized by removing credential
parameters and userinfo; when trustworthy sanitization cannot be established, store a stable scope
label instead. For named queries and URL imports, canonicalize the effective `query show --json`
definition with the shipped fingerprint helper: object keys sorted; known order-insensitive filter
arrays sorted; raw parameter pairs sorted only as complete pairs, preserving each key/value order;
volatile display metadata, human query name, and source URL removed; and URL userinfo and credential
parameters rejected before import. Hash those UTF-8 bytes with SHA-256. Persist the
human label separately. Rules are normalized JSON values so changing a staleness threshold or terminal-status
set is visible provenance, while only rules that affect a requested comparison make an older
snapshot incompatible.

## Failure behavior

- A failed query produces no new snapshot or report.
- Malformed, unsupported-version, wrong-server, wrong-scope, or insufficient-field snapshots are
  rejected with the mismatched dimensions named; they are never silently migrated.
- Partial or inaccessible result sets are reported as limitations and are not treated as absence.
- Report-generation or snapshot-write failure leaves `latest` and all prior completed runs intact.
- Missing optional artifact tooling falls back to Markdown and names the unavailable format.

## Trust boundaries

The local operator controls scope, destination, and rules. Bugzilla controls all returned field text
and links. The skill treats output as data: prefix spreadsheet strings beginning with `=`, `+`, `-`,
or `@` with an apostrophe; use an XLSX library's string-cell API; HTML-escape every text node and
attribute; permit generated hyperlinks only after parsing and accepting `http` or `https`. Snapshot
paths are derived from a conservative report slug and are checked to remain below the chosen root.
Credentials, comments, and attachment bodies are prohibited snapshot keys.

Out of scope are hostile local users who can already modify the operator's report directory and
security guarantees of optional third-party artifact tools beyond their documented safe APIs.

## AI surface and evaluation plan

AI-SPEC: A project manager triggers the installed skill with a saved query or Custom Search URL.
The agent consumes only operator instructions, deterministic `bzr` output, compatible snapshots,
and targeted details/history, then emits reports and a new local snapshot. It must not mutate
Bugzilla, invent transitions, expose secrets, or silently weaken compatibility. Use batched query
data first and bounded targeted reads only when needed; success is fixture-valid provenance, a
truthful comparison, safe output, and an atomic baseline advance.

Severity-5 modes are credential persistence, Bugzilla mutation, unsafe spreadsheet/HTML output, and
loss of the prior baseline. Severity-4 modes are false closure, comparing incompatible snapshots,
silently dropping inaccessible data, and presenting interpretation as fact. Fixture evaluations
cover: first-run baseline; compatible changes; same query name with changed effective parameters;
changed scope; inaccessible nodes; incompatible
server/scope/version/fields; injected formulas and HTML; forced report/snapshot-write failure;
ambiguous scope requiring clarification; stale/conflicting data; and bounded history use. The
repository has no agent-execution harness, so deterministic tests validate the executable fingerprint
builder, baseline selector, rule-driven comparator, publisher, safe-output filters, schema, documented command allowlist, mutation
denylist, briefing labels, and eval-case manifest. A release reviewer runs the manifest's prompt cases
in an active agent environment and records the transcript as acceptance evidence. Without that run,
the AI-behavior criteria remain explicitly unverified. No model self-grading is accepted.

## Demonstration and verification

Add a deterministic fixture runner that exercises the comparison and publication rules without network access and
extend the real-container functional phase to prove the exact saved-query, field projection,
pagination, whiteboard, relationship, and history commands documented by the skill. Extend the
asciinema driver to replay the exact deterministic collector/comparator workflow: it shows the
installed skill, baseline message, a real Bugzilla change, and the subsequent comparison without
claiming to execute an agent inside the terminal recording. `make
skills-test`, focused functional testing, `make lint`, `make test`, and `make functional-test-all`
are the required gates.
