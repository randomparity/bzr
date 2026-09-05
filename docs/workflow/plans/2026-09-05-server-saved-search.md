# Server-side saved search — implementation plan

Goal: `bzr bug search --saved-search <NAME> [--sharer <ID>]` sends Bugzilla's `savedsearch`
and `sharer_id` search parameters over REST and XML-RPC, composing with the existing paging,
projection, sorting and `--count` flags.

Architecture: two optional fields are added to the shared `SearchParams` struct
(`src/types/bug/search.rs`); the two existing transport mappers
(`src/client/resources/bug.rs` for REST, `src/xmlrpc/resources/bug.rs` for XML-RPC) each gain
one string and one numeric entry in the tables they already iterate; the `bug search` CLI
(`src/cli/bug/search.rs`) gains two clap arguments and the command module
(`src/commands/bug/search.rs`) routes them into the non-URL parameter branch. No new module,
type, or transport.

Tech stack: Rust 2021, clap derive, reqwest, wiremock for HTTP unit tests, Bash for the
functional harness.

Spec: `docs/workflow/specs/2026-09-05-server-saved-search-design.md`
Issue: #670 — Branch: `feat/saved-search-670` — Base branch: `main`

Expected implementation size: 270–400 changed lines (M) — derived from the file map below:
four source files taking a two-field addition, four test siblings, two documentation files,
one new functional phase script, and two one-line registrations. The charter's `$divination`
complexity verdict of S describes the decision surface (two additive pass-through parameters,
no new contract shape); this band describes changed lines, which the repository's
test-sibling and functional-phase conventions dominate. The two measure different things and
neither is wrong.

## Global Constraints

- Guardrails, run bare (no pipes, no `|| true`; the exit code is the result):
  - `make lint` — fmt, clippy `-D warnings` (pedantic, `unwrap_used` denied),
    check-build-script, check-test-layout, check-functional-test-ids, check-no-spawn,
    check-release-security-notes, check-shell.
  - `make test` — quiet unit + integration suite, roughly 3 minutes.
  - `make test-one T=<substring>` and `make test-fast` for iteration. **Never bare
    `cargo test`.**
  - `make functional-test` — roughly 10 minutes, requires Docker or podman.
  - Long runs exceed a 2-minute tool timeout: background them and read once on completion.
    Re-invoking on an apparent timeout restarts the suite.
- Unit tests live in sibling `<name>_tests.rs` files linked with
  `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`. An inline `mod tests { … }` block in
  `src/` is forbidden and `make check-test-layout` fails on one.
- All user-facing output goes through `Writers` (`w.out` / `w.err`). Never `println!` or
  `eprintln!` in `src/`.
- API tests use `#[tokio::test]`; the runtime is tokio.
- Functional phase test ids must match `^[a-z0-9]+(-[a-z0-9]+)*$` and phase filenames
  `^[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*$`; `make check-functional-test-ids` enforces both.
  Phase scripts use 4-space indentation.
- Adding a CLI long flag requires the matching entry in the `## Command Tree` block of
  `docs/bzr-cli.md`; `agent-skills/tests/flag-drift-check.sh` fails otherwise.
- Files owned by the concurrently running sibling issue #672 and not to be touched:
  `src/cli/bug/create.rs`, `src/cli/bug/update.rs`, `src/commands/bug/update/`,
  `src/types/bug/payload.rs`, `schemas/`.
- `docs/bzr-cli.md`, `docs/dev/python-bugzilla-parity.md` and
  `tests/functional/compare/01-bug-lifecycle.sh` are shared with that sibling: edits there
  stay minimal, additive, and inside this feature's own sections.

## File map

