# Implement operation-scoped search transport

Issue: [#611](https://github.com/randomparity/bzr/issues/611)

Spec: [Operation-scoped search transport design](../specs/2026-08-31-operation-scoped-search-transport-design.md)

Decision: [ADR 0032](../../adr/0032-operation-scoped-search-transport.md)

## Goal

Emit the raw-parameter REST-fallback warning at most once per CLI search operation while every
page continues to use REST and existing output, progress, validation, and error behavior remains
unchanged.

## Architecture

The bug client owns a crate-private mutable `BugSearch<'a>` handle. It resolves configured versus
forced-REST transport once, preserves required-ID normalization on every execution, and consumes a
pending warning immediately before the first forced-REST request. Paging begins one handle for
each fetch path and reuses it for every page; the command layer does not gain REST request logic.

## Tech stack

Rust 2021, Tokio, reqwest, tracing, wiremock, and the existing Bash real-container functional
harness. No dependency changes.

## Global constraints

- Preserve compatibility with all declared release targets:
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `powerpc64le-unknown-linux-gnu`, `s390x-unknown-linux-gnu`,
  `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, and
  `aarch64-pc-windows-msvc`.
- Add no dependency and no new public CLI or configuration contract.
- Keep CLI output paths unchanged: result data through existing writers, diagnostics through
  tracing, and progress through existing progress helpers.
- Keep test modules in sibling `*_tests.rs` files.
- Functional tests use semantic phase-local IDs and the existing runner helpers.
- Keep changes within `src/client/resources/bug.rs`, its sibling tests if needed,
  `src/commands/runtime/search/`, its sibling tests, and `tests/functional/` phase coverage.
- Run final guardrails exactly as `make lint`, `make test`, and `make functional-test-all`.
- Do not edit `docs/adr/README.md`; its ADR 0032 row is campaign-owned and remains pending.

## File map

- `src/client/resources/bug.rs` — owns `BugSearch<'a>`, one-time transport selection, lazy warning
  state, required-ID normalization, and the compatibility `search_bugs` wrapper.
- `src/commands/runtime/search/paging.rs` — begins one operation handle per fetch path and reuses
  it for all paginated requests.
- `src/commands/runtime/search/paging_tests.rs` — proves multi-page forced REST, one warning,
  required-ID normalization, explicit-REST silence, and warning-free pre-request validation.
- `tests/functional/phases/18f-project-manager-reporting.sh` — proves the real CLI behavior against
  Bugzilla with forced XML-RPC configuration and explicit REST configuration.

## Task 1: Select transport and warn once across paging

### Interfaces

Consumes existing:

```rust
fn force_id_fields(
    include: Option<&str>,
    exclude: Option<&str>,
) -> (Option<String>, Option<String>);

impl BugzillaClient {
    async fn search_bugs_rest(&self, params: &SearchParams) -> Result<Vec<Bug>>;
    pub(crate) async fn search_bugs_hybrid(
        &self,
        params: &SearchParams,
        fallback_timeout: Duration,
    ) -> Result<Vec<Bug>>;
}
```

Creates for paging:

```rust
pub(crate) struct BugSearch<'a> {
    client: &'a BugzillaClient,
    force_rest: bool,
    warning_pending: bool,
}

impl BugzillaClient {
    pub(crate) fn begin_bug_search(&self, params: &SearchParams) -> BugSearch<'_>;
}

impl BugSearch<'_> {
    pub(crate) async fn execute(&mut self, params: &SearchParams) -> Result<Vec<Bug>>;
}
```

`BugzillaClient::search_bugs` remains callable with its current signature and becomes a
one-request compatibility wrapper around these interfaces. `paging.rs` relies on the inferred
return type and does not need to import or re-export `BugSearch`, keeping the change inside the
assigned file surface.

### Steps

1. In `src/commands/runtime/search/paging_tests.rs`, extend the test-helper import to include
   `test_client_hybrid`. Add a raw-parameter constructor:

   ```rust
   fn raw_params_with_limit(limit: u32) -> SearchParams {
       SearchParams {
           limit: Some(limit),
           include_fields: Some("summary".into()),
           exclude_fields: Some("id".into()),
           raw_params: vec![
               ("f1".into(), "status_whiteboard".into()),
               ("o1".into(), "substring".into()),
               ("v1".into(), "marker".into()),
           ],
           ..Default::default()
       }
   }
   ```

2. In the same sibling test file, add a multi-page regression. Mount exactly three one-shot REST
   responses with explicit bodies: offset `0` returns bugs with IDs `[1, 2]`, offset `2` returns a
   bug with ID `[3]`, and offset `3` returns an empty array. Do not use `bugs_body(n)` for the
   second page because that helper restarts IDs at one. Each mock must require `limit=2`,
   `f1=status_whiteboard`, `o1=substring`, `v1=marker`, and `include_fields=id,summary`, and each
   must use `.expect(1)`. Use the Hybrid test client and install
   `TracingCapture::install(tracing::Level::WARN)`. Execute:

   ```rust
   let bugs = fetch_all_pages_with_cap(
       &client,
       &raw_params_with_limit(2),
       3,
       None,
       &mut crate::test_helpers::CapturedIo::new().writers(),
   )
   .await
   .unwrap();
   ```

   Assert IDs are `[1, 2, 3]`. Inspect `mock.received_requests()` and assert every request is GET
   `/rest/bug`, no request carries `exclude_fields`, and exactly three requests occurred. Count
   occurrences of `query contains raw URL parameters that require REST API` in the tracing capture
   and assert exactly one.

3. Add an explicit-REST test with one unbounded raw-parameter request. Install the WARN capture,
   call `fetch_page` with `test_client`, `paginate=true`, and `limit=0`, and assert the capture does
   not contain the fallback warning. The mock must expect the raw `f1/o1/v1` values and
   `include_fields=id,summary`.

4. Rename `fetch_page_rejects_limit_that_cannot_overfetch` to
   `fetch_page_raw_params_rejects_limit_that_cannot_overfetch`, then use the Hybrid client and
   `raw_params_with_limit(u32::MAX)`. Install a WARN capture and retain the current no-request
   assertion. Add an assertion that the fallback warning is absent, proving local validation still
   precedes the first diagnostic/request. The `raw_params` substring keeps this case inside the
   focused red/green selector below.

5. Run the focused tests before production edits:

   ```sh
   make test-one T=raw_params
   ```

   Expected: non-zero. The multi-page regression reports more than one warning because current
   `search_bugs` warns inside every loop iteration. Record that red result in the forge ledger.

6. In `src/client/resources/bug.rs`, place this type before the `BugzillaClient` resource impl:

   ```rust
   pub(crate) struct BugSearch<'a> {
       client: &'a BugzillaClient,
       force_rest: bool,
       warning_pending: bool,
   }
   ```

7. Replace the current `search_bugs` body with a factory, compatibility wrapper, configured
   dispatch helper, and handle execution. Preserve the existing warning text byte-for-byte:

   ```rust
   impl BugzillaClient {
       pub(crate) fn begin_bug_search(&self, params: &SearchParams) -> BugSearch<'_> {
           let force_rest = !params.raw_params.is_empty() && self.api_mode != ApiMode::Rest;
           BugSearch {
               client: self,
               force_rest,
               warning_pending: force_rest,
           }
       }

       pub async fn search_bugs(&self, params: &SearchParams) -> Result<Vec<Bug>> {
           let mut search = self.begin_bug_search(params);
           search.execute(params).await
       }

       async fn search_bugs_configured(&self, params: &SearchParams) -> Result<Vec<Bug>> {
           match self.api_mode {
               ApiMode::Rest => self.search_bugs_rest(params).await,
               ApiMode::XmlRpc => self.xmlrpc_client().search_bugs(params).await,
               ApiMode::Hybrid => {
                   self.search_bugs_hybrid(params, XMLRPC_FALLBACK_TIMEOUT)
                       .await
               }
           }
       }
   }

   impl BugSearch<'_> {
       pub(crate) async fn execute(&mut self, params: &SearchParams) -> Result<Vec<Bug>> {
           tracing::debug!(?params, %self.client.api_mode, "search parameters");
           let (inc, exc) = force_id_fields(
               params.include_fields.as_deref(),
               params.exclude_fields.as_deref(),
           );
           let normalized = (inc != params.include_fields || exc != params.exclude_fields)
               .then(|| SearchParams {
                   include_fields: inc,
                   exclude_fields: exc,
                   ..params.clone()
               });
           let params = normalized.as_ref().unwrap_or(params);

           if self.warning_pending {
               tracing::warn!(
                   "query contains raw URL parameters that require REST API; \
                    ignoring configured {} mode",
                   self.client.api_mode
               );
               self.warning_pending = false;
           }

           if self.force_rest {
               self.client.search_bugs_rest(params).await
           } else {
               self.client.search_bugs_configured(params).await
           }
       }
   }
   ```

   Remove the old inline normalization, raw-parameter warning branch, and configured-mode match
   from `search_bugs`; their behavior now lives in the handle.

8. In `src/commands/runtime/search/paging.rs`, replace each per-request
   `client.search_bugs(...)` call with a handle reused for that fetch path:

   - After the `paginate` branch in `fetch_page`, create
     `let mut search = client.begin_bug_search(params);`. Use `search.execute(params)` for the
     unbounded path and `search.execute(&probe)` after both over-fetch validations.
   - At the start of `fetch_all_pages_with_cap`, create
     `let mut search = client.begin_bug_search(params);`. Use `search.execute(params)` for the
     unbounded path and `search.execute(&p)` inside the loop.
   - Do not move or alter limit/offset validation, page accumulation, progress calls, termination,
     or error construction.

9. Re-run the focused tests:

   ```sh
   make test-one T=raw_params
   ```

   Expected: exit 0. The multi-page test returns all three IDs from offsets `0`, `2`, and `3`; all
   three requests are REST; the forced-REST capture contains one warning; explicit REST contains
   none; and the renamed pre-request validation case runs under this selector and remains
   warning-free.

10. Run the focused paging and existing client regressions:

    ```sh
    make test-one T=fetch_page
    make test-one T=hybrid_search_bugs_with_raw_params
    make test-one T=search_bugs_prepends_id
    ```

    Expected: each exits 0. Existing pagination/progress/error tests and the public compatibility
    wrapper retain their behavior.

11. Run `make lint`. Expected: exit 0 with no formatting, clippy, test-layout, semantic-ID,
    concurrency, release-note, or shell findings.

12. Commit only the three task files:

    ```sh
    git add src/client/resources/bug.rs \
      src/commands/runtime/search/paging.rs \
      src/commands/runtime/search/paging_tests.rs
    git commit -m "fix(search): warn once for raw-parameter pagination"
    ```

### Acceptance criteria

- One operation handle owns one transport decision and one pending warning.
- Every raw-parameter page uses REST in Hybrid/XML-RPC configured modes.
- Required `id` survives include/exclude projection normalization on every handle execution.
- Explicit REST emits no fallback warning.
- Limit/offset validation before the first request remains warning-free.
- Existing result, progress, and error tests remain green.

### Rollback

Revert the task commit. The compatibility wrapper and paging calls return to per-request dispatch;
no state cleanup is required.

## Task 2: Prove the CLI contract against real Bugzilla

### Interfaces

Consumes the Task 1 CLI behavior and existing functional helpers:

```bash
run_bzr
run_bzr_raw
assert_success
assert_json_array_length
assert_json
assert_ndjson_line_count
assert_stderr_not_contains
```

No new helper is introduced. The warning count uses one local `awk` expression so grep exit status
cannot confuse zero matches with a harness failure.

### Steps

1. In `tests/functional/phases/18f-project-manager-reporting.sh`, extend
   `pm-custom-search-saves-and-paginates-projected-json`. Run the saved raw-parameter query with
   configured XML-RPC, a one-row page, and an output projection that omits `id`:

   ```bash
   RUST_LOG=bzr=warn run_bzr --api xmlrpc query run "$_PM_QUERY" \
     --fields summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
     --limit 1 --paginate
   ```

   Retain the three-result and summary/whiteboard assertions. Replace the first-row shape assertion
   with:

   ```bash
   assert_json '.[0] | (has("id") | not) and has("summary") and has("status") and has("whiteboard")' "true"
   ```

   After those assertions pass, count the exact fallback-warning prefix:

   ```bash
   _PM_REST_WARNING_COUNT=$(awk '
     index($0, "query contains raw URL parameters that require REST API") { count++ }
     END { print count + 0 }
   ' "$BZR_STDERR")
   ```

   Call `test_pass` only when the count is `1`; otherwise call
   `test_fail "raw-parameter REST fallback warning count = $_PM_REST_WARNING_COUNT, expected 1"`.

2. Extend `pm-custom-search-emits-bare-projected-ndjson-rows` to run with explicit REST and multiple
   pages:

   ```bash
   RUST_LOG=bzr=warn run_bzr_raw --api rest --output ndjson bug search --from-url "$_PM_URL" \
     --fields id,summary,status,assigned_to,target_milestone,last_change_time,whiteboard \
     --limit 1 --paginate
   ```

   Prefixing both invocations with `RUST_LOG=bzr=warn` makes their presence/absence assertions
   independent of an ambient operator `RUST_LOG` override. Retain the current NDJSON row/order
   assertions and add
   `assert_stderr_not_contains "query contains raw URL parameters that require REST API"` before
   `test_pass`.

3. Add `_PM_REST_WARNING_COUNT` to the final unset list.

4. Run the semantic-ID and shell guards:

   ```sh
   make check-functional-test-ids
   make check-shell
   ```

   Expected: both exit 0. The existing test IDs remain stable and the edited Bash passes syntax,
   ShellCheck, and shfmt.

5. Run the complete real-container proof:

   ```sh
   make functional-test-all
   ```

   Expected: exit 0 for Bugzilla 5.0, 5.2, and 5.3. The projected JSON test reports three rows and
   one fallback warning despite four REST requests (three data pages plus the empty terminal page),
   and the explicit-REST NDJSON test reports three rows with no fallback warning.

6. Run the branch guardrails:

   ```sh
   make lint
   make test
   ```

   Expected: both exit 0. Record observed durations in the quest handoff.

7. Commit only the functional phase:

   ```sh
   git add tests/functional/phases/18f-project-manager-reporting.sh
   git commit -m "test(functional): cover raw-parameter warning lifetime"
   ```

### Acceptance criteria

- A real multi-page XML-RPC-configured invocation returns all three projected results and emits the
  fallback warning exactly once.
- The projection omits `id` from stdout while internal required-ID fetching still permits
  deserialization.
- A real explicit-REST multi-page invocation emits no fallback warning.
- All supported Bugzilla versions pass the full functional suite.

### Rollback

Revert the functional-test commit after reverting Task 1. No fixture, persisted query, or container
state survives the phase cleanup.

## Final self-review matrix

| Spec requirement | Implemented/proved by |
|---|---|
| One warning maximum per invocation | Task 1 tracing count; Task 2 XML-RPC-configured CLI count |
| Every page uses REST | Task 1 REST-only wiremock requests; Task 2 raw query succeeds under XML-RPC configuration |
| Results unchanged | Task 1 ordered IDs; Task 2 exact three-row projections |
| Structured stdout unchanged | Task 2 JSON/NDJSON shape assertions and no diagnostic on stdout |
| Progress unchanged | Existing `fetch_page` progress regressions run in Task 1 and full suite |
| Errors unchanged | Task 1 warning-free pre-request validation plus existing overflow/cap tests |
| Explicit REST is silent | Task 1 capture and Task 2 explicit-REST stderr assertion |
| Real-container coverage | Task 2 `make functional-test-all` |

No placeholder, migration, dependency, public interface, security-boundary, or cleanup work remains.
