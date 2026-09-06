# Server-side saved search design

- Issue: #670
- Scope token: `q670-1c1b8eb2`
- Decision: recorded in "Decision record" below; no ADR file in this run, and why
- Branch: `feat/saved-search-670`
- Base branch: `main`

## Outcome and scope

`bzr bug search` gains `--saved-search <NAME>` and `--sharer <ID>`, which send Bugzilla's
`savedsearch` and `sharer_id` search parameters over whichever transport the invocation
resolves. They are a third, mutually exclusive query source alongside the positional
quicksearch string and `--from-url`, and they compose with the existing paging, projection,
sorting, and `--count` flags exactly as those sources do.

These are *server-side* saved searches — named queries stored in the Bugzilla account. They
are unrelated to bzr's local saved queries (`bzr query`, `src/cli/query.rs`), which stay
untouched.

The permitted surface is the shared search parameter type, its two transport mappers, the
`bug search` CLI and command modules, their test siblings, the CLI reference, the
python-bugzilla parity report, the saved-search block of the lifecycle comparison script,
one new functional phase script, and two files that are mechanically coupled to edits the
criteria require:

- `tests/functional/run-tests.sh` — `tools/check-functional-test-ids.sh` compares the
  runner's `for _phase in` list against the basenames in `tests/functional/phases/` and
  fails `make lint` unless they match, so a new phase file is not addable without its
  runner row.
- `tests/functional/pybz/container-tests.sh` — this file models the whole `#670` gap, not
  just its parity row. It holds every parity-report row as a literal string
  (`run_parity_report_fixture`), **and** it drives the real
  `tests/functional/compare/01-bug-lifecycle.sh` through a stub `run_bzr` that answers
  `--saved-search` with the unsupported-flag diagnostic, **and** it asserts the resulting
  PASS/FAIL/GAP counts and stale-gap behaviour. Closing the gap without updating all of
  that leaves `make functional-compare-all` permanently red. The full list of edits is Task
  3 of the plan.

Neither is an expansion of the charter's surface; each is the unavoidable other half of a
sourced criterion. Both are flagged to the campaign orchestrator. The second is a
line-adjacent conflict risk with the concurrently running sibling issue #672, which owns the
`Comment tags and minor update` row and its own stub arm in the same file. No dependency,
config, auth, schema, or paging behaviour changes.

## Verified: upstream Bugzilla ignores these parameters

Two checks, run before this design against the project's own functional images
(`bz50` = 5.0.6, `bz52`, `bz53` = 5.3.3+):

1. **Source, all three images.** Neither `savedsearch` nor `sharer_id` appears anywhere
   under `Bugzilla/`. `Bugzilla::WebService::Bug::search` passes its parameter hash straight
   into `Bugzilla::Search->new(params => ...)`; `sharer` exists there only as a *top-level
   constructor option*, which `Bug.search` never sets. No module under
   `Bugzilla/WebService/` references the `namedqueries` table at all. This establishes that
   none of the three can *resolve* a saved search.
2. **Live REST probe, all three images.** `GET /rest/bug?include_fields=id&savedsearch=…&sharer_id=1`
   returned byte-identical output to the same request with no criterion, on each of bz50,
   bz52 and bz53. This establishes the separate fact that each *accepts* the unknown
   parameters rather than faulting on them.

One further check covers bz50 only: an XML-RPC `Bug.search` carrying a `savedsearch` member,
authenticated as `admin@test.bzr` and naming a query seeded for that same account so an
ownership mismatch cannot explain the result, returned the same unfiltered rows. The seeded
query was `bug_id=999999`, matching nothing, so a server that resolved it would have returned
an empty set.

Claims are therefore scoped as: *cannot resolve* — all three images, from source; *accepts
without error over REST* — all three, probed live; *accepts without error over XML-RPC* —
bz50, probed live, and inferred for the other two from check 1.

## Inferred, not verified: Red Hat Bugzilla honours them

Red Hat documents `sharer_id` explicitly as a Red Hat Extension, with the sharer's **numeric
user id** in its example payload (`{"savedsearch": "MySavedSearch", "sharer_id": 112233}`).
That establishes the parameter names and the identifier form. It does **not** establish how
Red Hat's fork implements them, and no check above observed a Red Hat server or read Red
Hat's source. Nothing in this design depends on the inference.

**What follows regardless.** The transport asymmetry the issue's triage feared — REST
ignoring the parameters while XML-RPC honours them — is ruled out for upstream by the probes
directly. A plain REST pass-through is the faithful implementation, and the comparison
harness's `observe_bzr_transport` REST assertion is satisfiable without contriving anything.