| File | Created / changed | Answerable for |
|---|---|---|
| `src/types/bug/search.rs` | changed | the two new `SearchParams` fields and their filter-predicate membership |
| `src/types/bug/search_tests.rs` | changed | filter-predicate coverage for those fields |
| `src/client/resources/bug.rs` | changed | REST query-parameter emission |
| `src/client/resources/bug_tests.rs` | changed | wiremock assertion that REST carries both parameters |
| `src/xmlrpc/resources/bug.rs` | changed | XML-RPC member emission |
| `src/xmlrpc/resources/bug_tests.rs` | changed | assertion that the XML-RPC call carries both members |
| `src/cli/bug/search.rs` | changed | the two clap arguments, their constraints, and the help text |
| `src/cli/bug/search_tests.rs` | changed | parser-level constraint coverage |
| `src/commands/bug/search.rs` | changed | routing the flags into the non-URL parameter branch |
| `docs/bzr-cli.md` | changed | command-tree entry, prose, examples, options table |
| `docs/dev/python-bugzilla-parity.md` | changed | the saved-search status row |
| `tests/functional/compare/01-bug-lifecycle.sh` | changed | dropping the expected-gap marking |
| `tests/functional/phases/08f-bug-saved-search.sh` | created | functional coverage against a real container |
| `tests/functional/run-tests.sh` | changed | sourcing the new phase |

## Task 1 — parameter fields and transport mapping

Adds the two fields and makes both transports emit them. Ends at a green focused test proving
each transport puts the parameters on the wire.

**Interfaces**

Consumes nothing from earlier tasks. Later tasks rely on:

```rust
// src/types/bug/search.rs, on the existing `pub struct SearchParams`
pub saved_search: Option<String>,
pub sharer_id: Option<u64>,
```

`SearchParams` is `#[derive(Clone, Debug, Default)]` and `#[non_exhaustive]`, so every
existing construction site using `..Default::default()` keeps compiling unchanged.

**Verification**

- Contract: REST `Bug.search` carries `savedsearch` and `sharer_id` as query parameters.
  Mode: focused-test. Test: `src/client/resources/bug_tests.rs`, new
  `search_bugs_sends_saved_search_and_sharer_id`. Expected red: the field does not exist, so
  the test file fails to compile with `E0560` (no field `saved_search` on `SearchParams`).
  Green: `make test-one T=search_bugs_sends_saved_search_and_sharer_id`.
- Contract: XML-RPC `Bug.search` carries `savedsearch` and `sharer_id` as call members.
  Mode: focused-test. Test: `src/xmlrpc/resources/bug_tests.rs`, new
  `search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expected red: same compile failure.
  Green: `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`.
- Contract: a saved-search name alone counts as a filter for `SearchParams::has_filters`, and
  does not count for `has_structured_filters`. Mode: focused-test. Test:
  `src/types/bug/search_tests.rs`, new `saved_search_is_a_filter_but_not_a_structured_filter`.
  Expected red: same compile failure. Green:
  `make test-one T=saved_search_is_a_filter_but_not_a_structured_filter`.

**Steps**

1. In `src/client/resources/bug_tests.rs`, add the REST test. Follow the existing
   `query_param` style already used in that file:

   ```rust
   #[tokio::test]
   async fn search_bugs_sends_saved_search_and_sharer_id() {
       let mock_server = MockServer::start().await;
       Mock::given(method("GET"))
           .and(path("/rest/bug"))
           .and(query_param("savedsearch", "my search"))
           .and(query_param("sharer_id", "112233"))
           .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "bugs": [] })))
           .expect(1)
           .mount(&mock_server)
           .await;

       let client = test_client(&mock_server);
       let params = SearchParams {
           saved_search: Some("my search".to_string()),
           sharer_id: Some(112_233),
           ..Default::default()
       };
       let bugs = client.search_bugs(&params).await.unwrap();
       assert!(bugs.is_empty());
   }
   ```

   Before writing it, read the top of `src/client/resources/bug_tests.rs` and reuse whatever
   the file already calls its client constructor and its imports; do not introduce a second
   helper.

2. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id`. Expect a compile
   error naming `saved_search` as an unknown field of `SearchParams`.

3. In `src/types/bug/search.rs`, add the two fields to `SearchParams`, after `quicksearch`
   and before `include_fields`:

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

4. In the same file, add `|| self.saved_search.is_some()` to `has_filters`. Leave
   `has_structured_filters` alone, and add one sentence to its doc comment saying why:

   ```rust
   /// `saved_search` is excluded for the same reason: it is resolved by one
   /// server-side sub shared by both transports, so an empty REST result is
   /// authoritative and a retry would return the same rows.
   ```

