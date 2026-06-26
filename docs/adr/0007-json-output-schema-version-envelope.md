# 0007 — `--json` output is wrapped in a versioned envelope

- Status: Accepted
- Date: 2026-06-26
- Issue: #464

## Context

`bzr` publishes JSON Schemas via `bzr schema`, but the `--json` output itself
carries no version marker and there is no documented stability policy. Agents
that cache a parsed shape break silently when a release renames or restructures a
field. Issue #464 asks for a stable, detectable JSON contract.

Today `--json` output is **unenveloped**: each command serializes its domain type
directly through `output::formatting::write_json` — `bzr bug view` emits a bare
object, `bzr bug search`/`attachment list` emit bare arrays. There is no
universal wrapper to hang a version off.

Two forks were decided by the operator (the issue's own acceptance criteria were
internally tense — #1 wanted a version in "every envelope" while #5 demanded
"additive only, no breaking changes"):

1. **How to surface the version.** Options: (a) restructure only `bzr schema
   --json` and leave payloads untouched (fully additive); (b) add a sibling
   `schema_version` to object-shaped outputs only (inconsistent, can't apply to
   bare arrays); (c) wrap *every* `--json` output in `{schema_version, data}`
   (uniform but breaking). **The operator chose (c).**
2. **Version source.** Options: track the crate version (stripped or verbatim),
   or a separate manually-bumped constant. **The operator chose a separate
   constant.**

## Decision

1. **Versioned envelope on the `Json` format only.** All pretty `--json` success
   output is wrapped at the single seam `output::formatting::write_json` as
   `{"schema_version": SCHEMA_VERSION, "data": <existing payload>}`. Error output
   (`main::format_dispatch_error`) gains a top-level `schema_version` sibling to
   `error`. The rule is: **`schema_version` is present iff the format is `Json`.**

2. **`--output ndjson` is never wrapped.** Streaming records and the single-line
   error stay byte-for-byte as today, satisfying acceptance #4 and leaving the
   line-oriented audience unaffected. The version for ndjson consumers is read
   from `bzr schema --json` or the binary `--version`.

3. **`bzr schema <name>` stays a verbatim passthrough.** It emits a JSON-Schema
   *document*, not a command-result payload; it bypasses `write_json` and is not
   wrapped. `bzr schema` (list) flows through `write_json` and is wrapped like any
   other result, so it reports the version via the uniform envelope rather than a
   bespoke `{schema_version, schemas}` shape.

4. **A separate, manually-bumped `SCHEMA_VERSION` constant** (`src/output/mod.rs`,
   initial `"0.6.0"`), decoupled from the crate version, governed by a documented
   semver policy: patch = additive, minor = rename-with-one-release-alias, major =
   breaking. The constant is the single source of truth for the envelope and the
   `bzr schema` report.

5. **Schemas:** add `schemas/envelope.json` for the wrapper; make
   `schema_version` an *optional* top-level field in `schemas/error.json` so both
   the `--json` (versioned) and `--output ndjson` (bare) error shapes validate.
   Per-payload schemas are unchanged — they describe `data` contents and are
   validated at the type level by existing drift tests.

## Consequences

- This is a **breaking change** to the `--json` contract (overriding the issue's
  acceptance #5). Every `jq` recipe, doc example, integration-test assertion, and
  functional-test phase that reads `--json` moves from `.field` / `.[].field` to
  `.data.field` / `.data[].field`. `--output ndjson` consumers are unaffected.
- A single seam owns the wrapper, so the contract cannot drift per-command and new
  commands inherit it for free.
- The envelope lives at the output layer, so domain types and their published
  payload schemas evolve independently of the wrapper.
- `SCHEMA_VERSION` must be bumped by hand on contract changes; the policy doc and
  CHANGELOG discipline (entries land with the change) are the guardrail against
  forgetting. There is no automated lockstep with the crate version.

## Considered & rejected

- **Schema-command-only (option a).** Fully additive, breaks nothing, but does not
  give per-output detectability the operator wanted; an agent reading a payload
  could not tell which contract produced it without a second call.
- **Sibling field on object outputs only (option b).** Cannot apply to bare-array
  outputs (`bug search`, `attachment list`) without wrapping them anyway, yielding
  an inconsistent contract where only some commands carry the version.
- **Wrap ndjson lines too.** Rejected by acceptance #4: repeating the version on
  every streamed line is redundant and bloats line-oriented output; ndjson
  consumers read the version out-of-band.
- **Track the crate version (stripped/verbatim).** Rejected by the operator in
  favor of an independently-bumped constant; the crate is `0.6.1-dev`, and a
  pre-release suffix in a stability field is undesirable, while exact lockstep
  would force a schema "change" on every unrelated crate bump.
- **Wrap the `bzr schema <name>` document.** Rejected: it would corrupt the
  emitted JSON-Schema document, which is its own published artifact.
