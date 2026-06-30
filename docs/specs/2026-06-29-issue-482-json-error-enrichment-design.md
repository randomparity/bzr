# Issue #482 — Enrich the structured JSON error body

- Status: Draft
- Date: 2026-06-29
- Issue: #482 (carved out of the closed #459)
- Related: ADR-0007 (`schema_version` envelope), ADR-0014 (this design's decisions), `schemas/error.json`

## Background

v0.7.0 ships a structured error object on **stderr** under `--json` /
`--output ndjson` (introduced 2026-03-18, contract published 2026-06-21 as
`schemas/error.json`, versioned 2026-06-26 by ADR-0007):

```json
{ "schema_version": "0.6.0",
  "error": { "type": "input", "message": "…", "exit_code": 7 } }
```

It is produced by `main::format_dispatch_error(err, format)` and written to
`writers.err`. The body carries only the three universal keys: `type`
(`BzrError::error_type()`), `message` (`Display`), `exit_code`
(`BzrError::exit_code()`).

The gap: the rich detail an agent needs to *act* — which field was rejected,
what the offending value was, the change tokens needed to retry a collision — is
flattened into the `message` string. This spec adds discrete, machine-readable
keys, **additively**, on the same stderr channel.

## Goals

- Add optional, variant-specific keys to the `error` object so an agent can
  branch on structured data instead of parsing prose.
- Keep the change additive: existing keys, channel (stderr), and exit codes are
  unchanged; consumers that ignore unknown keys are unaffected.
- Keep the contract self-describing: `schemas/error.json` and `bzr schema error`
  publish the enriched shape; the conformance test stays green.

## Non-goals (settled, do not reopen)

- **Moving the error to stdout / reverting stderr to prose.** Rejected when #459
  was split: it contradicts ADR-0007's deliberate stdout=data / stderr=diagnostics
  split. Out of scope here.
- **Renaming `exit_code` → `code`.** Keep the released field name; additive only.
- **A `schema_version` major/minor bump.** All changes here are additive new keys
  → **patch** bump per the ADR-0007 stability policy.

## Design

### The seam: `BzrError::structured_detail()`

Add one method in `error.rs`, beside `error_type()` / `exit_code()`:

```rust
/// Variant-specific structured keys for the `--json` / ndjson error object,
/// beyond the universal `type` / `message` / `exit_code`. Empty for variants
/// with no machine-useful detail. Every key is additive and OPTIONAL in
/// `schemas/error.json`.
pub fn structured_detail(&self) -> serde_json::Map<String, serde_json::Value>;
```

`format_dispatch_error` seeds the object with the `structured_detail()` entries
and then writes the universal `type` / `message` / `exit_code` keys **last**, so
the three contract-bearing keys can never be clobbered by a variant's detail map.
`structured_detail()` must never emit `type`, `message`, or `exit_code`; a unit
test asserts those three keys are present and correct on every variant.
Centralizing the mapping next to the existing classification methods keeps the
contract-bearing methods together and lets the conformance test cover them
uniformly.

### Per-variant keys

| Variant | Added keys |
|---------|-----------|
| `MidAirCollision { id, expected, actual }` | `bug_id` = id, `last_change_time` = actual, `if_match_token` = expected |
| `InputValidation { message, field, value }` | `field`, `value` — **only when present** |
| `NotFound { resource, id }` | `resource`, `identifier` = id |
| `HttpStatus { status, .. }` | `status` |
| `Api { code, .. }` | `api_code` = code |
| `BatchPartialFailure { succeeded, failed }` | `succeeded`, `failed` (counts) |
| `PinMismatch` / `IssuerChanged` | `server`, `expected`, `actual` |

All other variants contribute no extra keys (empty map).

### `InputValidation` field/value — variant reshape

`InputValidation(String)` has 101 construction sites. To carry `field`/`value`
as first-class data we reshape it to a struct variant:

```rust
InputValidation { message: String, field: Option<String>, value: Option<String> }
```

with constructors that keep the common path terse and let the bounded set of
value-shape validators supply attribution:

```rust
BzrError::input(msg)                 // field: None, value: None  (the 101 sites)
BzrError::input_field(msg, field, value)   // the validators that know both
```

The 101 existing `BzrError::InputValidation(x)` call sites become
`BzrError::input(x)` (mechanical; **compiler-driven, not sed-driven**). `Display`
changes from `#[error("{0}")]` to `#[error("{message}")]`; the `match` arms in
`exit_code()` / `error_type()` use `InputValidation { .. }`. Note the
**function-pointer** form: sites that pass the tuple constructor as a value —
`val.parse().map_err(BzrError::InputValidation)` at `src/main.rs:186`, and any
`ok_or`/`map_err` of the same shape — stop compiling because the struct variant
is no longer a `Fn(String) -> BzrError`; they must become `BzrError::input`. A
clean `cargo build` is the source of truth for finding every site; a text
substitution on `InputValidation(...)` would miss these. Initial attribution lands at the
local validators that cleanly know both halves: `validation/datetime.rs`
(`flag` = field, `s` = value), sort-order and `validation/fields` parsing, and
`--from-json` required-field checks. Other sites keep `None`, satisfying AC #4's
"when known" honestly.

**Rejected alternative — a second `FieldValidation` variant.** Avoids the
101-site churn but splits one logical error kind across two variants that share
`error_type` / `exit_code`, inviting "which one do I construct?" drift. One
variant with optional attribution is the cleaner data model; the churn is
mechanical and compiler-verified.

### `BatchPartialFailure` — counts, not a duplicated `failures` array

Finding: on a partial batch failure, the command **already emits the full
per-element `failed[]` array to stdout** as the `BatchCreateResult` /
`DownloadResult` body (schemas `batch-create-result.json`, `batch-result.json`,
each with `index` / `error` / `step`) *before* `ensure_batch_complete` returns
`BatchPartialFailure`. The variant holds only counts and, at the point
`format_dispatch_error` runs (after dispatch has returned), has no access to the
per-element failures.

Decision: the **error object carries the summary counts** (`succeeded`,
`failed`); the authoritative per-element detail stays where it already is — the
stdout result body. We do **not** thread the `Vec<CreateFailure>` into the error
variant solely to re-emit it on stderr; that would duplicate a published
contract and couple `error.rs` to per-command result types. `docs/bzr-cli.md`
documents that for batch verbs the per-element failures are read from the stdout
result's `failed[]`, with the stderr error giving the summary + exit code.

This **changes** #482's acceptance criterion, not merely "refines" it: the
issue's "BatchPartialFailure includes a `failures` array" is satisfied by the
already-published stdout result body (`failed[]`), and the stderr error object
carries the summary counts plus the exit code rather than a copy. **Resolution:
#482's acceptance criteria are updated to state this explicitly** (the error
object carries `succeeded`/`failed`; per-element detail is read from the stdout
result `failed[]`), so the implementation matches a written criterion rather than
silently deviating. Verified: both batch producers emit the full per-element
result to stdout before erroring — `write_batch_create` (`bug create
--from-json`, `create_json.rs`) and `write_attachment_batch` (`attachment
download`, `download.rs:243`) — so no per-element detail is lost under this
decision.