5. In `src/client/resources/bug.rs`, add `("savedsearch", &params.saved_search)` to the
   `option_fields` slice in `append_option_params`, after the `quicksearch` entry. Then, in
   the same function, after the `offset` block, add:

   ```rust
   if let Some(sharer_id) = params.sharer_id {
       builder = builder.query(&[("sharer_id", sharer_id)]);
   }
   ```

6. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id`. Expect one passing
   test.

7. In `src/xmlrpc/resources/bug_tests.rs`, add the XML-RPC test. Read
   `search_bugs_returns_results` at the top of that file first and reuse its mock-server
   setup and body-capture mechanism verbatim; assert that the recorded request body contains
   `savedsearch` with the name and `sharer_id` with the integer. Name it
   `search_bugs_sends_saved_search_and_sharer_id_xmlrpc`.

8. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expect a
   failure showing the parameters absent from the call body.

9. In `src/xmlrpc/resources/bug.rs`, in `search_bugs`, add
   `("savedsearch", &params.saved_search)` to the `option_fields` slice after `quicksearch`,
   and after the `offset` insertion add:

   ```rust
   if let Some(sharer_id) = params.sharer_id {
       rpc_params.insert("sharer_id".into(), Value::Int(i64::try_from(sharer_id).map_err(
           |_| BzrError::input(format!("--sharer {sharer_id} exceeds the XML-RPC integer range")),
       )?));
   }
   ```

   Do not write that range check by hand: `src/xmlrpc/resources/mappers.rs` already exports
   `pub(crate) fn xmlrpc_id(value: u64, label: &str) -> Result<Value>`, which performs
   exactly this `i64::try_from` conversion and produces the input error on overflow, and
   `src/xmlrpc/resources/bug.rs` already imports it. Write instead:

   ```rust
   if let Some(sharer_id) = params.sharer_id {
       rpc_params.insert("sharer_id".into(), xmlrpc_id(sharer_id, "sharer ID")?);
   }
   ```

10. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expect one
    passing test.

11. In `src/types/bug/search_tests.rs`, add:

    ```rust
    #[test]
    fn saved_search_is_a_filter_but_not_a_structured_filter() {
        let params = SearchParams {
            saved_search: Some("my search".to_string()),
            ..Default::default()
        };
        assert!(params.has_filters());
        assert!(!params.has_structured_filters());
    }
    ```

12. Run `make test-one T=saved_search_is_a_filter_but_not_a_structured_filter`. Expect one
    passing test.

13. Run `make lint` bare. Expect exit 0. Commit:
    `feat(search): carry saved-search parameters on both transports`.

**Acceptance criteria**

- `SearchParams` carries `saved_search: Option<String>` and `sharer_id: Option<u64>`.
- The REST request emits `savedsearch` and `sharer_id` query parameters, and omits each when
  the field is `None`.
- The XML-RPC call emits the matching members, and omits each when the field is `None`.
- `has_filters()` is true for a saved-search-only `SearchParams`; `has_structured_filters()`
  is false.
- `make lint` and `make test` are green.

## Task 2 — CLI flags and command routing

Adds the two clap arguments with their constraints and routes them into the search
parameters. Ends at a green focused test for every constraint.

**Interfaces**

Consumes `SearchParams::saved_search` and `SearchParams::sharer_id` from Task 1. Later tasks
rely on:

```rust
// src/cli/bug/search.rs, on the existing `pub(crate) struct SearchArgs`
pub saved_search: Option<String>,
pub sharer: Option<u64>,
```

The struct is `#[derive(Args, Debug)]`; the existing fields it is added beside are `query:
Option<String>`, `from_url: Option<String>`, and `save_as: Option<String>`. The command entry
point it feeds is the existing
`async fn resolve_client_and_params(args: &SearchArgs, ctx: &CommandContext) -> Result<SearchPlan>`
in `src/commands/bug/search.rs`, where
`type SearchPlan = (BugzillaClient, SearchParams, Option<(String, SavedQuery)>)`.

**Verification**

- Contract: `--saved-search` conflicts with the positional query and with `--from-url`;
  `--sharer` requires `--saved-search`; `--sharer` rejects a non-numeric value. Mode:
  focused-test. Test: `src/cli/bug/search_tests.rs`, four new cases —
  `saved_search_conflicts_with_positional_query`,
  `saved_search_conflicts_with_from_url`,
  `sharer_requires_saved_search`,
  `sharer_rejects_non_numeric`. Expected red: the parser accepts the combinations, so each
  `expect_err` assertion fails. Green: `make test-one T=saved_search` and
  `make test-one T=sharer`.
- Contract: a `--saved-search` invocation puts `savedsearch` and `sharer_id` on the outgoing
  request and no `quicksearch`. Mode: focused-test. Test: `src/commands/bug/search_tests.rs`,
  new `handle_search_saved_search_passes_saved_search_and_sharer`. Expected red: the field
  does not exist on `SearchArgs`, so the test file fails to compile. Green:
  `make test-one T=handle_search_saved_search_passes_saved_search_and_sharer`.
- Contract: `bug search` with no query source fails input validation naming all three
  sources. Mode: focused-test. Test: `src/commands/bug/search_tests.rs`, new
  `handle_search_without_a_query_source_names_all_three`. Expected red: the message names
  only two. Green:
  `make test-one T=handle_search_without_a_query_source_names_all_three`.

**Steps**

1. Read `src/cli/bug/search_tests.rs` in full. Note which clap `ErrorKind` the file's
   existing cases assert for each failure class; in this repository a conflict is
   `ArgumentConflict`, a missing `requires` target is `MissingRequiredArgument`, and a
   non-numeric integer is `ValueValidation`. When a `match` over `ErrorKind` has two arms,
   name the second variant explicitly — clippy runs with `-D warnings`.

2. Add the four parser cases to `src/cli/bug/search_tests.rs`. That file already defines
   `fn search_args(args: &[&str]) -> SearchArgs` and
   `fn parse_error_kind(args: &[&str]) -> ErrorKind`; use `parse_error_kind` for these four
   and add no new helper. Each parses an argv vector and asserts the expected `ErrorKind`:

   - `["bzr","bug","search","query text","--saved-search","s"]` → `ArgumentConflict`
   - `["bzr","bug","search","--from-url","https://x/buglist.cgi","--saved-search","s"]` →
     `ArgumentConflict`
   - `["bzr","bug","search","--sharer","1"]` → `MissingRequiredArgument`
   - `["bzr","bug","search","--saved-search","s","--sharer","abc"]` → `ValueValidation`

3. Run `make test-one T=saved_search` and `make test-one T=sharer`. Expect compile failure
   naming the unknown arguments.

4. In `src/cli/bug/search.rs`, add the two arguments to `SearchArgs`, after `save_as`:

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

5. In the same file, extend `LONG_ABOUT`. After the `--save-as` paragraph, insert:

   ```text
   `--saved-search <NAME>` runs a saved search stored in your
   Bugzilla account, optionally qualified by `--sharer <ID>` when
   another user shared it. Resolving one is a Red Hat Bugzilla
   extension: a stock Bugzilla accepts both parameters and ignores
   them, returning an unfiltered result. These are unrelated to
   bzr's local saved queries, which `bzr query` manages.
   ```

   and add one example line to the `Examples:` block:

   ```text
     bzr bug search --saved-search "my triage list" --sharer 112233
   ```

6. Run `make test-one T=saved_search` and `make test-one T=sharer`. Expect all four parser
   tests to pass.

7. In `src/commands/bug/search.rs`, replace the query-source resolution inside the
   `let Some(url_str) = args.from_url.as_deref() else { … }` block. The current body starts
   by unwrapping `args.query` into `query_str` and erroring when it is absent. Replace that
   with a presence check that runs *before* the connection, so an input error still costs no
   network round trip:

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

8. `SearchArgs` derives `Args` and `Debug` but **not** `Default`, so every struct-literal
   construction of it must gain the two new fields or the crate will not compile. There are
   exactly six such literal sites — `src/commands/bug/search_tests.rs` lines 21, 359, 526 and
   595, and `src/commands/bug/mod_tests.rs` lines 32 and 175. Add

   ```rust
       saved_search: None,
       sharer: None,
   ```

   to each, beside the existing `save_as` field. The other `SearchArgs` mentions in those
   files and in `src/cli/mod_tests.rs` are destructuring patterns ending in `..` and need no
   change. Run `cargo build --tests` and fix any site this list missed rather than assuming
   the list is exhaustive.

9. Add the two command-level tests to `src/commands/bug/search_tests.rs`, modelled on the
   existing `handle_search_quicksearch_passes_limit_and_field_filters` in that file — the
   same `setup_test_env().await` fixture, the same `Mock::given(method("GET")).and(path(
   "/rest/bug"))` shape, and the same `crate::commands::bug::execute(&action, &CommandContext
   ::new(None, OutputFormat::Json, None), &mut io.writers())` call:

   - `handle_search_saved_search_passes_saved_search_and_sharer` — build the action with
     `query: None`, `saved_search: Some("my search".into())`, `sharer: Some(112_233)`, and
     match on `query_param("savedsearch", "my search")` and
     `query_param("sharer_id", "112233")`. Assert the result is `Ok`.
   - `handle_search_without_a_query_source_names_all_three` — build the action with `query`,
     `from_url` and `saved_search` all `None`, mount no `Mock` at all, assert the result is
     `Err`, and assert the rendered error string contains `--saved-search`. Mounting nothing
     is what proves the error precedes the connection.

10. Run `make test-one T=handle_search_saved_search_passes_saved_search_and_sharer` and
    `make test-one T=handle_search_without_a_query_source_names_all_three`. Expect both to
    pass.

11. Run `make lint` bare, then `make test` bare in the background and read the result once.
    Expect exit 0 from both. Commit: `feat(search): add --saved-search and --sharer flags`.

**Acceptance criteria**

- `bzr bug search --saved-search NAME` builds `SearchParams` with `saved_search` set,
  `quicksearch` unset, the default limit of 50, and the built order.
- `--sharer` sets `sharer_id`.
- The four rejections happen at parse time with exit code 2.
- Omitting all three query sources fails input validation with a message naming all three.
- `make lint` and `make test` are green.

## Task 3 — documentation and comparison status

Makes the documented surface match the binary and drops the expected-gap marking. Ends at a
green flag-drift check and a comparison script whose saved-search block no longer expects a
gap.

**Interfaces**

Consumes the flag names `--saved-search` and `--sharer` from Task 2. Nothing later depends on
this task.

**Verification**

- Contract: every command-specific long flag the binary exposes for `bug search` appears in
  that command's `## Command Tree` block, and vice versa. Mode: focused-test. Test:
  `agent-skills/tests/flag-drift-check.sh`. Expected red: before the tree is updated, the
  check prints `command tree is missing --saved-search for \`bug search\`` and exits 1.
  Green: `BZR_BIN=target/debug/bzr sh agent-skills/tests/flag-drift-check.sh` exits 0 with no
  ERROR lines.