**What follows from the verified part alone.** On a stock Bugzilla the parameters are
silently ignored, so the search degrades to an unfiltered one. That is precisely what
python-bugzilla does today, which is why "parity" is an accurate claim at the level
`docs/dev/python-bugzilla-parity.md` measures. It is not something a user should have to
discover, so it is stated in the flag's own help text, in the CLI reference, and in a
footnote on the parity table.

## Parameter model

`SearchParams` gains two fields:

```rust
pub saved_search: Option<String>,
pub sharer_id: Option<u64>,
```

`saved_search` joins **both** `has_filters()` and `has_structured_filters()`.

`has_filters()` is a consistency invariant rather than a behavioural gate: it has no
production caller — the three non-test call sites (`src/commands/query/update.rs:144` and
`:169`, `src/commands/query/save.rs:63`) are all `SavedQuery::has_filters`, a separate
function at `src/types/query.rs:266`. What makes a saved-search-only invocation a complete
query is the presence check in `src/commands/bug/search.rs`, under "CLI contract" below.

`has_structured_filters()` is behavioural: it gates hybrid mode's XML-RPC retry of an *empty*
REST result (`src/client/resources/bug.rs:266-292`), and its stated purpose is the cases
where a buggy REST extension can disagree with the XML-RPC implementation. `saved_search` is
the only vendor extension this change adds, so excluding it would remove the one new
extension from the net built for extensions. The cost of including it is close to nothing:
the retry fires only when the REST leg returned no rows, and on a stock server
`--saved-search` yields an unfiltered result, which is empty only when the caller can see no
bugs at all. On a server that does resolve the search, an empty result is exactly the case a
handler disagreement would hide. So the retry is paid on an already-empty query and bounded
by the existing `XMLRPC_FALLBACK_TIMEOUT`.

This is deliberately *not* the same reasoning that excludes `quicksearch` and `summary`.
Those are excluded because upstream evaluates them through one shared free-text parser — a
verified property. No comparable property is known for a fork's saved-search handling, so
the safe default applies.

`sharer_id` is not a filter on its own; it only qualifies a saved-search name, and the CLI
requires the name to be present.

## Considered and rejected

**Accept a login for `--sharer` and resolve it to an id.** verified: the documented server
parameter is a numeric user id, surfaced by Red Hat's UI in the saved-search URL, and
python-bugzilla's `--savedsearch-sharer-id` takes the id. judgment: translating a login means
a `User.get` round trip whose result is usable only on servers that also implement the
extension — nothing gained where the feature works, one wasted request where it does not —
and it would make bzr's flag silently mean something different from the tool this campaign
measures parity against.

So `--sharer` is typed `u64` and maps to `sharer_id` unchanged. Clap rejects a non-numeric
value at parse time with its ordinary value-validation error and exit code 2, so no server
round trip happens for a mistyped id.

**Detect extension support and warn.** verified: the facility exists —
`BugzillaClient::server_extensions()` (`src/client/resources/server.rs:39`) fetches
`GET /rest/extensions` into `ServerExtensions` (`src/types/server_info.rs:13`), which the
server commands already surface. judgment: it costs a round trip on every `--saved-search`
invocation to restate what the flag's own `--help` and the CLI reference already say, wired
into a search path that today makes exactly one request — and the warning arrives after the
user has typed the flag, later than the documentation reaches them.

**Refuse the request when the extension is absent.** judgment: bzr would reject a request the
server accepts, on the strength of an `/rest/extensions` listing that is not a guaranteed
inventory of patched `Bug.search` behaviour. ADR 0015 already settles that bzr does not mask
server responses; a client-side rejection is that same failure in the other direction.

**Resolve the saved search client-side and translate it into ordinary filters.** verified:
upstream Bugzilla exposes named queries only through `buglist.cgi?cmdtype=runnamed`; no
module under `Bugzilla/WebService/` in any supported image references `namedqueries`. The
translation would mean scraping a CGI page outside the API surface bzr is built on.

## CLI contract

| Flag | Type | Constraint |
|---|---|---|
| `--saved-search <NAME>` | `Option<String>` | conflicts with the positional `<QUERY>` and with `--from-url` |
| `--sharer <ID>` | `Option<u64>` | requires `--saved-search` |

`--saved-search` conflicts with the positional quicksearch string for a verified reason:
`Bug.search` replaces its entire parameter hash when `quicksearch` is present
(`$match_params = $cgi->Vars` in `Bugzilla/WebService/Bug.pm`), so a saved-search name sent
alongside a quicksearch string is discarded by the server without a diagnostic. Rejecting the
combination at parse time is the only way the user learns.

`--saved-search` conflicts with `--from-url` because that flag is a complete alternate query
source with its own server resolution and `--save-as` persistence; there is no coherent
combination of the two.

