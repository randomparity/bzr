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

Expected implementation size: 300–420 changed lines (M) — summed from the file map below.
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
| `docs/dev/python-bugzilla-parity.md` | changed | the saved-search status row |
| `tests/functional/compare/01-bug-lifecycle.sh` | changed | dropping the expected-gap marking |
| `tests/functional/phases/08f-bug-saved-search.sh` | created | functional coverage on a real container |
| `tests/functional/run-tests.sh` | changed | sourcing the new phase |

## Task 1 — parameter fields and transport mapping

**Interfaces.** Consumes nothing. Later tasks rely on two fields added to the existing
`pub struct SearchParams` (which is `#[derive(Clone, Debug, Default)]` and
`#[non_exhaustive]`, so every `..Default::default()` construction site keeps compiling):

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
  Red: same compile failure. Green:
  `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`.
- Contract: a saved-search name alone is a filter for `has_filters` and is not one for
  `has_structured_filters`. Mode: focused-test. Test: `src/types/bug/search_tests.rs`,
  `saved_search_is_a_filter_but_not_a_structured_filter`. Red: same compile failure. Green:
  `make test-one T=saved_search_is_a_filter_but_not_a_structured_filter`.

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

4. In the same file add `|| self.saved_search.is_some()` to `has_filters`. Leave
   `has_structured_filters` alone and append one sentence to its doc comment: `saved_search`
   is excluded for the same reason as `quicksearch` — it is resolved by one server-side sub
   shared by both transports, so an empty REST result is authoritative and a retry would
   return the same rows.

5. In `src/client/resources/bug.rs`, add `("savedsearch", &params.saved_search)` to the
   `option_fields` slice in `append_option_params`, after the `quicksearch` entry; and after
   that function's `offset` block add:

   ```rust
   if let Some(sharer_id) = params.sharer_id {
       builder = builder.query(&[("sharer_id", sharer_id)]);
   }
   ```

6. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id`. Expect one pass.

7. In `src/xmlrpc/resources/bug_tests.rs`, add
   `search_bugs_sends_saved_search_and_sharer_id_xmlrpc`, reusing the mock-server setup and
   request-body capture of the file's existing `search_bugs_returns_results` verbatim, and
   asserting the recorded body contains `savedsearch` with the name and `sharer_id` with the
   integer.

8. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expect a
   failure showing both absent from the call body.

9. In `src/xmlrpc/resources/bug.rs` `search_bugs`, add
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

10. Run `make test-one T=search_bugs_sends_saved_search_and_sharer_id_xmlrpc`. Expect one
    pass.

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

12. Run `make test-one T=saved_search_is_a_filter_but_not_a_structured_filter`, then
    `make lint` bare. Expect exit 0 from both. Commit:
    `feat(search): carry saved-search parameters on both transports`.

**Acceptance criteria.** Both transports emit each parameter when set and omit it when
`None`; `has_filters()` is true and `has_structured_filters()` false for a
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
  `sharer_requires_saved_search`, `sharer_rejects_non_numeric`. Red: the parser accepts each
  combination, so every `assert_eq!` on the error kind fails. Green:
  `make test-one T=saved_search` and `make test-one T=sharer`.
- Contract: a `--saved-search` invocation puts `savedsearch` and `sharer_id` on the outgoing
  request and no `quicksearch`. Mode: focused-test. Test: `src/commands/bug/search_tests.rs`,
  `handle_search_saved_search_passes_saved_search_and_sharer`. Red: unknown field on
  `SearchArgs`, compile failure. Green:
  `make test-one T=handle_search_saved_search_passes_saved_search_and_sharer`.
- Contract: `bug search` with no query source fails input validation naming all three
  sources, before connecting. Mode: focused-test. Test:
  `src/commands/bug/search_tests.rs`, `handle_search_without_a_query_source_names_all_three`.
  Red: the message names only two. Green:
  `make test-one T=handle_search_without_a_query_source_names_all_three`.

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

2. Run `make test-one T=saved_search` and `make test-one T=sharer`. Expect compile failure
   naming the unknown arguments.

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

4. In the same file extend `LONG_ABOUT`: after the `--save-as` paragraph insert a paragraph
   saying that `--saved-search <NAME>` runs a saved search stored in the Bugzilla account,
   optionally qualified by `--sharer <ID>` when another user shared it; that resolving one is
   a Red Hat Bugzilla extension, so a stock Bugzilla accepts both parameters and ignores
   them, returning an unfiltered result; and that these are unrelated to bzr's local saved
   queries, which `bzr query` manages. Add one line to the `Examples:` block:
   `bzr bug search --saved-search "my triage list" --sharer 112233`.

5. Run `make test-one T=saved_search` and `make test-one T=sharer`. Expect four passes.

6. In `src/commands/bug/search.rs`, replace the query-source resolution inside the
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

7. `SearchArgs` derives `Args` and `Debug` but **not** `Default`, so every struct-literal
   construction must gain `saved_search: None,` and `sharer: None,` beside `save_as` or the
   crate will not compile. There are six such sites: `src/commands/bug/search_tests.rs` lines
   21, 359, 526, 595 and `src/commands/bug/mod_tests.rs` lines 32, 175. The other mentions in
   those files and in `src/cli/mod_tests.rs` are destructuring patterns ending in `..` and
   need no change. Run `cargo build --tests` and fix anything this list missed rather than
   trusting it to be exhaustive.

8. Add the two command-level tests to `src/commands/bug/search_tests.rs`, modelled on that
   file's existing `handle_search_quicksearch_passes_limit_and_field_filters` — same
   `setup_test_env().await` fixture, same `Mock::given(method("GET")).and(path("/rest/bug"))`
   shape, same `crate::commands::bug::execute(&action, &CommandContext::new(None,
   OutputFormat::Json, None), &mut io.writers())` call:

   - `handle_search_saved_search_passes_saved_search_and_sharer` — action with `query: None`,
     `saved_search: Some("my search".into())`, `sharer: Some(112_233)`; match on
     `query_param("savedsearch", "my search")` and `query_param("sharer_id", "112233")`;
     assert `Ok`.
   - `handle_search_without_a_query_source_names_all_three` — action with `query`, `from_url`
     and `saved_search` all `None`; mount **no** `Mock` at all, which is what proves the
     error precedes the connection; assert `Err` and that the rendered message contains
     `--saved-search`.

9. Run both focused tests, then `make lint` bare, then `make test` bare in the background and
   read the result once. Expect exit 0. Commit:
   `feat(search): add --saved-search and --sharer flags`.

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
  gap. Mode: task-test-not-applicable. Changed surface: two lines of a Bash comparison script
  that only executes inside a container-backed comparison run; no executable or structural
  observation available in this task can fail meaningfully on it. The observation that can is
  the comparison run itself, which Task 4's functional run performs.

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
   `--saved-search` is a third, mutually exclusive query source: it runs a saved search stored in your Bugzilla account, which is unrelated to bzr's local saved queries (see [`bzr query`](#bzr-query)).

   > **Note:** Resolving a server-side saved search is a Red Hat Bugzilla extension. A stock Bugzilla accepts `savedsearch` and `sharer_id` and ignores them, so `--saved-search` against such a server returns an unfiltered result rather than an error. Verified against Bugzilla 5.0.6, 5.2, and 5.3.3+.
   ```

5. In that section's options table, add after the `--save-as [NAME]` row:

   ```markdown
   | `--saved-search <NAME>` | No* | | Run a saved search stored on the server (Bugzilla `savedsearch`). Mutually exclusive with `<QUERY>` and `--from-url`. Resolving it requires a Bugzilla with the Red Hat saved-search extension; a stock server ignores the parameter. |
   | `--sharer <ID>` | No | | Numeric Bugzilla user ID of the account that shared the saved search (Bugzilla `sharer_id`). Requires `--saved-search`. |
   ```

   and change the trailing footnote to
   `*One of \`<QUERY>\`, \`--saved-search\`, or \`--from-url\` must be provided.`

6. In `docs/dev/python-bugzilla-parity.md`, change the `Server saved search` row's Status
   cell from `expected gap (#670)` to `parity`. Change nothing else in that table.

7. In the `saved-search` block of `tests/functional/compare/01-bug-lifecycle.sh`, replace

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

8. Run `make lint` bare — `check-shell` covers the comparison script. Expect exit 0. Commit:
   `docs(search): document --saved-search and flip the parity row`.

**Acceptance criteria.** The flag-drift check exits 0; the reference documents both flags,
states the Red Hat caveat, and its footnote names all three sources; the parity row reads
`parity`; the comparison block calls `lifecycle_bzr` with no `lifecycle_expect_gap 670`.

## Task 4 — functional phase coverage