- Contract: the lifecycle comparison's saved-search block no longer marks issue #670 as an
  expected gap. Mode: task-test-not-applicable. Changed surface: two lines of a Bash
  comparison script that only executes inside a container-backed comparison run. No
  executable or structural observation available in this task can fail meaningfully on it —
  the observation that can is the comparison run itself, which Task 4 performs.

**Steps**

1. Run `cargo build` so `target/debug/bzr` reflects Task 2.

2. Run `BZR_BIN=target/debug/bzr sh agent-skills/tests/flag-drift-check.sh` bare. Expect exit
   1 with `command tree is missing --saved-search for \`bug search\`` and the same for
   `--sharer`.

3. In `docs/bzr-cli.md`, in the `## Command Tree` fenced block, extend the `bug search` node.
   It currently reads:

   ```text
   │   ├── search [<QUERY>] [--from-url <URL>] [--save-as [NAME]] [--limit <N>] [--offset <N>] [--paginate] [--count] [--fields <F>] [--exclude-fields <F>]
   │   │          [--sort <FIELD>] [--order asc|desc]
   ```

   Add the two flags to the continuation line so it reads:

   ```text
   │   │          [--saved-search <NAME>] [--sharer <ID>] [--sort <FIELD>] [--order asc|desc]
   ```