`bug search` with none of the three sources keeps failing as input validation, with the
message widened to name all three. Everything else composes unchanged: `--limit` (default
50), `--offset`, `--paginate`, `--count`, `--fields`, `--exclude-fields`, `--sort`,
`--order`.

## Wire mapping

REST (`src/client/resources/bug.rs`): `savedsearch` joins the `append_option_params` string
table; `sharer_id` is appended beside `limit` and `offset`, the existing numeric entries.
Encoding goes through `reqwest`'s typed `query()` exactly as every other search parameter.

XML-RPC (`src/xmlrpc/resources/bug.rs`): `savedsearch` joins the `option_fields` string
table; `sharer_id` becomes a `Value::Int` via the existing `xmlrpc_id` range check.

Both mappers omit an absent parameter entirely rather than sending an empty value, matching
every other optional field on both paths.

## Testing

**Unit (wiremock and clap).** The wire contract is proven here, because it cannot be proven
against a real Bugzilla. A REST test asserts `query_param("savedsearch", …)` and
`query_param("sharer_id", …)` on the outgoing request; an XML-RPC test asserts the
corresponding members in the call body. Clap tests cover both conflicts, the `requires`
relation, and the non-numeric `--sharer` rejection.

**Functional phase (`tests/functional/phases/08f-bug-saved-search.sh`).** A real container
proves that Bugzilla accepts the request and that the CLI contract holds end-to-end; it
cannot prove filtering, because the servers under test ignore the parameter. The phase
asserts acceptance over REST and XML-RPC, composition with `--count`, the credentialless
path, and the four parse-time rejections.

The phase seeds no fixture and uses a literal saved-search name and sharer id. Seeding a real
`namedqueries` row would change no assertion — every supported image returns the same
unfiltered rows whether or not the name exists — while adding a failure mode that hides
itself, since `test_skip` does not fail a run and a container-exec failure would turn every
container assertion into a skip on a green run.

A new phase file rather than an addition to `08-bugs.sh` keeps this change out of a file a
concurrently running sibling issue may also touch.

**Comparison.** `tests/functional/compare/01-bug-lifecycle.sh` drops the expected-gap
marking, and `tests/functional/pybz/container-tests.sh` — which models that gap in five
places — is updated in the same commit. `bash tests/functional/pybz/container-tests.sh` is
the observation that catches a mismatch between the two; it needs no container. Note that
`make functional-compare` does **not** reach that file: only `make functional-compare-all`
(`Makefile:226`) invokes it, and no CI workflow does.

## Known limitation of the comparison assertion

Both sides of `compare/01-bug-lifecycle/saved-search` assert the search returns exactly the
two lifecycle bug ids. On upstream Bugzilla that passes because the parameter is ignored
*and* the container holds exactly those two bugs at that point — not because a saved search
was resolved. This is already true of the python-bugzilla side before this change; flipping
bzr's side inherits it. It is recorded, not fixed: strengthening the assertion would make
both clients fail against every supported image. See the plan's Deferrals section for the
owning follow-up.

Because the parity table is the durable, quotable artifact, that row's Status cell carries a
footnote saying what its evidence can and cannot establish.

## Decision record

**bzr sends vendor-extension search parameters unconditionally and discloses the silent no-op
in documentation, rather than detecting server support.** Disclosure means three placements,
each reaching the reader before or at the point of use: the flag's clap doc comment (so
`--help` and the man page carry it), the flag's row and a note in `docs/bzr-cli.md`, and a
footnote on the parity claim. The alternatives are recorded under "Considered and rejected"
above.

Consequences: `--saved-search` against a stock Bugzilla returns an unfiltered result and
exits 0, which is bzr faithfully reproducing the server; bzr issues no extra request per
invocation; and bzr cannot warn a user who has not read the documentation, which is accepted
because the alternative charges every user a round trip to catch the subset who did not.

**Scope of this record: `--saved-search` only.** An earlier draft claimed it as precedent
governing the four sibling parity gaps (#671, #672, #679, #680). That claim is withdrawn — a
policy recorded in `docs/workflow/specs/` is not linked from `docs/adr/README.md`, is not
surfaced to a later reviewer consulting the ADR set, and is invisible to the sibling issue
running concurrently, so it had no mechanism to bind anything.

The cross-cutting version does belong in `docs/adr/`, by that directory's own criterion
("choices with viable alternatives where the rationale is worth preserving"). It is not
written here because ADR numbers are assigned by the campaign orchestrator — concurrent
sibling issues would otherwise all take the same "next free" number — and no number was
assigned in time. This run's completion report carries the recommendation, so the decision to
make it binding stays with the party that can number it.

## Out of scope

- Local saved queries (`bzr query`) — unchanged.
- Match-type modifiers on the shared parameter builder — issue #679.