**Interfaces.** Consumes Task 2's flags and Task 3's documentation state. Helpers, each
confirmed at the path named: `run_bzr`, `run_bzr_raw`, `make_bug`, `test_begin`, `test_pass`,
`test_skip`, `assert_success`, `assert_exit_code`, `assert_json_array_min_length` in
`tests/functional/lib.sh`; `container_runtime` and `bugzilla_container_name` in
`tests/functional/container-env.sh`, which `lib.sh` sources at line 7, so a phase sees them
without sourcing anything. `SCRIPT_DIR`, `ADMIN_EMAIL` and `BZ_URL` are globals
`tests/functional/run-tests.sh` sets before sourcing any phase. The seeder
`tests/functional/compare/seed-saved-search.pl` takes `LOGIN NAME QUERY` on argv and is read
from stdin by `perl -I. -`.

**Verification**

- Contract: `bug search --saved-search` is accepted by a real Bugzilla over REST and
  XML-RPC, composes with `--count`, works credentiallessly, and rejects its four invalid
  argument combinations. Mode: focused-test. Test:
  `tests/functional/phases/08f-bug-saved-search.sh`. Red: before the phase is added to the
  runner's list `make functional-test` never runs it. Green: `make functional-test` reports
  every `bug-search-saved-search-*` id as PASS.

**Steps**

1. Create `tests/functional/phases/08f-bug-saved-search.sh` with a header in the style of
   `08b-bugs-paging.sh`. The header must state plainly what the phase cannot prove:

   ```bash
   # 08f-bug-saved-search
   # Sourced by run-tests.sh in order; assumes lib.sh helpers and the
   # orchestrator preamble (constants, shared globals, cleanup trap).
   # Reads: ADMIN_EMAIL, BZ_URL, SCRIPT_DIR. Creates: one marker-isolated
   # bug and one server-side saved search naming it.
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

2. Print the phase banner (`echo "── Phase 8f: Bug search --saved-search ───────────────"`),
   then build the fixture with 4-space indentation:

   ```bash
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

   _SS_SHARER=""
   run_bzr whoami
   if [[ $BZR_EXIT -eq 0 ]]; then
       _SS_SHARER=$(jq -r '.id // empty' "$BZR_STDOUT" 2>/dev/null || true)
   fi
   ```

   `bzr --json whoami` has a required `id` field (`schemas/whoami.json`), and lib.sh's
   envelope projection puts the payload at `$BZR_STDOUT`, so `.id` reads directly.

3. Add five acceptance tests. Each follows the same shape as the first, guarded so a seeding
   failure skips rather than reporting a false failure:

   ```bash
   test_begin "bug-search-saved-search-rest" "bug search --saved-search over REST"
   if [[ $_SS_SEEDED -eq 1 ]]; then
       run_bzr --api rest bug search --saved-search "$_SS_NAME"
       if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
   else test_skip "saved search not seeded"; fi
   ```

   The remaining four, same guard and same `test_skip` message unless noted:

   - `bug-search-saved-search-xmlrpc` / "bug search --saved-search over XML-RPC" —
     `run_bzr --api xmlrpc bug search --saved-search "$_SS_NAME"`, then
     `assert_success && assert_json_array_min_length '.' 1`.
   - `bug-search-saved-search-count` / "bug search --saved-search composes with --count" —
     `run_bzr bug search --saved-search "$_SS_NAME" --count`, then `assert_success`.
   - `credentialless-bug-search-saved-search` / "credentialless bug search --saved-search" —
     `run_bzr_raw --json --server-url "$BZ_URL" bug search --saved-search "$_SS_NAME"`, then
     `assert_success`.
   - `bug-search-saved-search-sharer` / "bug search --saved-search --sharer" — guard
     `[[ $_SS_SEEDED -eq 1 && -n $_SS_SHARER ]]`, skip message
     `"saved search not seeded or sharer ID unavailable"`, run
     `run_bzr bug search --saved-search "$_SS_NAME" --sharer "$_SS_SHARER"`, then
     `assert_success`.

4. Add the four rejection tests, which need no fixture and no guard. Each runs the command
   and asserts `assert_exit_code 2`:

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
   roughly 10 minutes. Expect exit 0 with every new id reporting PASS.

8. Commit: `test(search): cover --saved-search against a real container`.

**Acceptance criteria.** The phase exists, is sourced by the runner, and its header states
what it cannot prove; `make lint` green including `check-functional-test-ids` and
`check-shell`; `make functional-test` exits 0 with every new id passing.

## Rollback and cleanup

Every task is additive plus one new phase script; reverting the branch restores the previous
behaviour with no data or configuration migration. The phase creates one bug and one
`namedqueries` row per run inside the disposable functional container, discarded with it, as
every other phase seeds its fixtures.

## Deferrals carried from review

None recorded yet. Any deferral a review of this design produces is appended here with its
owning record path or tracker issue before the build begins.
