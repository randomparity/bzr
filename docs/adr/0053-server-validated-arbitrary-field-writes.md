# ADR 0053: Server-validated arbitrary field writes

## Status

Accepted

## Context

`bzr bug create` and `bzr bug update` expose a closed write surface. `CreateBugParams`
and `UpdateBugParams` (`src/types/bug/payload.rs`) are `#[non_exhaustive]` structs with no
`#[serde(flatten)]` map, and both structured-input shapes — `JsonCreateBug`
(`src/commands/bug/create_json.rs`) and `BugUpdateDraft`
(`src/commands/bug/update/draft.rs`) — carry `#[serde(deny_unknown_fields)]`. A user cannot
set a custom (`cf_*`) field, or any Bugzilla field bzr does not model, on a write.

Two issues meet here. Issue #283 asked whether the write path should gain `cf_*` support and
was held at `status:blocked` on "Waiting for legitimate use case". Issue #671 supplies that
use case: python-bugzilla's CLI accepts repeatable `--field NAME=VALUE` and `--field-json`
on create and modify, and the comparison harness at
`tests/functional/compare/01-bug-lifecycle.sh` already scripts the gap. The scripted case
drives `whiteboard` — a core built-in field, not a `cf_*` one — so a `cf_*`-only design does
not close it.

python-bugzilla's library already knows how to answer "does this field exist": `getbugfields()`
(`base.py:730`) calls `Bug.fields` with `include_fields:["name"]` and caches the result. Its
CLI never consults it — `_merge_field_opts` (`_cli.py:448`) does a raw `query.update` — which
is why `bugzilla modify --field bogus=1` returns success having changed nothing. Bugzilla
silently ignores request keys it does not recognise.

bzr already has the catalogue: `all_bug_fields()` (`src/client/resources/field.rs:70`) fetches
`GET /rest/field/bug`, and `custom_field_summaries()` (`src/client/resources/server.rs:78`)
already consumes it.

## Decision

Add repeatable `--field KEY=VALUE` and `--field-json <PATH|->` to `bzr bug create` and
`bzr bug update`. Any key is accepted syntactically; every key is checked against the
server's own bug-field catalogue before the mutation is dispatched.

**Reframed acceptance criterion.** Issue #283's fourth acceptance criterion read "Keep
arbitrary non-`cf_*` extension fields out of the public mutation surface." It is reframed,
not deleted, as: **no field reaches the wire that the server has not declared.** The criterion
was always about arbitrariness, not about the `cf_` prefix — a prefix test admits a typo'd
`cf_relase` and rejects a legitimate `whiteboard`, so it enforces neither half of what the
criterion was protecting. Server declaration is the property that actually bounds the surface.

**Value typing.** `--field KEY=VALUE` sets a JSON string; everything after the first `=` is
the value, and `--field key=` sets the empty string, which is how Bugzilla clears a field.
`--field-json` reads a JSON object whose values may be of any JSON type, covering
multi-select, boolean, numeric, and date-typed custom fields. A key supplied by both, or by
`--field` twice, is rejected (exit 7) rather than silently resolved.

**Collision with typed flags.** The merged extra-field map is checked against the typed
payload *as actually serialized* — the typed params are built first, rendered with
`serde_json::to_value`, and any extra key already present in that object is rejected (exit 7)
naming the dedicated flag. This is drift-proof: it needs no hand-maintained reserved-key list
and it tracks `skip_serializing_if`, so `--field whiteboard=x` is allowed when `--whiteboard`
was not given (the case the comparison harness drives) and rejected when it was. It also
catches create's always-emitted `product`/`component`/`summary`/`version`.

**The accepted name set.** Bugzilla's field catalogue reports *internal column* names for
many built-ins — a live 5.3.3 probe returns `status_whiteboard`, `short_desc`, `rep_platform`,
`bug_file_loc`, `blocked` — while `Bug.create` and `Bug.update` take the REST names
(`whiteboard`, `summary`, `platform`, `url`, `blocks`). Custom fields have the same name in
both. A key is therefore accepted when the server declares it **or** when it is a REST bug
field bzr itself models, taken from `BUG_FIELDS` (`src/types/bug/fields.rs`), the list already
maintained for `--fields`. Reusing it needs no second alias table to drift, and a name bzr
models is one bzr knows the server's REST layer speaks.

