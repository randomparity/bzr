# Server-side saved search — implementation plan

Goal: `bzr bug search --saved-search <NAME> [--sharer <ID>]` sends Bugzilla's `savedsearch`
and `sharer_id` search parameters over REST and XML-RPC, composing with the existing paging,
projection, sorting and `--count` flags.

Architecture: two optional fields join the shared `SearchParams` struct
(`src/types/bug/search.rs`); the two existing transport mappers
(`src/client/resources/bug.rs`, `src/xmlrpc/resources/bug.rs`) each gain one string and one
numeric entry in tables they already iterate; `src/cli/bug/search.rs` gains two clap
arguments and `src/commands/bug/search.rs` routes them into the non-URL parameter branch. No
new module, type, or transport.

Tech stack: Rust 2021, clap derive, reqwest, wiremock, Bash for the functional harness.

Spec: `docs/workflow/specs/2026-09-05-server-saved-search-design.md`
Issue: #670 — Branch: `feat/saved-search-670` — Base branch: `main`

Expected implementation size: 295–420 changed lines (M) — summed from the file map below. Two
review-driven corrections moved it: cutting the functional phase's seeding fixture removed
about 25 lines, and the six `container-tests.sh` gap-model edits Task 3 step 7 now requires
added about 15.
The charter's `$divination` complexity of S describes the decision surface; this band
describes changed lines, which the repository's test-sibling and functional-phase
conventions dominate. They measure different things.

## Global Constraints

- Guardrails run **bare** — no pipes, no `|| true`; the exit code is the result.
  `make lint` (fmt, clippy `-D warnings` pedantic with `unwrap_used` denied,
  check-build-script, check-test-layout, check-functional-test-ids, check-no-spawn,
  check-release-security-notes, check-shell); `make test` (~3 min); `make functional-test`
  (~10 min, needs Docker/podman). Iterate with `make test-one T=<substring>`. **Never bare
  `cargo test`.** Background the long runs and read once; re-invoking on an apparent timeout
  restarts the suite.
- Unit tests live in sibling `<name>_tests.rs` files. An inline `mod tests { … }` in `src/`
  is forbidden and `make check-test-layout` fails on one. API tests use `#[tokio::test]`.
- User-facing output goes through `Writers`; never `println!`/`eprintln!` in `src/`.
- Functional test ids match `^[a-z0-9]+(-[a-z0-9]+)*$`, phase filenames
  `^[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*$`; phase scripts use 4-space indentation.
- A new CLI long flag needs its entry in the `## Command Tree` block of `docs/bzr-cli.md` or
  `agent-skills/tests/flag-drift-check.sh` fails.
- Owned by the concurrent sibling issue #672, do not touch: `src/cli/bug/create.rs`,
  `src/cli/bug/update.rs`, `src/commands/bug/update/`, `src/types/bug/payload.rs`,
  `schemas/`. Shared with it — keep edits minimal and inside this feature's own sections:
  `docs/bzr-cli.md`, `docs/dev/python-bugzilla-parity.md`,
  `tests/functional/compare/01-bug-lifecycle.sh`.

## File map

| File | | Answerable for |
|---|---|---|
| `src/types/bug/search.rs` | changed | the two new `SearchParams` fields, filter-predicate membership |
| `src/types/bug/search_tests.rs` | changed | filter-predicate coverage |
| `src/client/resources/bug.rs` | changed | REST query-parameter emission |
| `src/client/resources/bug_tests.rs` | changed | wiremock proof REST carries both parameters |
| `src/xmlrpc/resources/bug.rs` | changed | XML-RPC member emission |
| `src/xmlrpc/resources/bug_tests.rs` | changed | proof the XML-RPC call carries both members |
| `src/cli/bug/search.rs` | changed | the two clap arguments, constraints, help text |
| `src/cli/bug/search_tests.rs` | changed | parser-level constraint coverage |
| `src/commands/bug/search.rs` | changed | routing the flags into the non-URL branch |
| `src/commands/bug/search_tests.rs`, `src/commands/bug/mod_tests.rs` | changed | end-to-end request coverage; literal `SearchArgs` sites |
| `docs/bzr-cli.md` | changed | command tree, prose, examples, options table |
| `docs/dev/python-bugzilla-parity.md` | changed | the saved-search status row and its footnote |
| `tests/functional/pybz/container-tests.sh` | changed | the whole #670 gap model: stub `run_bzr` arm, parity-row literal, PASS/FAIL/GAP counts, and three gap controls |
| `tests/functional/compare/01-bug-lifecycle.sh` | changed | dropping the expected-gap marking |
| `tests/functional/phases/08f-bug-saved-search.sh` | created | functional coverage on a real container |
| `tests/functional/run-tests.sh` | changed | sourcing the new phase |

