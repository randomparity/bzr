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
and one new functional phase script. No dependency, config, auth, schema, or paging
behaviour changes.

## Verified server contract

`savedsearch` and `sharer_id` on `Bug.search` are a **Red Hat Bugzilla extension**, not part
of upstream Bugzilla's API. Four independent checks, run before this design:

1. **Source.** Neither identifier appears anywhere under `Bugzilla/` in the project's own
   functional images (`bz50` = 5.0.6, `bz52`, `bz53` = 5.3.3+). `Bugzilla::WebService::Bug::search`
   passes its parameter hash straight into `Bugzilla::Search->new(params => ...)`. `sharer`
   exists there only as a *top-level constructor option*, which `Bug.search` never sets.
2. **Live REST probe.** With a named query seeded to match nothing (`bug_id=999999`),
   `GET /rest/bug?savedsearch=<name>&include_fields=id` against the bz50 image returned
   `{"bugs":[{"id":1}]}` — byte-identical to the same request with no criterion at all. A
   name matching no stored query returned the same.
3. **Live XML-RPC probe.** `Bug.search` with a `savedsearch` member against the same image
   returned that same unfiltered result.
4. **Vendor documentation.** Red Hat documents `sharer_id` explicitly as a Red Hat Extension,
   and its example payload carries the sharer's **numeric user id**
   (`{"savedsearch": "MySavedSearch", "sharer_id": 112233}`).

Two consequences follow, and they set the shape of everything below.

**There is no transport asymmetry.** Neither REST nor XML-RPC honours the parameters on
upstream Bugzilla; both honour them on Red Hat Bugzilla, because `Bug.search` is one server
sub behind both. A plain REST pass-through is therefore the faithful implementation, and the
comparison harness's `observe_bzr_transport` REST assertion is satisfiable without
contriving anything.

**On a stock Bugzilla the parameters are silently ignored,** so the search degrades to an
unfiltered one. That is precisely what python-bugzilla does today, which is why "parity" is
an accurate claim at the level `docs/dev/python-bugzilla-parity.md` measures. It is not
something a user should have to discover, so it is stated in the flag's own help text and in
the CLI reference rather than left implicit.

## Parameter model

`SearchParams` gains two fields:

```rust
pub saved_search: Option<String>,
pub sharer_id: Option<u64>,
```

`saved_search` joins `has_filters()`: a saved-search name is a filter, and a `bug search`
invocation carrying only that name is a complete query.

`saved_search` deliberately does **not** join `has_structured_filters()`. That predicate
gates hybrid mode's XML-RPC retry of an empty REST result, and it exists for filters whose
REST and XML-RPC handlers can disagree. Saved-search resolution is one server-side sub
shared by both transports, so an empty REST result is authoritative and a retry would return
the same rows after a second round trip — the same reasoning the existing doc comment
applies to `quicksearch` and `summary`.

`sharer_id` is not a filter on its own; it only qualifies a saved-search name, and the CLI
requires the name to be present.

## `--sharer` takes a numeric user id

`--sharer` is typed `u64` and maps to `sharer_id` unchanged. The alternative — accepting a
login and resolving it — is rejected on three grounds:

- The documented server parameter *is* a numeric user id, which Red Hat's UI surfaces in the
  saved-search URL. A login would have to be translated before it could be sent.
- Translating it means a `User.get` round trip whose result is usable only on servers that
  also implement the extension, so the extra request buys nothing on the servers where the
  feature works and wastes one on the servers where it does not.
- python-bugzilla's `--savedsearch-sharer-id` takes the id. Accepting a login would make
  bzr's flag silently mean something different from the tool this campaign measures parity
  against.

Clap rejects a non-numeric value at parse time with its ordinary value-validation error and
exit code 2, so no server round trip happens for a mistyped id.

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
its own change against `tests/functional/redhat-shape-proxy.py`. It is reported as follow-up
work, not folded in here.

## Why no ADR

The decision changes no module boundary, no contract shape, and no cross-cutting invariant:
it adds two optional fields to an existing parameter struct and two entries to two existing
mapper tables. Its one non-obvious consequence — silent no-op on a stock server — is
answered where a reader meets it, in `--help` and in the CLI reference, and the alternatives
considered for `--sharer`'s type are recorded above. Triage reached the same conclusion
independently.

## Out of scope

- Local saved queries (`bzr query`) — unchanged.
- Match-type modifiers on the shared parameter builder — issue #679.
- `redhat-shape-proxy.py`'s `is_termless_bug_search`, whose criterion set does not count
  `savedsearch`, so it would classify a saved-search-only request as termless. Nothing
  routes such a request through that proxy today; reported as follow-up work.