4. Re-run `BZR_BIN=target/debug/bzr sh agent-skills/tests/flag-drift-check.sh` bare. Expect
   exit 0 and no ERROR lines.

5. In the `### \`bzr bug search\`` section of `docs/bzr-cli.md`, add one example line to the
   fenced `bash` block, after the last `--from-url` example:

   ```bash
   bzr bug search --saved-search "my triage list" --sharer 112233
   ```

6. Immediately after the sentence
   `` `--from-url` and the positional `<QUERY>` argument are mutually exclusive. ``, add:

   ```markdown
   `--saved-search` is a third, mutually exclusive query source: it runs a saved search stored in your Bugzilla account, which is unrelated to bzr's local saved queries (see [`bzr query`](#bzr-query)).

   > **Note:** Resolving a server-side saved search is a Red Hat Bugzilla extension. A stock Bugzilla accepts `savedsearch` and `sharer_id` and ignores them, so `--saved-search` against such a server returns an unfiltered result rather than an error. Verified against Bugzilla 5.0.6, 5.2, and 5.3.3+.
   ```

7. In the same section's options table, add two rows after the `--save-as [NAME]` row:

   ```markdown
   | `--saved-search <NAME>` | No* | | Run a saved search stored on the server (Bugzilla `savedsearch`). Mutually exclusive with `<QUERY>` and `--from-url`. Resolving it requires a Bugzilla with the Red Hat saved-search extension; a stock server ignores the parameter. |
   | `--sharer <ID>` | No | | Numeric Bugzilla user ID of the account that shared the saved search (Bugzilla `sharer_id`). Requires `--saved-search`. |
   ```

   Change the table's trailing footnote from
   `*One of \`<QUERY>\` or \`--from-url\` must be provided.` to
   `*One of \`<QUERY>\`, \`--saved-search\`, or \`--from-url\` must be provided.`