## Task 1 — parameter fields and transport mapping

**Interfaces.** Consumes nothing. Later tasks rely on two fields added to the existing
`pub struct SearchParams`. Every in-crate construction site uses functional-update syntax
(`..Default::default()` or `..params.clone()`), so adding fields does not break them; the
struct's `#[non_exhaustive]` constrains downstream crates only and is inert here.

```rust
pub saved_search: Option<String>,
pub sharer_id: Option<u64>,
```

**Verification**

- Contract: REST `Bug.search` carries `savedsearch` and `sharer_id` as query parameters.
  Mode: focused-test. Test: `src/client/resources/bug_tests.rs`,
  `search_bugs_sends_saved_search_and_sharer_id`. Red: `E0560`, no field `saved_search` on
  `SearchParams`. Green: `make test-one T=search_bugs_sends_saved_search_and_sharer_id`.
- Contract: XML-RPC `Bug.search` carries both as call members. Mode: focused-test. Test:
  `src/xmlrpc/resources/bug_tests.rs`, `search_bugs_sends_saved_search_and_sharer_id_xmlrpc`.
  Red: the fields exist by this point, so the test compiles and fails on the assertion — the
  recorded call body contains neither member. Green:
  `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`.
- Contract: a saved-search name alone counts for `has_filters` and for
  `has_structured_filters`. (`has_filters` is a consistency invariant — it has no production
  caller; `has_structured_filters` is behavioural and gates hybrid mode's XML-RPC retry.)
  Mode: focused-test. Test: `src/types/bug/search_tests.rs`,
  `saved_search_counts_as_a_filter_and_a_structured_filter`. Red: written before step 4's
  predicate edits, so both assertions fail on a `SearchParams` whose predicates do not yet
  know the field. Green:
  `make test-one T=saved_search_counts_as_a_filter_and_a_structured_filter`.

**Steps**

1. In `src/client/resources/bug_tests.rs`, add `search_bugs_sends_saved_search_and_sharer_id`
   in the file's existing `#[tokio::test]` + `Mock::given(method("GET")).and(path("/rest/bug"))`
   style, matching `query_param("savedsearch", "my search")` and
   `query_param("sharer_id", "112233")`, responding `{"bugs": []}` with `.expect(1)`, and
   asserting the returned vector is empty. Reuse the file's existing client constructor and
   imports; add no second helper.

2. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id`. Expect a compile error
   naming `saved_search` as an unknown field.

3. In `src/types/bug/search.rs`, add the two fields to `SearchParams` after `quicksearch` and
   before `include_fields`:

   ```rust
   /// Name of a server-side saved search (Bugzilla `savedsearch`).
   ///
   /// Distinct from bzr's local saved queries (`bzr query`): this names a
   /// query stored in the Bugzilla account. Resolving it is a Red Hat
   /// Bugzilla extension; a stock Bugzilla accepts the parameter and
   /// ignores it, so the search degrades to an unfiltered one.
   pub saved_search: Option<String>,
   /// Numeric Bugzilla user ID owning a shared saved search
   /// (Bugzilla `sharer_id`). Only meaningful alongside `saved_search`.
   pub sharer_id: Option<u64>,
   ```

4. In `src/types/bug/search_tests.rs`, add the predicate test **before** touching either
   predicate, so it has an observable red:

   ```rust
   #[test]
   fn saved_search_counts_as_a_filter_and_a_structured_filter() {
       let params = SearchParams {
           saved_search: Some("my search".to_string()),
           ..Default::default()
       };
       assert!(params.has_filters());
       assert!(params.has_structured_filters());
   }
   ```

   Run `make test-one T=saved_search_counts_as_a_filter`. Expect it to compile (step 3 added
   the fields) and fail on the first assertion, because neither predicate knows the field
   yet.

5. In `src/types/bug/search.rs`, add `|| self.saved_search.is_some()` to **both**
   `has_filters` and `has_structured_filters`. Append one sentence to the latter's doc
   comment saying why `saved_search` is included where `quicksearch` and `summary` are not:
   those two are excluded because upstream evaluates them through one shared free-text parser,
   a verified property, whereas no comparable property is known for a fork's saved-search
   handling — so the retry stays available for the one vendor extension bzr sends, at the cost
   of one capped round trip on a result that was already empty.

   Run `make test-one T=saved_search_counts_as_a_filter`. Expect one pass.

6. In `src/client/resources/bug.rs`, add `("savedsearch", &params.saved_search)` to the
   `option_fields` slice in `append_option_params`, after the `quicksearch` entry; and after
   that function's `offset` block add:

   ```rust
   if let Some(sharer_id) = params.sharer_id {
       builder = builder.query(&[("sharer_id", sharer_id)]);
   }
   ```

7. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id`. Expect one pass.

8. In `src/xmlrpc/resources/bug_tests.rs`, add
   `search_bugs_sends_saved_search_and_sharer_id_xmlrpc`, reusing the mock-server setup and
   request-body capture of the file's existing `search_bugs_returns_results` verbatim, and
   asserting the recorded body contains `savedsearch` with the name and `sharer_id` with the
   integer.

9. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expect a
   compiling test that fails its assertion: both members absent from the recorded call body.

10. In `src/xmlrpc/resources/bug.rs` `search_bugs`, add
   `("savedsearch", &params.saved_search)` to the `option_fields` slice after `quicksearch`,
   and after the `offset` insertion add the line below. Do not hand-write the range check:
   `src/xmlrpc/resources/mappers.rs` already exports
   `pub(crate) fn xmlrpc_id(value: u64, label: &str) -> Result<Value>`, which performs this
   exact `i64::try_from` conversion and raises the input error on overflow, and this file
   already imports it.

   ```rust
   if let Some(sharer_id) = params.sharer_id {
       rpc_params.insert("sharer_id".into(), xmlrpc_id(sharer_id, "sharer ID")?);
   }
   ```

11. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expect one
    pass.

12. Run `make lint` bare. Expect exit 0. Commit:
    `feat(search): carry saved-search parameters on both transports`.

**Acceptance criteria.** Both transports emit each parameter when set and omit it when
`None`; both `has_filters()` and `has_structured_filters()` are true for a
saved-search-only `SearchParams`; `make lint` and `make test` green.

## Task 2 — CLI flags and command routing

**Interfaces.** Consumes Task 1's two `SearchParams` fields. Adds to the existing
`pub(crate) struct SearchArgs` in `src/cli/bug/search.rs` (`#[derive(Args, Debug)]`, sitting
beside `query: Option<String>`, `from_url: Option<String>`, `save_as: Option<String>`):

```rust
pub saved_search: Option<String>,
pub sharer: Option<u64>,
```

These feed the existing
`async fn resolve_client_and_params(args: &SearchArgs, ctx: &CommandContext) -> Result<SearchPlan>`
in `src/commands/bug/search.rs`, where
`type SearchPlan = (BugzillaClient, SearchParams, Option<(String, SavedQuery)>)`.

**Verification**

- Contract: `--saved-search` conflicts with the positional query and with `--from-url`;
  `--sharer` requires `--saved-search`; `--sharer` rejects a non-numeric value. Mode:
  focused-test. Test: `src/cli/bug/search_tests.rs`, four cases —
  `saved_search_conflicts_with_positional_query`, `saved_search_conflicts_with_from_url`,
  `sharer_requires_saved_search`, `sharer_rejects_non_numeric`. Red: the tests **compile**
  — they reference no Rust field, only argv strings through `Cli::try_parse_from` — and clap
  returns `ErrorKind::UnknownArgument` for all four because the flags do not exist yet, so
  each `assert_eq!` fails as `UnknownArgument != <expected kind>`. `UnknownArgument` is
  itself a rejection, so do not accept it as the intended red: these four tests exist only to
  pin the `conflicts_with_all` / `requires` attributes, and only the expected kinds prove
  those. Green: `make test-one T=saved_search_conflicts` and `make test-one T=sharer_re`.
- Contract: a `--saved-search` invocation puts `savedsearch` and `sharer_id` on the outgoing
  request and no `quicksearch`. Mode: focused-test. Test: `src/commands/bug/search_tests.rs`,
  `handle_search_saved_search_passes_saved_search_and_sharer`. Red: written at step 7, after
  the `SearchArgs` fields exist but before step 8 routes them, so it compiles and the wiremock
  mount's `.expect(1)` goes unsatisfied — the request carries neither parameter. Green:
  `make test-one T=handle_search_saved_search_passes`.
- Contract: `bug search` with no query source fails input validation naming all three
  sources, before connecting. Mode: focused-test. Test:
  `src/commands/bug/search_tests.rs`, `handle_search_without_a_query_source_names_all_three`.
  Red: also written at step 7, before step 8 widens the message, so the assertion that the
  rendered error contains `--saved-search` fails against the current two-source text. Green:
  `make test-one T=handle_search_without_a_query_source`.

**Steps**

1. Add the four parser cases to `src/cli/bug/search_tests.rs`. That file already defines
   `fn search_args(args: &[&str]) -> SearchArgs` and
   `fn parse_error_kind(args: &[&str]) -> ErrorKind`; use `parse_error_kind` and add no new
   helper. When a `match` over `ErrorKind` has two arms, name the second variant explicitly —
   clippy runs with `-D warnings`.

   - `["bzr","bug","search","query text","--saved-search","s"]` → `ErrorKind::ArgumentConflict`
   - `["bzr","bug","search","--from-url","https://x/buglist.cgi","--saved-search","s"]` →
     `ErrorKind::ArgumentConflict`
   - `["bzr","bug","search","--sharer","1"]` → `ErrorKind::MissingRequiredArgument`
   - `["bzr","bug","search","--saved-search","s","--sharer","abc"]` →
     `ErrorKind::ValueValidation`

2. Run `make test-one T=saved_search_conflicts` and `make test-one T=sharer_re`. Expect four
   failures of the form `UnknownArgument != ArgumentConflict` (and the corresponding
   `MissingRequiredArgument` / `ValueValidation` mismatches), not a compile error. The
   narrow filters are deliberate: `T=saved_search` would also match Task 1's tests, and each
   unit test is compiled into both the lib and bin targets, so a broad filter roughly doubles
   the reported count.

3. In `src/cli/bug/search.rs`, add the two arguments to `SearchArgs` after `save_as`:

   ```rust
   /// Run a saved search stored on the server (Bugzilla `savedsearch`).
   ///
   /// This is a *server-side* saved search — a query stored in your
   /// Bugzilla account — not one of bzr's local saved queries, which
   /// are managed with `bzr query`. Mutually exclusive with the
   /// positional query and with `--from-url`.
   ///
   /// Resolving a saved search is a Red Hat Bugzilla extension. A stock
   /// Bugzilla accepts the parameter and ignores it, so the search
   /// returns an unfiltered result rather than an error.
   #[arg(long, conflicts_with_all = ["query", "from_url"])]
   pub saved_search: Option<String>,
   /// Numeric Bugzilla user ID of the account that shared the saved
   /// search (Bugzilla `sharer_id`).
   ///
   /// Required only for a search someone else shared with you; Bugzilla
   /// shows the ID in the saved search's own URL. Requires
   /// `--saved-search`.
   #[arg(long, requires = "saved_search")]
   pub sharer: Option<u64>,
   ```

4. Step 3 breaks compilation immediately: `SearchArgs` derives `Args` and `Debug` but **not**
   `Default`, so every struct-literal construction must gain `saved_search: None,` and
   `sharer: None,` beside `save_as`. There are six such sites —
   `src/commands/bug/search_tests.rs` lines 21, 359, 526, 595 and
   `src/commands/bug/mod_tests.rs` lines 32, 175. The other mentions in those files and in
   `src/cli/mod_tests.rs` are destructuring patterns ending in `..` and need no change. Fix
   all six now, then run `cargo build --tests` and fix anything this list missed rather than
   trusting it to be exhaustive.

5. In `src/cli/bug/search.rs` extend `LONG_ABOUT`: after the `--save-as` paragraph insert a
   paragraph saying that `--saved-search <NAME>` runs a saved search stored in the Bugzilla
   account, optionally qualified by `--sharer <ID>` when another user shared it; that
   resolving one is a Red Hat Bugzilla extension, so a stock Bugzilla accepts both parameters
   and ignores them, returning an unfiltered result; and that these are unrelated to bzr's
   local saved queries, which `bzr query` manages. Add one line to the `Examples:` block:
   `bzr bug search --saved-search "my triage list" --sharer 112233`.

6. Run `make test-one T=saved_search_conflicts` and `make test-one T=sharer_re`. Expect the
   four new parser cases to pass.

7. Add the two command-level tests to `src/commands/bug/search_tests.rs` **now**, before the
   routing exists, so each has an observable red. Model them on that file's existing
   `handle_search_quicksearch_passes_limit_and_field_filters` — same `setup_test_env().await`
   fixture, same `Mock::given(method("GET")).and(path("/rest/bug"))` shape, same
   `crate::commands::bug::execute(&action, &CommandContext::new(None, OutputFormat::Json,
   None), &mut io.writers())` call:

   - `handle_search_saved_search_passes_saved_search_and_sharer` — action with `query: None`,
     `saved_search: Some("my search".into())`, `sharer: Some(112_233)`; match on
     `query_param("savedsearch", "my search")` and `query_param("sharer_id", "112233")` with
     `.expect(1)`; assert `Ok`.
   - `handle_search_without_a_query_source_names_all_three` — action with `query`, `from_url`
     and `saved_search` all `None`; mount **no** `Mock` at all, which is what proves the error
     precedes the connection; assert `Err` and that the rendered message contains
     `--saved-search`.

   Run `make test-one T=handle_search_saved_search_passes` and
   `make test-one T=handle_search_without_a_query_source`. Expect the first to fail on the
   unsatisfied `.expect(1)` (the request carries neither parameter) and the second to fail
   because the current message names only two sources.

8. In `src/commands/bug/search.rs`, replace the query-source resolution inside the
   `let Some(url_str) = args.from_url.as_deref() else { … }` block. The current body unwraps
   `args.query` into `query_str` and errors when absent; replace it with a presence check
   that runs *before* `connect_and_configure`, so an input error still costs no round trip:

   ```rust
   let Some(url_str) = args.from_url.as_deref() else {
       if args.query.is_none() && args.saved_search.is_none() {
           return Err(crate::error::BzrError::input(
               "a search query, --saved-search, or --from-url is required".into(),
           ));
       }
       let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
       let params = SearchParams {
           quicksearch: args.query.clone(),
           saved_search: args.saved_search.clone(),
           sharer_id: args.sharer,
           limit: Some(args.limit.unwrap_or(DEFAULT_SEARCH_LIMIT)),
           include_fields: canonical_field_list(fields),
           exclude_fields: canonical_field_list(exclude_fields),
           order: Some(crate::validation::build_order(
               args.sort_args.sort.as_deref(),
               args.sort_args.order,
           )),
           ..Default::default()
       };
       return Ok((client, params, None));
   };
   ```

   Assigning both `quicksearch` and `saved_search` unconditionally is safe because clap has
   already rejected the combination; exactly one is `Some`.

9. Run `make test-one T=handle_search_saved_search_passes` and
   `make test-one T=handle_search_without_a_query_source`. Expect both to pass.

10. Run `make lint` bare, then `make test` bare in the background and read the result once.
    Expect exit 0. Commit: `feat(search): add --saved-search and --sharer flags`.

**Acceptance criteria.** `--saved-search` sets `saved_search` and leaves `quicksearch` unset;
`--sharer` sets `sharer_id`; the four rejections happen at parse time (exit 2); omitting all
three sources fails input validation naming all three; `make lint` and `make test` green.

## Task 3 — documentation and comparison status

**Interfaces.** Consumes the flag names from Task 2. Nothing later depends on this task.

**Verification**

- Contract: every command-specific long flag the binary exposes for `bug search` appears in
  that command's `## Command Tree` block and vice versa. Mode: focused-test. Test:
  `agent-skills/tests/flag-drift-check.sh`. Red: `command tree is missing --saved-search for
  \`bug search\``, exit 1. Green: `BZR_BIN=target/debug/bzr sh
  agent-skills/tests/flag-drift-check.sh` exits 0 with no ERROR lines.
- Contract: the lifecycle comparison's saved-search block no longer marks #670 an expected
  gap, and every fixture that modelled that gap agrees. Mode: focused-test. Test:
  `tests/functional/pybz/container-tests.sh`, which sources the real
  `tests/functional/compare/01-bug-lifecycle.sh` against a stub `run_bzr` and asserts the
  PASS/FAIL/GAP counts, the per-slug result lines, and the parity-row literals. Red: with the
  phase edited (step 8) and the fixture not (step 7), the run fails on `lifecycle pass count`,
  `lifecycle gap count`, and the `saved-search ... GAP (#670)` grep. Green:
  `bash tests/functional/pybz/container-tests.sh` exits 0. It needs no container.

**Steps**

1. Run `cargo build`, then `BZR_BIN=target/debug/bzr sh
   agent-skills/tests/flag-drift-check.sh` bare. Expect exit 1 naming both missing flags.

2. In the `## Command Tree` fenced block of `docs/bzr-cli.md`, extend the `bug search` node's
   continuation line to read:

   ```text
   │   │          [--saved-search <NAME>] [--sharer <ID>] [--sort <FIELD>] [--order asc|desc]
   ```

3. Re-run the flag-drift check bare. Expect exit 0 and no ERROR lines.

4. In the `### \`bzr bug search\`` section, add
   `bzr bug search --saved-search "my triage list" --sharer 112233` to the fenced `bash`
   examples after the last `--from-url` line, and immediately after the sentence
   `` `--from-url` and the positional `<QUERY>` argument are mutually exclusive. `` add:

   ```markdown
   `--saved-search` is a third, mutually exclusive query source: it runs a saved search stored in your Bugzilla account, which is unrelated to bzr's local saved queries (see [`bzr query`](#bzr-query----saved-query-management)).

   > **Note:** Resolving a server-side saved search is a Red Hat Bugzilla extension. A stock Bugzilla accepts `savedsearch` and `sharer_id` and ignores them, so `--saved-search` against such a server returns an unfiltered result rather than an error. Confirmed against Bugzilla 5.0.6, 5.2, and 5.3.3+: none implements the parameters, and each accepts them over REST without error.
   ```

5. In that section's options table, add after the `--save-as [NAME]` row:

   ```markdown
   | `--saved-search <NAME>` | No* | | Run a saved search stored on the server (Bugzilla `savedsearch`). Mutually exclusive with `<QUERY>` and `--from-url`. Resolving it requires a Bugzilla with the Red Hat saved-search extension; a stock server ignores the parameter. |
   | `--sharer <ID>` | No | | Numeric Bugzilla user ID of the account that shared the saved search (Bugzilla `sharer_id`). Requires `--saved-search`. |
   ```

   and change the trailing footnote to
   `*One of \`<QUERY>\`, \`--saved-search\`, or \`--from-url\` must be provided.`

6. In `docs/dev/python-bugzilla-parity.md`, change the `Server saved search` row's Status
   cell from `expected gap (#670)` to `parity [^ss]`, and add a footnote definition
   immediately after the table:

   ```markdown
   [^ss]: Both clients send Bugzilla's `savedsearch`/`sharer_id`, which are a Red Hat extension. No supported functional image implements them — each accepts the parameters and returns an unfiltered result — so `compare/01-bug-lifecycle/saved-search` cannot distinguish a resolved saved search from an unfiltered one. The row records parameter parity, not verified resolution.
   ```

   The row still reads `parity`, which is the sourced criterion; the footnote applies this
   design's own disclosure rule to the one durable artifact most likely to be quoted out of
   context. Change nothing else in that table.

7. **`tests/functional/pybz/container-tests.sh` models the whole #670 gap in five places, not
   just the parity row.** It drives the real `01-bug-lifecycle.sh` through a stub `run_bzr`
   and asserts the resulting counts, so closing the gap without updating all of it leaves
   `make functional-compare-all` permanently red. Make every edit below in the same commit as
   step 8's comparison-script change. Line numbers are from `main` and will drift as you edit;
   the fixture run in step 9 is the authority.

   a. **Parity row literal, line 960.** `run_parity_report_fixture` (defined line 950, invoked
      line 3011) holds every parity-report row as a literal and asserts `grep -Fxc` finds each
      exactly once. Replace

      ```text
      '| Server saved search | `bzr bug search --saved-search` | expected gap (#670) | `compare/01-bug-lifecycle/saved-search` |'
      ```

      with the new row text, byte for byte including the footnote marker. Touch no other row:
      sibling issue #672 owns the `Comment tags and minor update` entry two lines below.

   b. **Stub `run_bzr`, lines 600-613.** Remove `--saved-search` from the unsupported-flag
      condition and delete its arm from the `case` that assigns `diagnostic`. Add a supported
      arm on the non-stale path returning the two lifecycle ids, mirroring the stale-path arm
      at line 625: `printf '[{"id":41},{"id":42}]\n' >"$BZR_STDOUT"` then
      `fixture_finish_bzr 0`. Leave the stale-path arm at 625 as it is. Without this the
      converted `lifecycle_bzr` probe takes `lifecycle_bzr_probe`'s `BZR_EXIT -ne 0` branch
      (`01-bug-lifecycle.sh:53-59`) and calls `test_fail`.

   c. **Baseline counts, lines 790-792.** `assert_equals 5 "$PASS_COUNT"` becomes `6`;
      `assert_equals 0 "$FAIL_COUNT"` is unchanged; `assert_equals 5 "$GAP_COUNT"` becomes `4`.

   d. **`run_eligibility_reset_control`, line 463.** Its first `grep -Fq` looks for
      `'[compare/01-bug-lifecycle/saved-search] server saved search ... GAP (#670)'`, a line
      no longer emitted. Change it to the PASS line the same probe now produces. Keep the
      second grep (`arbitrary-fields ... FAIL`) as it is: the control's subject is that gap
      eligibility from the preceding probe does not leak into the injected failure on
      `arbitrary-fields-create`, and saved-search passing cleanly still exercises that. Do not
      re-point it at a later slug — the control depends on saved-search running *before*
      arbitrary-fields in the phase.

   e. **`run_noop_stale_gap_control`, line 480.** `GAP_COUNT -ne 5` becomes `GAP_COUNT -ne 4`.

   f. **`LIFECYCLE_STALE_GAPS` control, lines 932 and 938.** Drop `670` from
      `for issue in 670 671 672 679 680`, and change `assert_equals 5 "$FAIL_COUNT"` to `4`.
      `expect_gap` emits `#N appears resolved` only from its PASS arm
      (`tests/functional/lib.sh:236-243`), and there is no longer a call for 670.

   g. **Two `run_gap_ineligible_control` entries, lines 875-887.** Six of the eight injected
      controls (missing/mixed events, connection failure, server error, malformed result,
      downstream assertion) still make the converted `lifecycle_bzr` probe fail and stay on
      `saved-search`. Two do not: `LIFECYCLE_WRONG_PARSER_DIAGNOSTIC` and
      `LIFECYCLE_EXPECTED_DIAGNOSTIC_EXIT_ONE` only take effect inside the unsupported-flag
      branch, which `LIFECYCLE_STALE_GAPS=1` skips, so saved-search now passes under them and
      the control's `... FAIL` assertion breaks. Move those two to a slug that still uses
      `lifecycle_bzr_gap`:
      `run_gap_ineligible_control "$control" query-match-types 'whiteboard match types'`
      (`01-bug-lifecycle.sh:540`). Avoid `update-options` — sibling issue #672 is closing that
      gap concurrently.

      Note for the completion report, not for fixing here: those two controls are already
      weaker than their names suggest, because `run_gap_ineligible_control` forces
      `LIFECYCLE_STALE_GAPS=1` and that flag skips the very branch they inject into. That is a
      pre-existing property of the fixture, not something this change introduces.

8. In the `saved-search` block of `tests/functional/compare/01-bug-lifecycle.sh`, replace

   ```bash
       if lifecycle_bzr_gap saved-search "error: unexpected argument '--saved-search' found" \
           bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
   ```

   with

   ```bash
       if lifecycle_bzr saved-search bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
   ```

   and delete the `    lifecycle_expect_gap 670` line four lines below it. Touch no other
   block in that file.

9. Run `bash tests/functional/pybz/container-tests.sh` bare. This is the observation that
   catches any disagreement between step 7's fixture and step 8's phase edit; it sources the
   real phase against stubs and needs no container. Expect exit 0. Iterate on step 7 until it
   is green — the reported counts and the named slug lines tell you which sub-edit is wrong.

10. Run `make lint` bare — `check-shell` covers both shell files. Expect exit 0. Commit:
    `docs(search): document --saved-search and flip the parity row`.

**Acceptance criteria.** The flag-drift check exits 0; the reference documents both flags,
states the Red Hat caveat, and its options-table footnote names all three query sources; the
parity row reads `parity` with a footnote stating what its evidence covers, and
`bash tests/functional/pybz/container-tests.sh` exits 0 against it; the comparison block
calls `lifecycle_bzr` with no `lifecycle_expect_gap 670`.

## Task 4 — functional phase coverage

**Interfaces.** Consumes Task 2's flags and Task 3's documentation state. Helpers, each
confirmed at the path named: `run_bzr`, `run_bzr_raw`, `test_begin`, `test_pass`,
`assert_success`, `assert_exit_code`, `assert_json_array_min_length` in
`tests/functional/lib.sh`. `BZ_URL` is a global `tests/functional/run-tests.sh` sets before
sourcing any phase.

**The phase seeds no fixture, deliberately** — see the spec's Testing section for why. Use a
literal saved-search name and a literal sharer id so every test always executes and the phase
reports no SKIP.

**Verification**

- Contract: `bug search --saved-search` is accepted by a real Bugzilla over REST and
  XML-RPC, composes with `--count`, works credentiallessly, and rejects its four invalid
  argument combinations. Mode: focused-test. Test:
  `tests/functional/phases/08f-bug-saved-search.sh`. Red: before the phase is added to the
  runner's list `make functional-test` never runs it, and `make lint` fails through
  `check-functional-test-ids`, which compares the runner's `for _phase in` list against the
  phase-directory basenames. Green: `make functional-test` reports all nine of the phase's
  ids as PASS, with no SKIP — the phase has no conditional test, so a SKIP would itself be a
  defect.

**Steps**

1. Create `tests/functional/phases/08f-bug-saved-search.sh` with a header in the style of
   `08b-bugs-paging.sh`. The header must state plainly what the phase cannot prove:

   ```bash
   # 08f-bug-saved-search
   # Sourced by run-tests.sh in order; assumes lib.sh helpers and the
   # orchestrator preamble (constants, shared globals, cleanup trap).
   # Reads: BZ_URL. Creates: nothing.
   # shellcheck shell=bash
   #
   # Proves a real Bugzilla accepts --saved-search / --sharer over REST and
   # XML-RPC, that they compose with --count and the credentialless path,
   # and that the four invalid argument combinations are rejected at parse
   # time. Does NOT prove the server resolved the saved search: resolving
   # `savedsearch` is a Red Hat extension that every supported image here
   # ignores, which is also why the phase seeds nothing. The wire mapping is
   # proven by the wiremock tests in src/client/resources/bug_tests.rs and
   # src/xmlrpc/resources/bug_tests.rs; see
   # docs/workflow/specs/2026-09-05-server-saved-search-design.md.
   ```

2. Print the phase banner (`echo "── Phase 8f: Bug search --saved-search ───────────────"`),
   then declare the two literals, with 4-space indentation throughout the file:

   ```bash
   _SS_NAME="bzr-func-saved-search"
   _SS_SHARER=1
   ```

3. Add five acceptance tests, unconditional:

   ```bash
   test_begin "bug-search-saved-search-rest" "bug search --saved-search over REST"
   run_bzr --api rest bug search --saved-search "$_SS_NAME"
   if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
   ```

   The remaining four, same shape:

   - `bug-search-saved-search-xmlrpc` / "bug search --saved-search over XML-RPC" —
     `run_bzr --api xmlrpc bug search --saved-search "$_SS_NAME"`, then
     `assert_success && assert_json_array_min_length '.' 1`.
   - `bug-search-saved-search-count` / "bug search --saved-search composes with --count" —
     `run_bzr bug search --saved-search "$_SS_NAME" --count`, then `assert_success`.
   - `bug-search-saved-search-sharer` / "bug search --saved-search --sharer" —
     `run_bzr bug search --saved-search "$_SS_NAME" --sharer "$_SS_SHARER"`, then
     `assert_success`.
   - `credentialless-bug-search-saved-search` / "credentialless bug search --saved-search" —
     `run_bzr_raw --json --server-url "$BZ_URL" bug search --saved-search "$_SS_NAME"`, then
     `assert_success`.

4. Add the four rejection tests. Each runs the command and asserts `assert_exit_code 2`:

   ```bash
   test_begin "bug-search-saved-search-rejects-query" "bug search rejects --saved-search with a query"
   run_bzr bug search "some text" --saved-search "$_SS_NAME"
   if assert_exit_code 2; then test_pass; fi
   ```

   The other three: `bug-search-saved-search-rejects-from-url` running
   `bug search --from-url "${BZ_URL}/buglist.cgi?bug_id=1" --saved-search "$_SS_NAME"`;
   `bug-search-sharer-requires-saved-search` running `bug search "some text" --sharer 1`; and
   `bug-search-sharer-rejects-non-numeric` running
   `bug search --saved-search "$_SS_NAME" --sharer not-a-number`.

5. In `tests/functional/run-tests.sh`, add `08f-bug-saved-search` to the `for _phase in \`
   list, immediately after `08e-bugs-restricted-access`.

6. Run `make lint` bare — `check-functional-test-ids` and `check-shell` both cover the new
   file. Expect exit 0.

7. Run `make functional-test` bare in the background and read its result once; it takes
   roughly 10 minutes. Expect exit 0, all nine new ids PASS, and no SKIP among them.

8. Run `make functional-compare` bare in the background and read its result once. This is the
   only run that exercises criterion 5 end to end — the converted `lifecycle_bzr` probe and
   its `lifecycle_ids_are` assertion against a real container. Expect
   `compare/01-bug-lifecycle/saved-search` to report PASS with no GAP. Note that this target
   runs `tests/functional/run-compare.sh` only; `tests/functional/pybz/container-tests.sh` is
   reached solely by `make functional-compare-all` (`Makefile:226`), which is why Task 3 step
   9 runs that file directly.

9. Commit: `test(search): cover --saved-search against a real container`.

**Acceptance criteria.** The phase exists, is sourced by the runner, and its header states
what it cannot prove; every one of its nine tests reports PASS with no SKIP; `make lint`
green including `check-functional-test-ids` and `check-shell`; `make functional-test` exits 0;
`make functional-compare` exits 0 with `compare/01-bug-lifecycle/saved-search` PASS and no
GAP.

## Rollback and cleanup

Every task is additive plus one new phase script; reverting the branch restores the previous
behaviour with no data or configuration migration. The phase creates no server-side state at
all — it seeds no bug and no saved search — so it needs no cleanup beyond the disposable
functional container itself.

## Deferrals carried from review

This repository keeps no `docs/debt/` directory, so neither deferral has a record path. Both
are filed as follow-up tracker issues from this run's completion report; the issue numbers are
reported there rather than back-filled into this plan.

1. **The saved-search comparison assertion is vacuous on every supported image.** Both
   `compare/01-bug-lifecycle/saved-search` clients assert the search returns exactly the two
   lifecycle bug ids, and on upstream Bugzilla that passes because the parameter is ignored
   *and* the container happens to hold exactly those two bugs — not because a saved search was
   resolved. Already true of the python-bugzilla side before this change; flipping bzr's side
   inherits it. The honest fix is a Red-Hat-shaped fixture in
   `tests/functional/redhat-shape-proxy.py`, out of this charter's surface.

2. **Two gap-ineligibility controls cannot exercise what they inject.**
   `run_gap_ineligible_control` forces `LIFECYCLE_STALE_GAPS=1`, and that flag skips the
   unsupported-flag branch that `LIFECYCLE_WRONG_PARSER_DIAGNOSTIC` and
   `LIFECYCLE_EXPECTED_DIAGNOSTIC_EXIT_ONE` are the only injections into. Pre-existing; this
   change only relocates the two entries to a slug that still fails under stale gaps.
