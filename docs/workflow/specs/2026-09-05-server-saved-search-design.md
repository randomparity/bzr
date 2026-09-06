# Server-side saved search design

- Issue: #670
- Scope token: `q670-1c1b8eb2`
- Decision: no ADR (see "Why no ADR" below)
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
one new functional phase script, and the runner list in `tests/functional/run-tests.sh` that
the new phase must appear in. That last edit is a necessary consequence of the sourced
criterion requiring a phase script, not an expansion of it: `tools/check-functional-test-ids.sh`
compares the runner's `for _phase in` list against the basenames in
`tests/functional/phases/` and fails `make lint` unless they match. No dependency, config,
auth, schema, or paging behaviour changes.

## Verified: upstream Bugzilla ignores these parameters

Three checks, run before this design, all against the project's own functional images
(`bz50` = 5.0.6, `bz52`, `bz53` = 5.3.3+). Together they establish one thing: **upstream
Bugzilla accepts `savedsearch` and `sharer_id` on `Bug.search` and does nothing with them,
on both transports.**

1. **Source.** Neither identifier appears anywhere under `Bugzilla/` in any of the three
   images. `Bugzilla::WebService::Bug::search` passes its parameter hash straight into
   `Bugzilla::Search->new(params => ...)`; `sharer` exists there only as a *top-level
   constructor option*, which `Bug.search` never sets.
2. **Live REST probe**, authenticated as `admin@test.bzr` via that account's functional-test
   API key — the same account the probe's named query was seeded for, so an ownership
   mismatch cannot explain the result. With that query seeded to match nothing
   (`bug_id=999999`), `GET /rest/bug?savedsearch=<name>&include_fields=id` against bz50
   returned `{"bugs":[{"id":1}]}` — byte-identical to the same request with no criterion at
   all. A name matching no stored query returned the same.
3. **Live XML-RPC probe**, same account and same seeded query: `Bug.search` with a
   `savedsearch` member returned that same unfiltered result.

## Inferred, not verified: Red Hat Bugzilla honours them

Red Hat documents `sharer_id` explicitly as a Red Hat Extension, with the sharer's **numeric
user id** in its example payload (`{"savedsearch": "MySavedSearch", "sharer_id": 112233}`).
That establishes the parameter names and the identifier form. It does **not** establish how
Red Hat's fork implements them, and no check above observed a Red Hat server or read Red
Hat's source.

So the following is an inference from vendor documentation plus upstream's dispatch shape,
and it is untested: because `Bug.search` is one server-side sub behind both the REST and the
XML-RPC entry points upstream, a fork that implements saved-search resolution inside it gets
identical behaviour on both transports. The inference is reasonable and it is the basis for
one design choice below, but it is a projection of upstream's architecture onto the very
area the fork patched, which is where that projection is weakest.

**What follows regardless of the inference.** The transport asymmetry the issue's triage
feared — REST ignoring the parameters while XML-RPC honours them — is ruled out for upstream
by checks 2 and 3 directly. A plain REST pass-through is therefore the faithful
implementation, and the comparison harness's `observe_bzr_transport` REST assertion is
satisfiable without contriving anything.

**What follows from the verified part alone.** On a stock Bugzilla the parameters are
silently ignored, so the search degrades to an unfiltered one. That is precisely what
python-bugzilla does today, which is why "parity" is an accurate claim at the level
`docs/dev/python-bugzilla-parity.md` measures. It is not something a user should have to
discover, so it is stated in the flag's own help text, in the CLI reference, and in a
footnote on the parity table rather than left implicit.

## Parameter model

`SearchParams` gains two fields:

```rust
pub saved_search: Option<String>,
pub sharer_id: Option<u64>,
```

`saved_search` joins `has_filters()` for consistency with every other filter field. That
predicate is currently a consistency invariant rather than a behavioural gate: it has no
production caller — the three non-test call sites (`src/commands/query/update.rs:144` and
`:169`, `src/commands/query/save.rs:63`) are all `SavedQuery::has_filters`, a separate
function at `src/types/query.rs:266`. What actually makes a saved-search-only invocation a
complete query is the presence check in `src/commands/bug/search.rs`, described under "CLI
contract" below.

