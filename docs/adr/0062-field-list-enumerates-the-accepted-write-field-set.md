# ADR 0062: `field list` enumerates the accepted write-field set

## Status

Accepted

## Context

ADR 0053 gave `bzr bug create` / `bzr bug update` a `--field KEY=VALUE` /
`--field-json` passthrough and bounded it with a validator: a key is accepted when the
connected server's `field/bug` catalogue declares it **or** when it is a REST bug field
bzr itself models, taken from `BUG_FIELDS` (`src/types/bug/fields.rs`). That union is
deliberate, and ADR 0053 explains why — the catalogue reports Bugzilla's internal column
names for many built-ins (`status_whiteboard`, `short_desc`, `rep_platform`,
`bug_file_loc`, `blocked`) while the write API takes the REST names (`whiteboard`,
`summary`, `platform`, `url`, `blocks`), so a catalogue-only check would reject
`--field whiteboard=…`.

Neither half of that union is enumerable from the CLI. `bzr field list <name>` requires
a field name and lists that one field's legal *values*. `bzr field aliases` prints bzr's
six-entry static alias table. `bzr server capabilities` prints only fields where
`is_custom` is true. So the accepted set is strictly wider than anything a user can list,
and the only way to find a valid key is to guess and read the rejection —
issue #718.

The rejection message itself records the gap. `undeclared()`
(`src/commands/runtime/shared/field_catalogue.rs`) points at `bzr server capabilities`
and carries a comment explaining that it deliberately stops at "custom fields" because
promising more would be the same overclaim in a new place.

## Decision

Give `bzr field list` a no-argument form that enumerates **the whole accepted set**, one
row per name, each row marked with why that name is accepted.

**One listing, both name sets, source marked.** The rows are the union of the server's
declared catalogue names and `BUG_FIELDS`'s canonical REST names, with a `source` of
`server`, `bzr`, or `both`. Listing either half alone would repeat the asymmetry the
issue exists to close: catalogue-only omits `whiteboard`, `summary`, `platform`, `url`,
and `blocks`, all of which `--field` accepts; `BUG_FIELDS`-only omits every `cf_*` field
and every internal column name.

**The marked relationship is provenance, not synonymy.** A row says *why* its name is
accepted. It does not pair `status_whiteboard` with `whiteboard`, because bzr models no
such pairing: `FIELD_ALIASES` covers six `bug_*` names and none of the five the issue
names, and Bugzilla's `field/bug` response carries no REST-name field to derive one from.
Building that mapping means a second hand-written table — precisely the drift ADR 0053
rejected when it chose `BUG_FIELDS` over a new alias table. The pairing is documented in
prose in `docs/bzr-cli.md` and left out of the data.

**Agreement is structural on both halves.** The listing and the validator live in one
module — `src/commands/runtime/shared/field_catalogue.rs`, which already owns "what
`--field` accepts" — and neither half of the union is bound by convention. `BUG_FIELDS` is
shared because both sides read that one constant: the validator through
`is_bzr_known_bug_field`, the listing by iterating it directly. (They are not the *same
call*; a predicate that answers "is this key in `BUG_FIELDS`" is the wrong shape for
building the list, so the binding here is the constant, not a function.) The catalogue is
shared because
`accepted_bug_fields()` fetches it *itself*, through the same
`BugzillaClient::bug_field_names()` the validator probes with. Taking the catalogue as a
`&[String]` parameter was the first shape and is rejected: it type-checks against any
`Vec<String>`, so a later edit to the one call site could publish names the validator
rejects with nothing failing to compile, and the ADR would be telling the next reader that
this is impossible. `validate_bug_fields()` keeps its fast paths unchanged.

**The listing always probes; it never reads the cache.** `ServerConfig.bug_field_names`
is a validator fast path whose staleness is harmless there — a cache miss re-probes, so a
stale entry can only cost a request, never accept a field the server does not declare.
A listing has no such safety: a stale entry would print names the current server has
dropped, which is the disagreement criterion 2 forbids. The listing also does not
*write* the cache; that would be a performance-only coupling to a private helper.

**No `--custom` / `--all` filters, and `bzr server capabilities` is unchanged.** Bugzilla
requires custom fields to carry the `cf_` prefix — bzr already relies on this in
`is_custom_bug_field` — so custom fields are self-identifying in the listing and a filter
flag buys a `grep`. The two commands answer different questions: `server capabilities`
reports custom fields *with their mapped type and legal values*; `field list` reports
*names a write will accept*. Making one defer to the other would drop fields from a
published schema for no user gain.

**The rejection message now names this command.** `undeclared()` points at
`bzr field list`, which is finally the command that answers "what can I set here".