8. In `docs/dev/python-bugzilla-parity.md`, change the `Server saved search` row's Status
   cell from `expected gap (#670)` to `parity`. Change nothing else in that table.

9. In `tests/functional/compare/01-bug-lifecycle.sh`, in the `saved-search` block: change

   ```bash
       if lifecycle_bzr_gap saved-search "error: unexpected argument '--saved-search' found" \
           bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
   ```

   to

   ```bash
       if lifecycle_bzr saved-search bug search --saved-search "$LIFECYCLE_SAVED_SEARCH" &&
   ```

   and delete the `    lifecycle_expect_gap 670` line four lines below it. Touch no other
   block in that file.

10. Run `make lint` bare. Expect exit 0 — `check-shell` covers the comparison script. Commit:
    `docs(search): document --saved-search and flip the parity row`.

**Acceptance criteria**

- `BZR_BIN=target/debug/bzr sh agent-skills/tests/flag-drift-check.sh` exits 0.
- The `bug search` reference section documents both flags, states the Red Hat extension
  caveat, and its footnote names all three query sources.
- The parity report's saved-search row reads `parity`.
- The comparison script's saved-search block calls `lifecycle_bzr` and contains no
  `lifecycle_expect_gap 670`.
- `make lint` is green.

## Task 4 — functional phase coverage

Exercises the new flags against a real Bugzilla container, including the credentialless path.
Ends at a green functional run.

**Interfaces**

Consumes the flags from Task 2 and the documentation state from Task 3. Nothing later depends
on this task.

Helpers this task uses, each confirmed present at the path named: `run_bzr`, `run_bzr_raw`,
`make_bug`, `test_begin`, `test_pass`, `test_fail`, `test_skip`, `assert_success`,
`assert_exit_code`, `assert_json_array_min_length` in `tests/functional/lib.sh`;
`container_runtime` and `bugzilla_container_name` in `tests/functional/container-env.sh`,
which `lib.sh` sources at its line 7, so a phase sees them without sourcing anything itself.
`SCRIPT_DIR`, `ADMIN_EMAIL` and `BZ_URL` are globals the orchestrator
`tests/functional/run-tests.sh` sets before sourcing any phase.
The Perl seeder `tests/functional/compare/seed-saved-search.pl` takes
`LOGIN NAME QUERY` on argv and is read from stdin by `perl -I. -`.

