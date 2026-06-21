# Issue #365: Bug Update Structured Input Design

## Context

`bug create --from-json` lets agents submit a structured JSON object or array
instead of flattening a bug model into shell flags. `bug update` still requires
flags, so an agent that already has an update object must translate it into
`--status`, `--keywords-add`, `--comment`, and similar flags.

Issue #365 asks for the same structured-input contract on `bug update`:
unknown-key rejection, explicit CLI flags overriding JSON fields, list
add/remove semantics, comment-source coverage, optimistic-concurrency coverage,
and the existing exit-11 partial-failure model.

## Input Shapes

Add `--from-json <PATH|->` to `bug update`.

Top-level object:

- `bzr bug update <ID...> --from-json update.json` applies one structured edit
  to the positional IDs.
- The object may also carry `id` for the single-target form
  `bzr bug update --from-json update.json`.
- Positional IDs and object `id` are mutually exclusive. Target IDs are
  higher-blast-radius than ordinary update fields, so `bzr` rejects mixed target
  sources instead of silently choosing one.
- Multiple positional IDs keep the current `bug update` semantics: the same
  update is applied to every ID, and partial server failures use the existing
  batch-result shape.

Top-level array:

- `bzr bug update --from-json updates.json` applies one independent edit per
  array element.
- Each array element must include `id`.
- Positional IDs are rejected with top-level arrays so a caller cannot
  accidentally apply every array element to the same bug set.
- A one-element array still emits the batch-result shape, matching
  `bug create --from-json`'s "output follows input shape" rule.

Any other top-level shape exits 7.

## Accepted Keys

Update keys match the public update flag names converted to JSON object keys:

- scalar fields: `status`, `resolution`, `dupe_of`, `alias`, `deadline`,
  `estimated_time`, `remaining_time`, `work_time`, `assignee`, `priority`,
  `severity`, `summary`, `whiteboard`, `url`, `target_milestone`
- reset booleans: `reset_assigned_to`, `reset_qa_contact`
- flag updates: `flags` as an array of normal `--flag` syntax strings
- ID-list deltas: `blocks_add`, `blocks_remove`, `depends_on_add`,
  `depends_on_remove`
- string-list deltas: `keywords_add`, `keywords_remove`, `cc_add`,
  `cc_remove`, `groups_add`, `groups_remove`, `see_also_add`,
  `see_also_remove`
- comment fields: `comment`, `comment_file`, `comment_private`
- optimistic concurrency: `expect_unchanged_since`
- array/object target key: `id`

Unknown keys are rejected via `serde(deny_unknown_fields)`. Custom-field writes
stay out of scope until #283 makes a deliberate design decision.

## CLI Override Rules

Explicit CLI values override corresponding JSON values:

- `Some` scalar flags replace JSON scalar values.
- non-empty repeatable flags replace JSON arrays for the matching field.
- true boolean flags set the JSON boolean to true; absent boolean flags leave
  JSON true/false/absent unchanged.
- `--comment` or `--comment-file` replaces the JSON comment source uniformly.
- `--comment-private` sets all resulting comments private.
- `--expect-unchanged-since` replaces JSON `expect_unchanged_since` uniformly.

For arrays, overrides apply to every element. Positional IDs are not an override
for arrays; they are rejected for that shape.

## Validation and Writes

All JSON entries are parsed and converted to `UpdateBugParams` before any write:

- invalid JSON or wrong top-level shape exits 7;
- empty arrays exit 7;
- array elements without `id` exit 7;
- object input without positional IDs and without `id` exits 7;
- object input with positional IDs and JSON `id` exits 7;
- `comment` and `comment_file` are mutually exclusive;
- `comment_private` without a comment source exits 7;
- malformed deadlines, malformed flags, empty list values, and no-op updates
  reuse the existing update validation;
- `alias` with multiple target IDs remains invalid;
- `dupe_of` combined with `status` or `resolution` remains invalid outside
  clap-validated CLI parsing, so JSON cannot bypass the CLI conflict.

`--from-json -` and `--comment -` / `--comment-file -` cannot both consume
stdin. Reject that combination with an input-validation error. JSON
`comment_file` must name a file path; `comment_file: "-"` is rejected. For
per-entry comments, put the comment text in JSON. For a uniform stdin comment,
use CLI `--comment -` only with top-level object input when `--from-json` reads
from a file. Array input rejects CLI stdin comment sources because stdin cannot
be consumed separately for each element.

Object input reuses the existing `apply_checked` path, preserving current
single/batch output and all-or-nothing optimistic-concurrency behavior for a
multi-ID object update.

Array input validates every entry first, then processes entries independently:

- dry-run emits one dry-run result whose `changes` array contains one object per
  array element;
- success/failure output uses the existing `BatchResult` shape;
- per-entry `expect_unchanged_since` is checked before writing that entry;
- a stale entry is recorded as a failure for its `id` and later entries still
  run;
- any failure exits 11 after output is written.

## Files

- `src/cli/bug.rs`: add `UpdateArgs::from_json`, relax positional IDs when
  `--from-json` is present, and document the flag in long help.
- `src/cli/mod_tests.rs`: prove `bug update --from-json -` parses without
  positional IDs.
- `src/commands/bug/update.rs`: parse/overlay structured input, convert it
  through existing update validation, and add array batch handling.
- `src/commands/bug/update_tests.rs`: add parser, validation, override,
  comment/concurrency, partial-failure, and request-body tests.
- `docs/bzr-cli.md`: document object and array shapes.
- `CHANGELOG.md`: add an Unreleased entry.

## Testing

- `cargo test parse_bug_update_from_json_stdin`
- `cargo test bug_update_from_json --lib`
- `cargo test commands::bug::update::tests --lib`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `git diff --check`

## Out of Scope

- `bug update` custom-field writes.
- Input schemas for this payload; #366 covers `bzr schema` input contracts.
- Structured input for admin resources; #369 covers those resources.
- Changing existing flag-only `bug update` behavior.
