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
  (`status_whiteboard` ↔ `whiteboard`). ADR 0062 records why: nothing bzr parses carries
  the pairing, so building one means a hand-written table.
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
  `write_formatted_projected`, so `--fields` / `--exclude-fields` project it. Both JSON-family
  formats come from that one helper, which the named form already uses, so ndjson needs no
  coverage of its own beyond the `--json` test.

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

Made structural, not documentary: both sides read the same two sources through one
function in the module that already owns the accept rule.

```rust
// src/commands/runtime/shared/field_catalogue.rs
pub(crate) async fn accepted_bug_fields(client: &BugzillaClient) -> Result<Vec<FieldName>>
```

It takes the client, not a `&[String]` of names, so it fetches the catalogue through the
same `bug_field_names()` the validator probes with. A parameter would type-check against
any `Vec<String>` and leave the two bound only by what one call site passes.

`validate_bug_fields` accepts exactly
`{k : is_bzr_known_bug_field(k)} ∪ {k : k ∈ declared}`, where `declared` is what
`bug_field_names()` returned; `accepted_bug_fields` emits exactly that union, over the
same `is_bzr_known_bug_field` and the same `bug_field_names()`. ADR 0062 records why the
sources are what they are; this section states only what the criterion requires.

**Given one snapshot of the catalogue, nothing accepted is unlisted.** Three caveats
bound that, and all three go in `docs/bzr-cli.md`:

1. **A removed field is the one accepted-but-unlisted case.** `validate_bug_fields`
   short-circuits on a `ServerConfig.bug_field_names` cache hit without probing (the
   `cached_names` fast path in `field_catalogue.rs`), so a field the server drops after its names were
   cached stays accepted on a write while the listing, which always probes, no longer
   shows it. ADR 0053 records this residual and accepts it as the price of the cache;
   closing it means probing on every write. Documented, not fixed.
2. **A field added since the listing is safe.** It is a cache *miss*, and a miss always
   re-probes, so this direction never rejects a real field.
3. **Listed means bzr will not refuse the key**, not that the server will honour it.
   Bugzilla silently ignores write keys it does not want (ADR 0053). This covers both
   halves of the union: read-only catalogue names, and the read-only entries of
   `BUG_FIELDS` — which is a `--fields` read-projection list, so `id`, `creator`,
   `creation_time`, and `last_change_time` are listed and are not settable.

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

- `src/types/field_tests.rs` — `FieldNameSource` serializes to the three lowercase strings,
  and `FIELD_NAME_FIELDS` equals the serialized key set (mirroring the existing
  `field_value_fields_matches_serialized_keys`).
- `src/commands/runtime/shared/field_catalogue_tests.rs` — `accepted_bug_fields` against
  a catalogue that overlaps `BUG_FIELDS` partially: a server-only name, a bzr-only name,
  and an overlapping name each get the right `source`; a duplicate in `declared` yields
  one row; an empty `declared` yields exactly the 28 `BUG_FIELDS` names, all `bzr`.
- **The agreement test.** Drive `validate_bug_fields` with the exact key set
  `accepted_bug_fields` produced, against a `wiremock` server serving the same catalogue,
  and assert `Ok(())`. This is the executable form of criterion 2 and it discriminates:
  emitting a name neither source backs makes it fail.
- `src/output/resources/field_tests.rs` — table headers; JSON projection to `name` only, with
  the negative half (`source` absent) as the assertion that bites. The table `source` spelling
  needs no test of its own: `FieldNameSource::as_str` is the single definition, and serde
  serializes through it via `#[serde(into = "&'static str")]`, so the JSON and table spellings
  cannot diverge. Row order is asserted where it is produced, in `accepted_bug_fields`, not
  again at the writer.
- `src/commands/field_tests.rs` — the no-argument form against `wiremock`, asserting one row
  of each source; the named form unchanged; and `--fields sort_key` exits 7. `sort_key` rather
  than a nonsense token: it is a valid key of the *named* form and invalid here, so it fails
  if the handler validates against the wrong key set, which a nonsense token could not detect.
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
- **The live agreement oracle.** Read `status_whiteboard` *out of the listing* with `jq`,
  requiring `source == "server"`, then feed that name to `bug update --field <name>=…`
  and require it not to be rejected. The name is pinned rather than taken from an
  arbitrary `.[0]`: an arbitrary catalogue row could land on a read-only field Bugzilla
  refuses on its own, reddening the block for a reason that has nothing to do with bzr's
  validator. Reading it out of the listing is still what makes the block bite — an absent
  row yields an empty name and the guard's `else` calls `test_fail` rather than passing
  vacuously, and a listed name the validator rejects exits 7. What it proves against a
  live server is that bzr does not refuse a catalogue-only name, which is what the block
  is named for; it does not prove Bugzilla honours the key, and cannot (caveat 3 above).

Every fixture guard in the new blocks gets an `else` that calls `test_fail`. None of the
new assertions is skippable: the containers always declare a catalogue, so an absent
fixture is a real failure, not a reason to skip.

## Files

| path | change |
|------|--------|
| `src/cli/field.rs` | `name: Option<String>`; `List` and `Aliases` doc comments |
| `src/cli/mod.rs` | the `Field` group doc comment (`bzr field --help`) |
| `src/commands/field.rs` | dispatch on `name`; the no-argument branch |
| `src/types/field.rs` | `FieldName`, `FieldNameSource`, `FIELD_NAME_FIELDS` |
| `src/output/resources/field.rs` | `write_field_names` |
| `src/commands/runtime/shared/field_catalogue.rs` | `accepted_bug_fields`; `undeclared()` rewording |
| `src/commands/schema.rs` | `"field-name"` in `SCHEMAS` |
| `src/output/mod.rs` | `SCHEMA_VERSION` 3.0.2 → 3.0.3 |
| `schemas/field-name.json` | new |
| `docs/bzr-cli.md` | command tree, `field list` section, projection table, schema list, the `field` group heading and its TOC anchor (the section is no longer value-lookup only), **and the `--field` section's stale "wider than the set it can currently list" paragraph, which this change falsifies** |
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

One boundary is worth naming: the server controls the field names printed. The first
draft of this section concluded that reusing the existing table and JSON writers raised
"no new destination-encoding concern", on the grounds that `bzr server capabilities`
already renders catalogue-supplied custom field names the same way. `$detect-evil`
refuted that: the repository had already ruled the unescaped treatment defective for
comment tags (`8afa1c7a`), so a new table cell that skipped it was a new instance of a
settled defect, not parity. The control is therefore **present, not absent** —
`write_field_names` escapes the name cell through `escape_table_control`
(`src/output/formatting.rs`). `source` needs none: it is a closed set of three literals.
Residual, matching the repository's existing standard rather than widening it:
`char::is_control` is Unicode Cc only, so bidi overrides and format characters still pass
through; ADR 0062 records this and it is tracked as follow-up. JSON and NDJSON need no
control — serde escapes when it serializes.
