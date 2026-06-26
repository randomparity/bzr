# Issue #464 — Stable JSON schema versioning for `--json` output

- Issue: #464
- ADR: [0007](../../adr/0007-json-output-schema-version-envelope.md)
- Status: Draft
- Date: 2026-06-26

## Problem

`bzr schema` publishes JSON Schemas, but `--json` output carries no version field
and there is no documented stability policy. An agent that caches a parsed shape
(a `jq` recipe, a typed deserializer) silently breaks when a release renames or
restructures a field. The `commands.yml` drift check keeps the *command surface*
honest; nothing guards the *JSON contract*.

## Decision summary (operator-chosen)

Two design forks were settled by the operator (see ADR 0007 Context):

1. **Surface approach: wrap every `--json` output** in an envelope
   `{"schema_version": "<semver>", "data": <existing payload>}`. This is a
   **breaking** restructure of the `--json` contract (not the additive patch the
   issue's acceptance #5 assumed) and is treated as such.
2. **Version source: a separate, manually-maintained `SCHEMA_VERSION` constant**,
   decoupled from the crate version, bumped per the documented stability policy.

## The contract rule (single, falsifiable)

> A top-level `schema_version` string is present **if and only if** the resolved
> output format is `Json` (i.e. pretty `--json`).

| Format | Success output | Error output (stderr) |
|--------|----------------|-----------------------|
| `Json` (`--json`) | `{"schema_version":"X","data":<payload>}` | `{"schema_version":"X","error":{…}}` |
| `Ndjson` (`--output ndjson`) | bare records, one per line (unchanged) | bare `{"error":{…}}` one line (unchanged) |
| `Table` (default TTY) | unchanged | `error: …` (unchanged) |

Rationale for the iff rule:
- Acceptance #4 requires `--output ndjson` *not* to carry the version on every
  streamed line. Tying the envelope to the `Json` arm satisfies this with one
  rule that covers success and error symmetrically.
- `Ndjson` consumers (the streaming, line-oriented audience) are therefore
  **unaffected** by this change. The break is confined to pretty `--json`.

## Seams

There are exactly two JSON-emitting seams; the envelope is applied at both, gated
on `OutputFormat::Json`:

1. **Success:** `src/output/formatting.rs::write_json` — the single chokepoint all
   pretty-JSON success output already flows through (`write_json_family` and
   `write_formatted` both route their `Json` arm here; `write_result`/
   `write_count`/`write_saved` route through `write_json_family`). Wrapping here
   guarantees exactly one envelope per `--json` invocation.
2. **Error:** `src/main.rs::format_dispatch_error` — a separate hand-built emitter.
   For `Json`, prepend `schema_version`; for `Ndjson`, leave the existing bare
   compact line untouched.

### Exemptions (must NOT be wrapped)

- **`--output ndjson`** — `write_ndjson` is a distinct path; never wrapped.
- **`bzr schema <name>`** — `schema.rs::write_one` writes a JSON-Schema *document*
  verbatim via `write!`, bypassing `write_json`. Wrapping it would corrupt the
  emitted schema document. It stays exempt (the document is its own contract, not
  a command result payload). This is verified by an explicit test.
- **`Table`** output — never JSON.

### Consequence for `bzr schema` (list)

`bzr schema --json` (no name) flows through `write_result` → `write_json`, so it
is wrapped automatically: `{"schema_version":"X","data":["action-result",…]}`.
This satisfies acceptance #2 ("`bzr schema --json` reports the version") without a
bespoke `{schema_version, schemas}` shape — the universal envelope already carries
the version, and `data` is the uniform payload key. (The issue's illustrative
`schemas` key predates the decision to use a uniform envelope.)

## Version constant

```rust
// src/output/mod.rs
/// Version of the `--json` output contract. Bumped manually per the JSON Output
/// Stability policy in docs/bzr-cli.md, independent of the crate version.
pub const SCHEMA_VERSION: &str = "0.6.0";
```

- Initial value `0.6.0`: the inaugural published contract version. The envelope's
  introduction is itself the breaking change from *unversioned* output; from this
  baseline forward the semver policy below governs changes.
- Format: `MAJOR.MINOR.PATCH` (no pre-release tags). Validated by the envelope
  schema `pattern` and a unit test.

## Stability policy (documented in `docs/bzr-cli.md` and mirrored in skill)

A new "JSON Output Stability" subsection under the existing JSON Output docs:

- **Patch** (`x.y.Z`): additive only — a new field added to a payload or envelope.
  Consumers ignoring unknown fields are unaffected.
- **Minor** (`x.Y.0`): field rename or restructure, shipped with a one-release
  deprecation alias (old + new field both present for one minor release).
- **Major** (`X.0.0`): breaking removal/retype with no alias.
- `schema_version` reflects the **envelope + payload** contract as a whole.
- `--output ndjson` records do not carry `schema_version`; consumers read the
  version from `bzr schema --json` (`.schema_version`) or the binary's `--version`.

## Schemas

- **Add `schemas/envelope.json`** (registered in `SCHEMAS`, kept sorted):
  describes `{"schema_version": <semver string>, "data": <any>}`,
  `required: ["schema_version","data"]`, `additionalProperties: false`.
- **Update `schemas/error.json`**: add an optional top-level `schema_version`
  (string), `required` unchanged (`["error"]`), so both the `--json` (versioned)
  and `--output ndjson` (bare) error shapes validate. Description notes
  `schema_version` is present only under `--json`.
- Per-payload schemas (`bug.json`, `attachment.json`, …) are **unchanged**: they
  describe `data` contents, validated at the type level by the existing drift
  tests (`serde_json::to_value(typed_value)`), which the output-layer envelope
  does not touch.

## Success criteria (falsifiable)

1. `bzr ... --json` for every read/list/view/mutation path emits a top-level
   object with `schema_version` (matching `^\d+\.\d+\.\d+$`) and `data`.
2. A wiremock test asserts `schema_version == bzr::output::SCHEMA_VERSION` and the
   `data` payload equals the previously-bare shape.
3. `bzr schema --json | jq -r '.schema_version'` prints the version;
   `jq '.data'` is the array of names.
4. `bzr schema bug --json` still emits the verbatim Bug JSON-Schema document (no
   envelope), asserted by test.
5. `bzr ... --output ndjson` output is byte-for-byte unchanged (no per-line
   version); a regression test pins this.
6. `--json` error output carries `schema_version`; `--output ndjson` error output
   does not; both asserted.
7. `make skills-test`, `cargo test`, `make lint` green. A functional phase
   exercises `schema_version` against a real container (incl. the credentialless
   read path).
8. `docs/bzr-cli.md` documents the envelope + stability policy; `json-recipes.md`
   recipes updated to `.data.*` / `.data[].*` and note the version field;
   `CHANGELOG.md` records the breaking change.

## Edge cases & risks

- **Double-wrap:** impossible — only `write_json` wraps, and each command invokes
  one write helper once. A grep audit confirms no `write_json`/`write_json_family`
  call sites inside output loops (the only loop candidate,
  `write_attachment_batch`, serializes one aggregate value).
- **Empty payloads:** `bzr bug search` with no hits → `{"schema_version":"X",
  "data":[]}` (array preserved inside `data`). `--count` → `data:{"count":0}`.
- **`error.json` dual shape:** handled by making `schema_version` optional there.
- **Existing consumers break:** every `jq` recipe, doc example, integration
  assertion, and functional phase that reads `--json` must move to `.data`. This
  is the bulk of the diff and is enumerated in the plan.
