# 0014 — Structured per-variant detail keys on the `--json` error object

- Status: Accepted
- Date: 2026-06-29
- Issue: #482 (carved out of the closed #459)
- Related: [0007](0007-json-output-schema-version-envelope.md)

## Context

The `--json` / `--output ndjson` error object shipped in v0.7.0 (`schemas/error.json`,
ADR-0007) carries three universal keys on **stderr**: `type`, `message`,
`exit_code`. The machine-actionable detail an agent needs — which field was
rejected and its offending value, the change tokens to retry a mid-air
collision, the resource that was not found — is flattened into the `message`
string, forcing prose pattern-matching.

Issue #459 originally asked to *move* the error to stdout and enrich it. The
move was rejected (it reverses ADR-0007's deliberate stdout=data /
stderr=diagnostics split); #482 is the additive, stays-on-stderr enrichment.
See `docs/specs/2026-06-29-issue-482-json-error-enrichment-design.md`.

Three decisions here have viable alternatives worth recording.

## Decision

1. **A single `BzrError::structured_detail()` seam.** A method beside
   `error_type()` / `exit_code()` returns a `serde_json::Map` of variant-specific
   keys (empty for variants with no machine-useful detail).
   `main::format_dispatch_error` seeds the `error` object with that map, then
   writes `type` / `message` / `exit_code` **last** so they can never be clobbered;
   `structured_detail()` must not emit those three reserved keys. The contract is
   additive: every new key is **optional** in `schemas/error.json`, the channel
   stays stderr, and the bump is a **patch** (`SCHEMA_VERSION` 0.6.0 → 0.6.1) under
   the ADR-0007 policy.

2. **`InputValidation` becomes a struct variant carrying optional attribution.**
   `InputValidation(String)` → `InputValidation { message, field: Option<String>,
   value: Option<String> }`, with `BzrError::input(msg)` for the ~100 sites that
   know only a message and `BzrError::input_field(msg, field, value)` for the
   local value-shape validators that know both (date/sort/fields parsing,
   `--from-json` required-field checks). `field`/`value` are emitted only when
   present, honoring "when known".

3. **`BatchPartialFailure` reports summary counts; per-element failures stay on
   stdout.** The error object carries `succeeded`/`failed`. The authoritative
   per-element `failed[]` array is already published on **stdout** in the batch
   result body (`batch-create-result.json` / `batch-result.json`), emitted before
   the exit-11 error by `write_batch_create` and `write_attachment_batch`. The
   error variant is not widened to re-carry it.

## Consequences

- New error keys (`field`, `value`, `bug_id`, `last_change_time`,
  `if_match_token`, `resource`, `identifier`, `status`, `api_code`, `succeeded`,
  `failed`, `server`, `expected`, `actual`) are all optional in one flat schema
  object. The schema therefore guards the key *vocabulary* and types but **cannot**
  enforce per-variant presence (no `if/then` per `type`); per-variant presence is
  guarded by wiremock unit tests, which assert the specific keys, not just schema
  validity.
- The `InputValidation` reshape touches ~101 construction sites plus the
  function-pointer use at `src/main.rs:186` (`map_err(BzrError::InputValidation)`
  → `BzrError::input`). The migration is compiler-driven; a clean `cargo build`,
  not a text substitution, is the source of truth.
- `error.rs` stays decoupled from per-command result types: it never imports
  `CreateFailure` et al. The cost is that an agent reading *only* stderr on a batch
  failure sees counts, not per-element rows — documented in `docs/bzr-cli.md` as
  "read `failed[]` from the stdout result body".

## Considered & rejected

- **A second `FieldValidation` variant** (instead of reshaping `InputValidation`).
  Avoids the 101-site churn, but splits one logical error kind across two variants
  that share `error_type`/`exit_code`, inviting "which do I construct?" drift. One
  variant with optional attribution is the cleaner data model; the churn is
  mechanical and compiler-verified.
- **Threading `Vec<CreateFailure>` into `BatchPartialFailure`** to emit a
  `failures` array on stderr. Rejected: it duplicates a contract already published
  on stdout and couples `error.rs` to per-command result types for no new
  information.
- **Moving the error to stdout** (the original #459 ask). Rejected: reverses
  ADR-0007's stdout=data / stderr=diagnostics decision two days after release and
  breaks both streams of a released contract; agents already detect failure via
  exit code and read the stderr error object.
- **Per-`type` `if/then` subschemas** to enforce presence. Rejected as
  disproportionate; unit tests enforce presence more legibly than conditional JSON
  Schema.