**Schema.** A new published output shape `field-name` (`schemas/field-name.json`).
Adding a result type is additive under ADR 0007, so `SCHEMA_VERSION` goes 3.0.2 → 3.0.3.

## Consequences

- **A `SCHEMA_VERSION` bump is not a one-line change, and ADR 0007's "additive = patch"
  does not make it cheap.** Eleven files outside `src/` pin the literal, and each is an
  exact-match consumer: four functional phase scripts, `README.md`, `docs/bzr-cli.md`, two
  `bzr-reference` files, and — the load-bearing one —
  `content/skills/bzr-dependency-analysis/scripts/collect.py`, whose
  `BZR_SCHEMA_VERSION` is compared with `!=` at `collect.py:421` and raises
  `FatalCollection` on any difference. So the bundled dependency-analysis skill treats
  *every* bump as breaking, patch included, which is at odds with what a patch bump is
  supposed to mean for a consumer. All eleven are updated here because they are direct
  consequences of this change; the exact-match policy in `collect.py` is not, and is
  recorded as follow-up rather than fixed under this charter, together with the fact that
  nothing mechanically ties those eleven literals to `output::SCHEMA_VERSION` — the
  coupling is found by grep and held by review. The 16 functional failures it produced on
  the first full run all had this single cause.
- **An *unquoted* shell variable that expands to nothing changes behaviour rather than
  failing.** `bzr field list $FIELD` with `FIELD` unset drops the word entirely: that was
  a clap usage error (exit 2) and is now the listing, exiting 0 with a different JSON
  shape, so a script relying on the old failure to catch an unset variable stops catching
  it. The *quoted* form `bzr field list "$FIELD"` is unaffected — it passes an empty
  string, which both before and after exits 7 with `field name must not be empty, '.', or
  '..'` (verified against the built binary). This is the unavoidable cost of the
  no-argument form the issue asks for, and it is the only behaviour change to an existing
  invocation.
- **Server-supplied names are escaped into the table, to the same standard as comment
  tags and no further.** `write_field_names` runs the name cell through
  `escape_table_control` (promoted here out of `src/output/resources/comment.rs`, where
  `8afa1c7a` introduced it), so a hostile server cannot put a raw ESC on the user's
  terminal. `source` is not escaped: it comes from `FieldNameSource::as_str`, a closed set
  of three literals. The residual is that `char::is_control` covers Unicode category Cc
  only, so bidirectional overrides and format characters (U+202E, U+2066..U+2069,
  U+200B/E/F) still reach the cell. That gap is the repository's existing standard, not
  one this change introduces, and closing it belongs to every table writer at once rather
  than to this one; it is recorded as follow-up. The JSON and NDJSON paths need nothing —
  serde escapes control characters when it serializes.
- One subcommand emits two shapes, selected by whether the positional is present:
  `FieldValue[]` with a name, `FieldName[]` without. `bzr schema` already does exactly
  this, so the pattern is not new to the CLI, but an agent that hard-codes `field list`'s
  JSON shape must now branch on the argument.
- Making the positional optional changes clap's behaviour for `bzr field list` alone:
  what was a usage error (exit 2) is now a network command. Nothing else about
  `bzr field list <name>` changes.
- The no-argument form costs one `field/bug?include_fields=name` request every time, by
  design. On a large installation that is a few hundred names; the request is already
  bounded by `include_fields=name`.
- The listing is never empty even against a server that declares nothing, because
  `BUG_FIELDS` contributes 28 rows unconditionally. There is therefore no empty-listing
  message to specify, and a `source: server` row missing from the output is a real
  signal rather than an ambiguous one.