**Verification**

- Contract: `bug search --saved-search` is accepted by a real Bugzilla over REST and over
  XML-RPC, composes with `--count`, works credentiallessly, and rejects its four invalid
  argument combinations. Mode: focused-test. Test:
  `tests/functional/phases/08f-bug-saved-search.sh`. Expected red: before the phase is added
  to the runner's list, `make functional-test` never runs it and
  `make check-functional-test-ids` reports the file as an unsourced phase. Green:
  `make functional-test` reports every `bug-search-saved-search-*` id as PASS.

**Steps**

1. Create `tests/functional/phases/08f-bug-saved-search.sh` with a header comment in the
   style of `08b-bugs-paging.sh`, stating plainly what the phase can and cannot prove:

   ```bash
   # 08f-bug-saved-search
   # Sourced by run-tests.sh in order; assumes lib.sh helpers and the
   # orchestrator preamble (constants, shared globals, cleanup trap).
   # Reads: ADMIN_EMAIL, BZ_URL. Creates: one marker-isolated bug and one
   # server-side saved search naming it.
   # shellcheck shell=bash
   #
   # Exercises `bug search --saved-search` / `--sharer` against a real
   # container. What this proves: a real Bugzilla accepts both parameters
   # over REST and XML-RPC, they compose with --count, the credentialless
   # path works, and the four invalid argument combinations are rejected at
   # parse time. What it deliberately does NOT prove: that the server
   # resolved the saved search. Resolving `savedsearch` is a Red Hat
   # Bugzilla extension; every supported image here accepts the parameter
   # and ignores it, so no assertion over the returned rows could
   # distinguish a resolved search from an unfiltered one. The wire mapping
   # is proven instead by the wiremock tests in
   # src/client/resources/bug_tests.rs and src/xmlrpc/resources/bug_tests.rs.
   ```

2. Below the header, print the phase banner in the style the other phases use, then set up
   the fixture. Use 4-space indentation throughout:

   ```bash
   echo "── Phase 8f: Bug search --saved-search ─────────────────────"

   _SS_MARK="ssmark$$x${RANDOM}"
   _SS_NAME="saved-search-$$-${RANDOM}"
   _SS_BUG=$(make_bug --marker "$_SS_MARK" --product FuncTestProd --component Backend \
       --op-sys Linux --platform PC --description d --summary "saved search target")
   _SS_SEEDED=0
   if [[ -n $_SS_BUG ]]; then
       _SS_RUNTIME=$(container_runtime) || _SS_RUNTIME=""
       _SS_CONTAINER=$(bugzilla_container_name) || _SS_CONTAINER=""
       if [[ -n $_SS_RUNTIME && -n $_SS_CONTAINER ]] &&
           "$_SS_RUNTIME" exec -i --workdir /var/www/html/bugzilla "$_SS_CONTAINER" \
               perl -I. - "$ADMIN_EMAIL" "$_SS_NAME" \
               "bug_id=${_SS_BUG}&bug_id_type=anyexact" \
               <"$SCRIPT_DIR/compare/seed-saved-search.pl"; then
           _SS_SEEDED=1
       fi
   fi
   ```