`saved_search` deliberately does **not** join `has_structured_filters()`. That predicate
gates hybrid mode's XML-RPC retry of an empty REST result, and it exists for filters whose
REST and XML-RPC handlers can disagree. The exclusion rests on the untested Red Hat
inference above: if saved-search resolution is one server-side sub shared by both
transports, an empty REST result is authoritative and a retry would return the same rows
after a second round trip — the same reasoning the existing doc comment applies to
`quicksearch` and `summary`.

Stating the bet plainly, since it is a bet: this removes the one extension the change
targets from a safety net built for extensions. If Red Hat's REST and XML-RPC handlers do
disagree, a hybrid-mode saved search whose REST leg returns nothing is reported as empty
without the XML-RPC retry that would have caught it. The alternative — including it, and
paying a second round trip on every empty saved-search result — is rejected because it
protects against a divergence nobody has observed, at a cost paid on every miss. Adding it
later is a one-line change if a real server ever exhibits the divergence.

`sharer_id` is not a filter on its own; it only qualifies a saved-search name, and the CLI
requires the name to be present.

## Considered and rejected

**Accept a login for `--sharer` and resolve it to an id.** Rejected on three grounds. The
documented server parameter *is* a numeric user id, which Red Hat's UI surfaces in the
saved-search URL, so a login would have to be translated before it could be sent. Translating
it means a `User.get` round trip whose result is usable only on servers that also implement
the extension — it buys nothing where the feature works and wastes a request where it does
not. And python-bugzilla's `--savedsearch-sharer-id` takes the id, so accepting a login would
make bzr's flag silently mean something different from the tool this campaign measures
parity against.

So `--sharer` is typed `u64` and maps to `sharer_id` unchanged. Clap rejects a non-numeric
value at parse time with its ordinary value-validation error and exit code 2, so no server
round trip happens for a mistyped id.

**Detect extension support and warn (or refuse) when the server lacks it.** The facility
exists: `BugzillaClient::server_extensions()` (`src/client/resources/server.rs:39`) already
fetches `GET /rest/extensions` into `ServerExtensions`
(`src/types/server_info.rs:13`), which the server commands already surface. Rejected: it
costs an extra round trip on every `--saved-search` invocation to tell the user something the
flag's own `--help` and the CLI reference already tell them, and detection would have to be
wired into a search path that currently makes exactly one request. Disclosure is the cheaper
control for the same information.

That second rejection is a policy, not a local choice — see "Decision record" below.

## CLI contract

| Flag | Type | Constraint |
|---|---|---|
| `--saved-search <NAME>` | `Option<String>` | conflicts with the positional `<QUERY>` and with `--from-url` |
| `--sharer <ID>` | `Option<u64>` | requires `--saved-search` |

`--saved-search` conflicts with the positional quicksearch string for a verified reason, not
a stylistic one: `Bug.search` replaces its entire parameter hash when `quicksearch` is
present (`$match_params = $cgi->Vars` in `Bugzilla/WebService/Bug.pm`), so a saved-search
name sent alongside a quicksearch string is discarded by the server without a diagnostic.
Rejecting the combination at parse time is the only way the user learns.

`--saved-search` conflicts with `--from-url` because that flag is a complete alternate query
source with its own server resolution and `--save-as` persistence; there is no coherent
combination of the two.

`bug search` with none of the three sources keeps failing as input validation, with the
message widened to name all three.

Everything else composes unchanged: `--limit` (default 50), `--offset`, `--paginate`,
`--count`, `--fields`, `--exclude-fields`, `--sort`, `--order`.

## Wire mapping

REST (`src/client/resources/bug.rs`): `savedsearch` joins the `append_option_params` string
table; `sharer_id` is appended beside `limit` and `offset`, which are the existing numeric
entries. Encoding goes through `reqwest`'s typed `query()` exactly as every other search
parameter does.

XML-RPC (`src/xmlrpc/resources/bug.rs`): `savedsearch` joins the `option_fields` string
table; `sharer_id` becomes a `Value::Int` beside `limit` and `offset`.

Both mappers omit an absent parameter entirely rather than sending an empty value, matching
every other optional field on both paths.

## Testing