- Read-only names appear on **both** halves of the union, not just the server's. A field
  the server declares but Bugzilla's write API rejects (`bug_id`, timestamps) is listed,
  and so is every read-only name in `BUG_FIELDS` — which is bzr's `--fields`
  *read-projection* allow-list (`src/types/bug/fields.rs:175`), so `source: bzr` rows
  include `id`, `creator`, `creation_time`, and `last_change_time`, none of which a write
  can set. ADR 0053 already settled that these are *accepted*; this change is what makes
  them *published*, which its own defence ("the flag's own help and this document point
  at the REST names") did not anticipate. The listing's claim is therefore precisely "bzr
  will not refuse this key", never "the server will honour it", and `docs/bzr-cli.md`
  says so.
- **The one accepted-but-unlisted case, and it runs the other way from the obvious one.**
  `validate_bug_fields` short-circuits on a `ServerConfig.bug_field_names` cache hit
  without probing, so a field the server *removes* after its names were cached is still
  accepted on a write while this listing, which always probes, no longer shows it. ADR
  0053 records that residual explicitly and accepts it as the price of the cache. The
  opposite direction is safe: a field *added* since the last listing is a cache miss,
  which always re-probes, so it is accepted rather than wrongly rejected. Closing the
  removal case would mean probing on every write, which is the round trip ADR 0053's
  cache exists to avoid; it is documented instead.
- `tests/functional/phases/08g-bug-arbitrary-fields.sh` carries an assertion added by
  #283 that runs the command the rejection names and requires it to work. It now runs
  `bzr field list`, and its comment explaining why `field list` *could not* be that
  command is removed rather than left to read as false.

## Considered & rejected

- **List only the server's catalogue names.** verified: `validate_bug_fields`
  (`src/commands/runtime/shared/field_catalogue.rs`) returns `Ok(())` for any key
  satisfying `is_bzr_known_bug_field` before it ever probes, so all 28 `BUG_FIELDS` names —
  including the five REST spellings ADR 0053 names — would be accepted while absent from the
  listing. This is not automatically a criterion-2 breach: the criterion permits
  accepted-but-unlisted names *if the relationship is documented*, and a prose paragraph
  would satisfy it as worded. judgment: it fails the issue's own delegated open question 1,
  which states that showing one name set is "a repeat of the asymmetry this issue is about",
  and it would leave the user's practical answer to "what can I name here" a paragraph rather
  than a listing.
- **List only `BUG_FIELDS`.** verified: the same function rejects nothing the catalogue
  declares, and the containers under `tests/functional/` declare internal column names
  that appear in no `BUG_FIELDS` entry — `tests/functional/phases/05-fields-classifications.sh`
  passes `field list short_desc` and `field list bug_status` against them, while
  `BUG_FIELDS` is a fixed 28-element list at `src/types/bug/fields.rs:176` containing
  neither. It would also need no server at all, making it a different feature from the
  one asked for.
- **Pair internal and REST names in the output (`status_whiteboard` → `whiteboard`).**
  verified: bzr has no source for the pairing — `FIELD_ALIASES`
  (`src/types/field.rs:13`) holds six entries, none of them the five, and
  `FieldDefinition` (`src/client/resources/field.rs:26`) deserializes only `name`,
  `type`, `is_custom`, and `values`, so nothing bzr parses today carries a REST name.
  That is a fact about bzr's deserializer, not a proof that Bugzilla publishes nothing:
  `field/bug` returns members bzr discards, and this record does not claim to have
  audited them. judgment:
  the remaining option is a hand-written table, which is the drift ADR 0053's
  "Accepted name set" section rejected by choosing `BUG_FIELDS`.
- **A new `bzr field names` subcommand instead of a no-argument form.** judgment: the
  issue asks for a no-argument form of an existing verb, and a third verb under `field`
  makes the discovery surface it is trying to simplify one item longer.
- **`--custom` / `--all` filters.** judgment: `cf_` is a Bugzilla-mandated prefix, so
  `bzr field list --json | jq '.[] | select(.name | startswith("cf_"))'` is already the
  filter, and `--fields` / `--exclude-fields` projection is already wired.
- **Make `server capabilities` defer to `field list` for custom fields.** verified:
  `custom_fields` is a required member of the published `server-capabilities` schema
  (`schemas/server-capabilities.json`), so dropping it is a breaking change requiring a
  major `SCHEMA_VERSION` bump under ADR 0007. judgment: it also loses the mapped type and
  legal values, which the name listing does not carry.
- **Do nothing.** verified: the gap is recorded in the source itself — the comment on
  `undeclared()` explains that the message stops at "custom fields" because the accepted
  set is wider than anything bzr can list, and names issue #718 as the tracker for it.
- **Have the listing read `ServerConfig.bug_field_names`.** judgment: it would make the
  listing disagree with a fresh probe after the server's catalogue changed, which is the
  one property this feature must not have.
- **Omit, or separately mark, the names no write can set** (`id`, `creator`,
  `creation_time`, `last_change_time` from `BUG_FIELDS`; `bug_id` and timestamps from the
  catalogue). verified: bzr has no writability signal to split on — `FieldDefinition`
  (`src/client/resources/field.rs:26`) carries `name`, `type`, `is_custom`, and `values`
  and nothing about writability, and ADR 0053 accepts that the server "silently ignores
  request keys it does not recognise" rather than predicting them. Whether Bugzilla
  exposes such a signal in a member bzr discards was not audited. judgment: any
  split would therefore be a third hand-written table with the same drift this record
  rejects twice above, and criterion 2 permits omission only if the omitted set is
  documented — a wider caveat than the one being kept. The consequence is recorded
  instead, and the listing's claim is stated as "bzr will not refuse this key".