**Validation and fail-closed probe.** Keys bzr already models are accepted with no network
call at all. For the rest, bzr resolves the catalogue before dispatch:

1. Consult `ServerConfig.bug_field_names`, the persisted per-server detection state.
2. If every remaining key is present, accept with no network call.
3. Otherwise probe `GET /rest/field/bug?include_fields=name`, persist the result under the
   config lock, and re-check. A key still absent fails with `InputValidation` (exit 7) naming
   the field and pointing at `bzr server capabilities`.
4. A failed probe **refuses the write** with a message that names the catalogue probe as the
   thing that failed, never as an absent field.

Step 3 is what makes the cache safe: a cache miss always forces a fresh probe, so a stale
cache can never reject a field the server has since declared. The cache is an optimisation
that cannot change an answer — and, for the same reason, cannot break one: a config bzr
cannot write is logged and stepped over rather than failing the user's write, and a
catalogue above a 4096-name ceiling is used for the request and not written at all, so a
server cannot bloat a file every later invocation parses.

The probe failure preserves the underlying error's class — and therefore its exit code —
while appending the probe context to the message, using the `annotate_search_fallback`
pattern already established at `src/client/resources/bug.rs:75`. So an undeclared field is
always exit 7 with "unknown field", and a probe failure is exit 4/5/8/9 with "the server's
bug field catalogue could not be retrieved … no changes were sent". Probe failure is not an
absent capability; both refuse, and the two are never confusable.

**`--from-json` stays strict.** `schemas/bug-create-input.json` and
`schemas/bug-update-input.json` are unchanged and `SCHEMA_VERSION` is not bumped. Issue #671
asks for flags only, and `docs/workflow/specs/2026-09-01-bug-mutation-surface-design.md:24`
is precedent that a flags-only change needs no schema bump. CLI `--field` still overlays onto
a `--from-json` invocation; the field carrying it is `#[serde(skip)]`, so `deny_unknown_fields`
continues to reject an `extra_fields` key in the document itself.

**Dry run stays offline.** `--dry-run` performs no write and makes no connection, so it
performs no catalogue validation. The previewed payload shows the extra fields as they would
be sent.

## Consequences

`bzr bug create --field cf_release=9.6` and `bzr bug update N --field whiteboard=text` work
against any Bugzilla that declares those fields, closing #671 with no comparison-harness
change: `whiteboard` is declared, so the scripted assertions pass as written once the flag
exists. `lifecycle_expect_gap 671` must be removed in the same change — `expect_gap`
(`tests/functional/lib.sh:209`) converts PASS to FAIL with "expected gap issue #N appears
resolved", so leaving the marker breaks the run.

A typo'd key fails locally at exit 7 before any request is sent, which is the behaviour
python-bugzilla's CLI does not have. The cost is one `GET /rest/field/bug` on the first
`--field` invocation against a server, amortised by the persisted name list;
`include_fields=name` keeps that response small on installations with hundreds of fields.

The check bounds arbitrariness; it is not a writability oracle, and Bugzilla exposes no
endpoint that is one. A declared internal name whose REST write form differs
(`--field status_whiteboard=x`) passes and is then ignored by `Bug.update`, and a read-only
field bzr models (`--field id=5`) does the same. Both require deliberately reaching past the
documented name — the flag's own help and this document point at the REST names — and both
are a far smaller surface than accepting every string.

The set bzr accepts is also wider than the set bzr can list. `bzr server capabilities` shows
only `is_custom` fields, and `bzr field list` takes a required field name and enumerates that
one field's legal *values*, so neither enumerates the accepted names: the non-custom catalogue
names and the `BUG_FIELDS` entries appear in no listing. The rejection message therefore points
at the custom fields `server capabilities` does show rather than claiming to show everything
accepted, and that output must not be read as the validator's allow-list. Issue #718 tracks the
missing enumeration command.