3. Add the acceptance tests. Each is guarded on the fixture so a seeding failure skips rather
   than reports a false failure:

   ```bash
   test_begin "bug-search-saved-search-rest" "bug search --saved-search over REST"
   if [[ $_SS_SEEDED -eq 1 ]]; then
       run_bzr --api rest bug search --saved-search "$_SS_NAME"
       if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
   else test_skip "saved search not seeded"; fi

   test_begin "bug-search-saved-search-xmlrpc" "bug search --saved-search over XML-RPC"
   if [[ $_SS_SEEDED -eq 1 ]]; then
       run_bzr --api xmlrpc bug search --saved-search "$_SS_NAME"
       if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
   else test_skip "saved search not seeded"; fi

   test_begin "bug-search-saved-search-count" "bug search --saved-search composes with --count"
   if [[ $_SS_SEEDED -eq 1 ]]; then
       run_bzr bug search --saved-search "$_SS_NAME" --count
       if assert_success; then test_pass; fi
   else test_skip "saved search not seeded"; fi

   test_begin "credentialless-bug-search-saved-search" "credentialless bug search --saved-search"
   if [[ $_SS_SEEDED -eq 1 ]]; then
       run_bzr_raw --json --server-url "$BZ_URL" bug search --saved-search "$_SS_NAME"
       if assert_success; then test_pass; fi
   else test_skip "saved search not seeded"; fi
   ```

4. Add a `--sharer` acceptance test. Resolve the admin's numeric user ID from
   `bzr --json whoami`, whose payload has a required `id` field (see `schemas/whoami.json`):

   ```bash
   _SS_SHARER=""
   run_bzr whoami
   if [[ $BZR_EXIT -eq 0 ]]; then
       _SS_SHARER=$(jq -r '.id // empty' "$BZR_STDOUT" 2>/dev/null || true)
   fi

   test_begin "bug-search-saved-search-sharer" "bug search --saved-search --sharer"
   if [[ $_SS_SEEDED -eq 1 && -n $_SS_SHARER ]]; then
       run_bzr bug search --saved-search "$_SS_NAME" --sharer "$_SS_SHARER"
       if assert_success; then test_pass; fi
   else test_skip "saved search not seeded or sharer ID unavailable"; fi
   ```

5. Add the four rejection tests. These need no fixture:

   ```bash
   test_begin "bug-search-saved-search-rejects-query" "bug search rejects --saved-search with a query"
   run_bzr bug search "some text" --saved-search "$_SS_NAME"
   if assert_exit_code 2; then test_pass; fi

   test_begin "bug-search-saved-search-rejects-from-url" "bug search rejects --saved-search with --from-url"
   run_bzr bug search --from-url "${BZ_URL}/buglist.cgi?bug_id=1" --saved-search "$_SS_NAME"
   if assert_exit_code 2; then test_pass; fi

   test_begin "bug-search-sharer-requires-saved-search" "bug search --sharer requires --saved-search"
   run_bzr bug search "some text" --sharer 1
   if assert_exit_code 2; then test_pass; fi

   test_begin "bug-search-sharer-rejects-non-numeric" "bug search --sharer rejects a non-numeric ID"
   run_bzr bug search --saved-search "$_SS_NAME" --sharer not-a-number
   if assert_exit_code 2; then test_pass; fi
   ```

6. In `tests/functional/run-tests.sh`, add `08f-bug-saved-search` to the `for _phase in \`
   list, immediately after `08e-bugs-restricted-access` and before
   `09-bug-relationships`.

7. Run `make lint` bare. Expect exit 0; `check-functional-test-ids` and `check-shell` both
   cover the new file.

8. Run `make functional-test` bare, in the background, and read its result once when it
   completes. It takes roughly 10 minutes. Expect every `bug-search-saved-search-*` and
   `credentialless-bug-search-saved-search` id to report PASS and the run to exit 0.

9. Commit: `test(search): cover --saved-search against a real container`.

**Acceptance criteria**

- `tests/functional/phases/08f-bug-saved-search.sh` exists and is sourced by the runner.
- `make lint` is green, including `check-functional-test-ids` and `check-shell`.
- `make functional-test` exits 0 with every new test id passing.
- The phase's header states what it cannot prove and why.

## Rollback and cleanup

Every task is additive to existing files plus one new phase script; reverting the branch
restores the previous behaviour with no data or configuration migration. The phase script
creates one bug and one `namedqueries` row per run inside the disposable functional
container, which the harness discards with the container — matching how every other phase
seeds fixtures.

## Deferrals carried from review

None recorded yet. Any deferral a review of this design produces is appended here with its
owning record path or tracker issue before the build begins.