**Unit (wiremock and clap).** The wire contract is proven here, because it cannot be proven
against a real Bugzilla — see below. A REST test asserts `query_param("savedsearch", …)` and
`query_param("sharer_id", …)` on the outgoing request; an XML-RPC test asserts the
corresponding members in the call body. Clap tests cover both conflicts, the `requires`
relation, and the non-numeric `--sharer` rejection.

**Functional phase (`tests/functional/phases/08f-bug-saved-search.sh`).** A real container
can prove that a real Bugzilla *accepts* the request and that the CLI contract holds
end-to-end; it cannot prove filtering, because the servers under test ignore the parameter
by design. The phase therefore asserts: a credentialed `--saved-search` search succeeds and
returns a JSON array; the same search under `--api xmlrpc` succeeds; `--count` composes;
the credentialless path (`--server-url` with no credential) succeeds; and the four
validation rejections exit 2. Stating what it cannot assert is part of the test's comment
header, so a later reader does not mistake its silence for coverage.

The phase seeds **no fixture** and uses a literal saved-search name and sharer id. Seeding a
real `namedqueries` row would change no assertion — every supported image returns the same
unfiltered rows whether or not the name exists — while adding a failure mode that hides
itself: a container-exec failure would turn all five container assertions into skips, and
`test_skip` does not fail a run, so the phase could report green having proved nothing.
Unconditional tests are both smaller and more honest here.

A new phase file rather than an addition to `08-bugs.sh` keeps this change out of a file a
concurrently running sibling issue may also touch.

**Comparison (`tests/functional/compare/01-bug-lifecycle.sh`).** The saved-search block
drops `lifecycle_bzr_gap`'s diagnostic argument for plain `lifecycle_bzr`, and the
`lifecycle_expect_gap 670` line is deleted. `expect_gap` converts a pass into a failure once
the gap closes, so leaving it would turn the working feature red.

## Known limitation of the comparison assertion

Both sides of `compare/01-bug-lifecycle/saved-search` assert that the search returns exactly
the two lifecycle bug ids. On upstream Bugzilla that assertion passes because the parameter
is ignored *and* the container holds exactly those two bugs at that point — not because the
saved search was resolved. This is true of the python-bugzilla side today, before this
change, and flipping bzr's side to a plain `lifecycle_bzr` inherits the same property.

This is recorded rather than fixed. Strengthening the assertion — seeding a third bug the
saved search excludes — would make both clients fail against every supported image, since
neither server honours the parameter. The honest fix is a Red-Hat-shaped fixture, which is
its own change against `tests/functional/redhat-shape-proxy.py`. It is not folded in here.

Because the parity table is the durable, quotable artifact, the row's Status cell carries a
footnote marker rather than a bare `parity`, and the footnote says the evidence test cannot
distinguish a resolved saved search from an unfiltered one on any supported image. Applying
this spec's own disclosure rule to the table is the point: that row is the line most likely
to be read out of context.

This limitation has no owning tracker issue at design time. The repository keeps no
`docs/debt/` directory, so it is carried in the plan's Deferrals section and filed as a
follow-up issue from this run's completion report.

## Decision record

`docs/adr/README.md` sets this repository's criterion for an ADR: "choices with viable
alternatives where the rationale is worth preserving". Disclose-rather-than-detect meets it.
It is not local to this issue — `docs/dev/python-bugzilla-parity.md` lists four sibling gaps
in the same parity campaign (#671 generic arbitrary fields, #672 comment tags, #679
whiteboard match types, #680 personal bug tags), and each faces the same question: ship a
vendor-extension parameter that stock servers ignore and disclose it, or detect support
first. Whichever way this issue answers becomes the precedent, so recording it once here
saves four re-litigations.

An earlier draft of this spec argued no ADR was needed on the grounds that the change
touches no module boundary or contract shape. That is the wrong criterion — it is not the
one the README states — and the argument is withdrawn.

The ADR number is assigned by the campaign orchestrator, not chosen here, because sibling
issues are running concurrently and all would otherwise pick the same "next free" number.
The index row in `docs/adr/README.md` is the orchestrator's to add; the index is not coupled
to any gated check in this repository.

## Out of scope

- Local saved queries (`bzr query`) — unchanged.
- Match-type modifiers on the shared parameter builder — issue #679.
