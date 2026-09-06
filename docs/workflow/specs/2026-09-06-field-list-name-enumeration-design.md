# `bzr field list` name enumeration — design

Issue: [#718](https://github.com/randomparity/bzr/issues/718) — "No way to enumerate the
field names a server accepts".
Decision record: [ADR 0062](../../adr/0062-field-list-enumerates-the-accepted-write-field-set.md).

## Problem

The `--field KEY=VALUE` / `--field-json` write surface (ADR 0053) accepts a key when the
connected server's `field/bug` catalogue declares it **or** when `BUG_FIELDS`
(`src/types/bug/fields.rs:176`) models it. Neither half is enumerable from the CLI, so
the only way to find a valid key is to guess and read the rejection.

## Goal

`bzr field list` with no positional prints every bug field name a `--field` write will
accept, each row marked with why it is accepted.

## Non-goals

- Pairing internal catalogue names with their REST equivalents in the data
  (`status_whiteboard` ↔ `whiteboard`). ADR 0062 records why: no source for the pairing
  exists, and building one means a hand-written table.
- `--custom` / `--all` filters.
- Any change to `bzr server capabilities`, `bzr field aliases`, or
  `bzr field list <name>`.
- Persisting the probed names to `ServerConfig.bug_field_names`.

## Behaviour

### Surface

```
bzr field list                      # NEW: enumerate accepted field names
bzr field list <FIELD_NAME>         # unchanged: legal values of one field
```

The positional becomes optional. No new flag is added. `--fields` /
`--exclude-fields` (`ProjectionArgs`) already flatten into the subcommand and apply to
both forms, against different key sets.

### Data

One row per accepted name:

| key | type | meaning |
|-----|------|---------|
| `name` | string | the field name a `--field` write accepts |
| `source` | `"server"` \| `"bzr"` \| `"both"` | why it is accepted |

- `server` — the connected server's `field/bug` catalogue declares it, and `BUG_FIELDS`
  does not model it.
- `bzr` — `BUG_FIELDS` models it as a canonical REST name, and the catalogue does not
  declare it.
- `both` — both.

Rows are sorted by `name`, ascending, byte order. Names are unique: the union is built
through a `BTreeMap<String, FieldNameSource>` keyed on the name, so a name in both
sources yields exactly one `both` row rather than two rows.

### Output

- Table (`--output table`, the default at a TTY): two columns, `NAME` and `SOURCE`,
  through `write_table_records` like every other table writer.
- `--json` / `--output ndjson`: the `field-name` shape, through
  `write_formatted_projected`, so `--fields` / `--exclude-fields` project it.

The listing is never empty: `BUG_FIELDS` contributes 28 rows unconditionally, even
against a server whose catalogue is empty. There is therefore no empty-listing message,
and no branch for one.

### Errors

The no-argument form makes exactly one request, `GET /rest/field/bug?include_fields=name`,
through the existing `BugzillaClient::bug_field_names()`. Any failure propagates
unchanged — same variant, same exit code — because a listing has no partial answer worth
printing. `--fields` with an unknown key exits 7, as on every other projected verb.

`bzr field list <name>` keeps every existing behaviour, including the
`No values for field '<name>'.` table message and `NotFound`
(`EXIT_CODE_NOT_FOUND`, `src/error.rs:144` — exit 2) for a name the server does not
recognise.

### The `--field` rejection message

`undeclared()` in `src/commands/runtime/shared/field_catalogue.rs` currently reads:

```
--field: this server does not declare a field named '<key>'; run `bzr server capabilities` to see the custom fields it declares
```

It becomes:

```
--field: this server does not declare a field named '<key>'; run `bzr field list` to see every field name this server accepts
```

The hedge in the old wording existed because no command could show the whole accepted
set. One now can, so the hedge and the comment explaining it are removed rather than
left to read as false.

## Agreement (acceptance criterion 2)

> The listing and the `--field` validator agree: anything listed is accepted, and the
> relationship for anything accepted but not listed is documented.

This is made structural, not documentary. Both sides read the same two sources through
one function in the module that already owns "what `--field` accepts":

```rust
// src/commands/runtime/shared/field_catalogue.rs
pub(crate) fn accepted_bug_fields(declared: &[String]) -> Vec<FieldName>
```

`validate_bug_fields()` keeps its existing fast paths and its existing predicate; what
changes is that `is_bzr_known_bug_field` and `accepted_bug_fields` are the only two
readers of `BUG_FIELDS` in this module, and `accepted_bug_fields` is defined in terms of
`is_bzr_known_bug_field` so the two cannot disagree about a name.

**Nothing accepted is unlisted.** `validate_bug_fields` accepts exactly
`{k : is_bzr_known_bug_field(k)} ∪ {k : k ∈ declared}`, and `accepted_bug_fields(declared)`
emits exactly that union. The second half of the criterion is therefore vacuous rather
than documented: there is no accepted-but-unlisted name, subject to the two caveats
below, which `docs/bzr-cli.md` states.

Two caveats, both stated in the docs:

1. The listing reflects the catalogue *at the moment of the call*. A field added to the
   server between the listing and the write is accepted without having been listed. The
   validator re-probes on a miss, so this direction never rejects a real field.
2. Listed means "bzr will not refuse this key", not "the server will honour it".
   Bugzilla silently ignores write keys it does not want (ADR 0053), and bzr cannot know
   which those are.

## Schema

New published output shape `field-name`, `schemas/field-name.json`. Adding a result type
is additive under ADR 0007, so `SCHEMA_VERSION` moves 3.0.2 → 3.0.3.

The five coupled updates a new `--json` shape requires:

1. `schemas/field-name.json`.
2. The `SCHEMAS` registry in `src/commands/schema.rs`, sorted — `field-name` sorts before
   `field-value`.
3. A conformance case in `src/commands/schema_tests.rs`. `assert_conforms` checks
   top-level keys only, so the case must also assert the `source` enum, which nests
   nowhere but is still not something `assert_conforms` validates against the instance.
4. The available-schemas list in `docs/bzr-cli.md`.
5. Input parser-key drift — **not applicable**: `field-name` is an output shape, and the
   drift check covers `*-input.json` payload parsers only.

## Testing

### Unit

- `src/types/field_tests.rs` — `FieldNameSource` serializes to the three lowercase
  strings; `FieldName` round-trips.
- `src/commands/runtime/shared/field_catalogue_tests.rs` — `accepted_bug_fields` against
  a catalogue that overlaps `BUG_FIELDS` partially: a server-only name, a bzr-only name,
  and an overlapping name each get the right `source`; a duplicate in `declared` yields
  one row; an empty `declared` yields exactly the 28 `BUG_FIELDS` names, all `bzr`.
- **The agreement test.** Drive `validate_bug_fields` with the exact key set
  `accepted_bug_fields` produced, against a `wiremock` server serving the same catalogue,
  and assert `Ok(())`. This is the executable form of criterion 2 and it discriminates:
  emitting a name neither source backs makes it fail.
- `src/output/resources/field_tests.rs` — table headers and row order; JSON projection to
  `name` only.
- `src/commands/field_tests.rs` — the no-argument form against `wiremock`; the named form
  unchanged; `--fields source` projects; `--fields bogus` exits 7.
- `src/cli/field_tests.rs` — `field list` parses with `name: None`, `field list status`
  with `name: Some`.

### Functional (`make functional-test`)

Required by the acceptance criteria, including the credentialless path.

`tests/functional/phases/05-fields-classifications.sh` — the listing itself:

- `field list` succeeds and contains both a `server` row and a `bzr` row. Asserting both
  sources are present is what discriminates a union from either half alone; a
  catalogue-only regression drops every `bzr` row and a `BUG_FIELDS`-only regression
  drops every `server` row.
- The listing contains `status_whiteboard` with `source == "server"` and `whiteboard`
  with `source == "bzr"` — the concrete pair ADR 0053 names, so the assertion fails on
  the exact regression the issue is about rather than on an abstraction of it.
- `field list --fields name` projects to `{name}` only; `--fields bogus_xyz` exits 7.
- Credentialless: `--server public field list` succeeds and still carries both sources.

`tests/functional/phases/08g-bug-arbitrary-fields.sh` — agreement against a live server:

- The rejection message names `bzr field list` (replacing the `bzr server capabilities`
  assertion added by #283).
- The `advice-names-a-command-that-works` test runs `bzr field list` instead of
  `bzr server capabilities`, and its comment about why `field list` could not be that
  command is deleted.
- **The live agreement oracle.** Read a `source == "server"` name *out of the listing*
  with `jq`, then feed that name to `bug update --field <name>=…` and require it not to
  be rejected. Deriving the name from the listing rather than hard-coding it is what
  makes the assertion bite in both directions: a listing that omits server names yields
  an empty name and the guard's `else` branch calls `test_fail`, and a listing that emits
  a name the validator rejects exits 7.

Every fixture guard in the new blocks gets an `else` that calls `test_fail`. None of the
new assertions is skippable: the containers always declare a catalogue, so an absent
fixture is a real failure, not a reason to skip.

## Files

| path | change |
|------|--------|
| `src/cli/field.rs` | `name: Option<String>`; doc comment covers both forms |
| `src/commands/field.rs` | dispatch on `name`; the no-argument branch |
| `src/types/field.rs` | `FieldName`, `FieldNameSource`, `FIELD_NAME_FIELDS` |
| `src/output/resources/field.rs` | `write_field_names` |
| `src/commands/runtime/shared/field_catalogue.rs` | `accepted_bug_fields`; `undeclared()` rewording |
| `src/commands/schema.rs` | `"field-name"` in `SCHEMAS` |
| `src/output/mod.rs` | `SCHEMA_VERSION` 3.0.2 → 3.0.3 |
| `schemas/field-name.json` | new |
| `docs/bzr-cli.md` | command tree, `field list` section, projection table, schema list |
| `tests/functional/phases/05-fields-classifications.sh` | listing coverage |
| `tests/functional/phases/08g-bug-arbitrary-fields.sh` | message + agreement oracle |
| sibling `*_tests.rs` of each source file above | unit coverage |

## Security

Not security-relevant, and the trigger list in `$quest` step 6 is the test applied:

- No new entry point an untrusted actor reaches. The no-argument form is a new *shape* of
  an existing authenticated-or-anonymous read on a path bzr already calls
  (`bug_field_names()`, used by the `--field` validator today).
- No authn/authz, tenancy, session, or secret handling changes.
- No new deserialization: `FieldBugResponse` / `FieldDefinition` are unchanged and already
  parse this response.
- No command, query, path, URL, or template is built from a non-literal. The no-argument
  form takes no user-controlled input into the request at all — the query string is the
  fixed `include_fields=name`.
- No permission grant, dependency, file mode, TLS, or security-relevant default changes.

One boundary is worth naming even though it is not new: the server controls the field
names printed. They are written through the existing table and JSON writers, which is the
same handling `bzr server capabilities` already gives catalogue-supplied custom field
names, so no new destination-encoding concern is introduced.
