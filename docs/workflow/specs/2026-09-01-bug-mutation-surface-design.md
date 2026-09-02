# Bug mutation surface design

Issue: [#623](https://github.com/randomparity/bzr/issues/623)
Decision: [ADR 0036](../../adr/0036-preserve-bug-create-group-presence.md)

## Goal

Make `bug resolve` usable on site-defined workflows and make the existing structured bug-create
input able to opt out of product default groups.

## Scope and constraints

- Rust remains at 1.89.0 and no dependency is added.
- `bug resolve --status <STATUS>` defaults to `RESOLVED` and uses the exact, case-sensitive
  `Bug.fields` status validation already used by `close` and `reopen`.
- Local empty-status validation runs before network setup. A non-dry-run unknown status exits 7
  with `no status named '<status>' on this server; valid statuses: ...` and sends no update.
- Dry-run preserves the shared mutation rule: it validates locally and previews the requested
  status without connecting to Bugzilla.
- `bug create --from-json` is the empty-groups entry point. Missing `groups` omits the API member;
  `groups: []` sends an empty array; a non-empty array sends its values. JSON `null` stays invalid.
- Non-empty `--groups` overrides structured input exactly as today. Flag, template, and clone paths
  with no group values continue omitting the member.
- `schemas/bug-create-input.json` and `SCHEMA_VERSION` remain byte-identical because the schema
  already describes an optional array, including an empty array.
- The campaign-assigned ADR is 0036. Its index row remains pending for the campaign orchestrator.

## Components and data flow

`ResolveArgs` gains a `status: String` with Clap's `RESOLVED` default. The command's `resolve`
implementation follows the existing `close` path: resolve comment input, validate a non-empty
status locally, assemble one `ApplyRequest`, and either send the dry-run through `apply_checked` or
connect once, validate the name with `get_field_values("status")`, and reuse that client through
`apply_checked_connected`.

`CreateBugParams.groups` remains the existing public `Vec<String>`. The non-exhaustive payload adds
a crate-visible, non-public `groups_present` marker and replaces derived serialization with a
private wire view containing one computed `groups: Option<&[String]>` field. The wire view omits an
empty vector unless the marker is true, and otherwise serializes the vector once. This remains
duplicate-free even if in-crate code mutates the public vector after the marker is set. Ordinary
create, clone, and external Rust callers retain their existing source and wire behavior; existing
in-crate full struct expressions initialize the marker to false.
`JsonCreateBug` uses a small presence-preserving `JsonGroups` input value: its default is absent,
while its deserializer accepts only a JSON array and records that array as present. The overlay
replaces this value only when the CLI supplied a non-empty group list. Conversion calls the
payload setter only for present structured input, causing an explicit empty array to use the
presence marker and non-empty input to use the public field.

## Error handling and compatibility

Resolve status validation introduces no new error type or wording; it extends the exact helper and
single-client path used by its sibling verbs. The server remains responsible for whether a valid
status is a legal transition from each bug's current state.

Group presence is an internal serialization correction. The public `CreateBugParams.groups` Rust
field remains a `Vec<String>`, and existing omitted and non-empty inputs keep their source and wire
shapes. `groups: null` remains a structured-input validation error, matching the unchanged schema.
No persisted configuration or public result shape changes.

## Threat model

### Boundary inventory

- Widened existing boundary: a local operator-controlled `--status` value enters the resolve
  command and becomes a Bugzilla status field.
- Corrected existing boundary: local operator-controlled structured JSON distinguishes a missing
  groups member from an empty array before becoming the outbound Bugzilla create payload.
- Existing boundary, not widened: Bugzilla-controlled status names enter validation error text.

### Actors and controls

The local operator controls CLI and JSON input. Resolve rejects empty input locally, matches status
names exactly against the authenticated server's field list, and uses typed JSON serialization
rather than string construction. Structured groups remain bounded by the existing input-file and
JSON parsing path and serialize as a typed string array; no shell, path, query, or template
destination is introduced. Status error output can reveal only status names already visible to the
authenticated operator through the same API.

### Out of scope

This change does not add authorization: Bugzilla continues deciding whether the authenticated user
may resolve a bug or assign groups. It does not validate transition legality because that depends
on each bug's current state and remains the server's responsibility, as for close and reopen.

## Testing and acceptance

- Add a resolve command test whose status field fixture includes a custom name and whose update
  body must carry it; first run `make test-one T=resolve_with_status_override` before implementation
  and record the compile failure caused by the missing `status` field.
- Add structured-create tests that distinguish omitted groups from an explicit empty array at the
  request body. First run `make test-one T=from_json_explicit_empty_groups` before implementation
  and record the body-matcher failure showing the empty member was omitted.
- Before replacing derived payload serialization, add two exact parity oracles for
  `CreateBugParams`: one maximally populated object covering every field name/value and one
  minimal/default object whose complete JSON contains only the four required fields. Run both
  against the derived baseline and again unchanged against the private wire view so every
  unchanged field name, value, and omission rule remains covered.
- Preserve and extend default resolve and non-empty create-group coverage.
- Extend phase 11b by first exercising and asserting the no-flag `RESOLVED` transition, then invoke
  the stock non-default `VERIFIED` status on that resolved bug and assert `VERIFIED` with its
  resolution. Stock Bugzilla permits `RESOLVED -> VERIFIED` across the supported images, and the
  second assertion fails if command assembly ignores the override. Add client-side invalid-status
  rejection separately.
- Extend the restricted-access phase with a dedicated product whose group control is default (2),
  not mandatory (3). Assert omission applies that group, then create with `groups: []` and assert
  the resulting bug's group list is empty.
- Run focused tests, `make test-fast`, `make lint`, `make test`, and
  `make functional-test-all` across bz50, bz52, and bz53.

## Rollback

Revert the CLI/command and payload/input commits with their tests and docs. No migration,
persisted-data repair, or server cleanup is needed; functional fixtures are scoped to the harness.