A field the server *removes* after the names were cached is still accepted on a cache hit and
then silently ignored by Bugzilla — the residual case the cache cannot close, since a hit by
definition skips the probe. Field removal is rare and destructive on the server side, and
closing it would mean probing on every write, which is the round trip the cache exists to
avoid.

`bug clone` and saved bug templates do not gain `--field`; that work is issue #712.
Create and update are REST-only (`src/xmlrpc/resources/bug.rs` implements only `search_bugs`
and `get_bug`), so there is no XML-RPC arm to mirror.

## Considered & rejected

- **A — validate the `cf_` prefix only.** judgment: the smallest and safest-looking option,
  and it needs no network call, but it does not close #671 — the comparison test drives
  `whiteboard`, a core built-in, which a prefix test rejects. It also still passes a typo'd
  `cf_relase` straight through to the silent no-op it was meant to prevent, so it buys
  restriction without buying correctness.
- **B — unrestricted passthrough, #671 as written.** judgment: exact parity with
  python-bugzilla's CLI and the least code, but it reproduces the defect that CLI has:
  `bug update --field bogus=1` returns HTTP 200 and exit 0 having changed nothing. It also
  fails #283's criterion under either reading. B is not a stepping stone to C — tightening a
  shipped passthrough into a validated one breaks every caller relying on the loose behaviour,
  so choosing B forecloses C rather than deferring it.
- **D — `cf_*` by default plus a documented unvalidated escape hatch.** judgment: two
  surfaces where one will do, and hatch users inherit B's silent no-op in full. The
  documentation would have to say that the escape hatch can fail silently, which is an
  argument against shipping it.
- **Fail open when the catalogue probe fails.** judgment: this reduces the design to B plus a
  wasted round trip on exactly the occasions when validation would have mattered, and it makes
  "the server does not declare this field" and "bzr could not ask" produce different outcomes
  for the same input on different days.
- **Validate inside `BugzillaClient::create_bug`/`update_bug`.** judgment: two unbypassable
  choke points instead of four call sites, but the client layer deliberately does no config
  I/O — `detect_server_settings` documents that the caller owns caching and persistence — and
  the cache lives in config. Putting the check in the command runtime keeps that separation.
- **A process-lifetime cache only, as python-bugzilla does.** verified: bzr is one process per
  invocation, so a process cache never survives to a second `--field` call and saves nothing.
- **Validate against the catalogue names alone.** verified against a live Bugzilla 5.3.3
  container: `field/bug` declares `status_whiteboard`, not `whiteboard`, so this rejected
  `--field whiteboard=...` and left #671 open. Caught by the functional suite, not by any
  wiremock fixture — a fixture only proves that a shape parses, never that it is the shape a
  server sends.
- **Extend `FIELD_ALIASES` (`src/types/field.rs`) with the missing REST-to-internal pairs.**
  judgment: it is the same information in a hand-written table that would have to be kept in
  step with Bugzilla, and it silently changes `bzr field list <name>` resolution for commands
  outside this change's scope. `BUG_FIELDS` already carries the REST names and is already
  maintained.
- **Add a `BzrError` variant for the probe failure.** judgment: one call site does not justify
  a nineteenth variant with its own exit code, `error_type`, and structured-detail arm; the
  established class-preserving re-wrap gives a distinct message while keeping the exit code
  honest about what actually went wrong.
- **Resolve a typed-flag/`--field` collision by precedence.** judgment: python-bugzilla lets
  `--field` win via `query.update`, but a silent winner between two things the user explicitly
  wrote is the same failure class as the silent no-op this ADR exists to remove.
- **Loosen `--from-json` with a named extra-fields object.** judgment: #671 asks for flags,
  the strict document shape is a contract consumers depend on, and an additive schema property
  would pull in a `SCHEMA_VERSION` patch bump and drift-test updates for surface nobody
  requested. It stays available as a later additive change.