## Schema, version, docs, tests

- **`schemas/error.json`**: add the new keys under `error.properties` as
  optional (not in `required`); the object keeps `additionalProperties: false`.
  Update the schema `description` (still stderr). Keep `exit_code` bounds `1..=14`.
  - **Caveat on the existing conformance test.** `assert_error_matches_schema`
    (`main_tests.rs`) is a hand-rolled **partial** check: it asserts only the three
    universal keys + `exit_code` bounds and does **not** enforce
    `additionalProperties` or validate the new keys. So adding emitted keys does
    *not* fail it, and it does *not* by itself keep schema and formatter in lockstep.
    This work **strengthens** that test so an emitted key absent from the schema's
    `error.properties` fails it (manual `additionalProperties:false` enforcement) —
    that is the guardrail that keeps schema and formatter moving together.
  - **The schema is intentionally permissive about presence.** A single flat
    object with optional keys cannot express "a `collision`-type error MUST carry
    `if_match_token`" without per-type `if/then`/`oneOf` subschemas, which this
    spec deliberately does not add (the error object is keyed by `type`; agents
    branch on `type` first). Consequently schema-validity does **not** prove a
    variant emitted its expected keys. Per-variant key **presence** is enforced by
    the wiremock unit tests below — each must assert the specific keys are present
    (e.g. `MidAirCollision` ⇒ `if_match_token` present), not merely that the
    output is schema-valid. The schema guards the key *vocabulary* and types; the
    tests guard per-variant *presence*.
- **`SCHEMA_VERSION`**: `0.6.0` → `0.6.1` (additive/patch). Update the envelope,
  docs example, and `schema_version`-asserting tests.
- **`docs/bzr-cli.md`** "JSON Output": document each new key and the batch
  stdout-vs-stderr split.
- **`agent-skills/.../bzr-triage-bug`**: teach the branch-on-error pattern (read
  exit code → parse the stderr `error` object → switch on `type`, then `field` /
  `last_change_time` / `if_match_token`). Honor the agent-skill drift rules.
- **`CHANGELOG.md`**: add an `### Added` entry under `## [Unreleased]`.
- **Wiremock tests**: `InputValidation` (field/value), `HttpStatus` 404,
  `BatchPartialFailure` (counts), `MidAirCollision` (tokens), driven through the
  formatter and validated against the published schema. Each test additionally
  **asserts the specific variant keys are present** (e.g. `MidAirCollision` ⇒
  `last_change_time` and `if_match_token` non-null), since schema-validity alone
  does not prove presence. A cross-variant test asserts `type`/`message`/
  `exit_code` are present and correct on every `BzrError` variant.
- **Functional phase**: a `tests/functional/phases/` script triggers a `--json`
  error against a real container (e.g. a not-found or invalid-field mutation) and
  asserts the stderr `error` object shape.

## Success criteria

1. `bzr <failing cmd> --json` emits, on stderr, an `error` object whose
   `type`/`exit_code` match the `BzrError` methods and whose variant-specific
   keys appear per the table above.
2. `MidAirCollision` carries `last_change_time` + `if_match_token`;
   `InputValidation` from the attributed validators carries `field` + `value`.
3. `bzr schema error` and `schemas/error.json` validate every emitted shape; the
   conformance test passes.
4. stdout, exit codes, and the stderr channel are unchanged for non-error and
   table-mode paths.
