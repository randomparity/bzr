# Inline URL Server Warning Implementation Plan

**Goal:** make `bug search --from-url` diagnostics agree with the explicit inline request
destination and prove the stateless behavior against real Bugzilla containers.

**Architecture:** URL parsing receives optional active-server context but continues to own imported
URL validation, sanitization, and configured-server matching. The bug-search command supplies its
inline URL from `CommandContext`; shared connection setup remains the sole routing owner.

**Tech stack:** Rust 1.89.0, `url`, Tokio tests with wiremock, Bash functional harness, Docker or
Podman Bugzilla containers.

## Global constraints

- Authority is issue #593 under scope token `q593-8c64ca6a`; issue #593 governs matching and
  mismatch behavior, and the campaign request governs production fidelity plus all-version
  functional proof. Accepted ADR 0027 governs hostname-only comparison and separation of saved
  configured-server metadata from the inline request destination.
- Permitted surface is the URL parser, bug-search caller, direct signature callers, sibling unit
  tests, functional tests, ADR 0027, and direct design dependencies.
- Excluded are credential stripping/sanitization changes, unrelated server-resolution behavior,
  merging, the ADR index, and the campaign manifest. The campaign orchestrator owns the latter
  three external workflow actions/surfaces.
- Host equality uses `url::Url` normalized hostnames; schemes and ports do not participate.
- Credential stripping, URL sanitization, named/default resolution, TLS, and routing remain intact.
- User-facing output uses tracing/Writers conventions; tests live in sibling `_tests.rs` files.
- Branch `feat/inline-url-server-warning-593`; base `main`.
- Guardrails are `make test-one T=<name-substring>`, `make test-fast`, `make lint`, `make test`, and
  `make functional-test-all`.

## Task 1: Make parser diagnostics inline-aware

**Files:** modify `src/commands/runtime/input/url_parser.rs`,
`src/commands/runtime/input/url_parser_tests.rs`, `src/commands/bug/search.rs`, and
`src/commands/bug/search_tests.rs`; update direct signature callers in
`src/commands/query/mod.rs` and the fuzz wrapper in `src/lib.rs`; add the matching regression case
to `tests/functional/phases/08-bugs.sh`. The fuzz target itself remains unchanged because it calls
the public wrapper in `src/lib.rs`, not the internal parser.

**Interfaces:** change
`parse_bugzilla_url(url_str: &str, config: &Config) -> Result<ParsedUrl>` to
`parse_bugzilla_url(url_str: &str, config: &Config, active_server_url: Option<&str>) -> Result<ParsedUrl>`.
`search::resolve_client_and_params` supplies
`ctx.inline_server().map(|server| server.url.as_str())`. Query and fuzz callers supply `None` as a
direct compilation dependency. Later functional coverage relies on matching inline hostname with
`query.server == None` and successful connection through `CommandContext`.

1. Add the matching-host functional case after test 35b. Set
   `_INLINE_SEARCH_CONFIG="$FUNC_CONFIG_DIR/inline-search-empty.toml"` without creating it, then run
   `run_bzr_raw --json --config "$_INLINE_SEARCH_CONFIG" --server-url "$BZ_URL" bug search
   --from-url "${BZ_URL}/buglist.cgi?bug_id=${BUG1}"`. The unique bug-ID filter and absence of an
   explicit low limit prevent unrelated pagination diagnostics. Require
   `assert_success`, `assert_json_array_min_length '.' 1`, and `assert_stderr_empty`. Run
   `make functional-test`; expect this case to fail with the no-default configuration error. This
   is the red proof that the production-shaped fixture bites before implementation.
2. Add the optional `active_server_url: Option<&str>` parser parameter without consulting it yet,
   and update every existing production caller, fuzz wrapper, and parser test to pass `None`. Run
   `make test-one T=parse_url`; expect the existing parser suite to compile and pass.
3. Add a parser test using an empty `Config`, imported
   `https://bugzilla.example.com/buglist.cgi?product=Firefox`, and active
   `https://bugzilla.example.com:8443`. Assert success and no saved server name.
4. Run `make test-one T=parse_url_hostname_matches_inline_server_without_default`; expect the new
   test to execute and fail with the existing no-default configuration error.
5. Add a mismatched-inline parser test with an empty config and assert parsing succeeds with no
   named server. Retain the existing no-inline/no-default error test.
6. Add table-driven parser cases showing identical hostnames remain silent across scheme and port
   differences. Add malformed-active cases proving configured match and empty/default-less error
   precedence remain unchanged.
7. Add a Tokio search test with an empty explicit config path and
   `CommandContext::with_inline_server(Some(InlineServer { url: mock.uri(), ... }))`; import the
   same mock hostname and assert the expected REST request occurs and succeeds credentiallessly.
   Mount `GET /rest/version` returning `{"version":"5.1.2"}` before the `/rest/bug` mock because
   inline servers have no cached API mode.
8. Add a two-server precedence test. Start mock A and configure/import it as
    `http://localhost:<A-port>`; use mock B's ordinary `http://127.0.0.1:<B-port>` URI for the
    inline server. Assert mismatch guidance through `TracingCapture`, zero requests received by A,
    and saved-query server name A after `--save-as`. On B, mount `/rest/version` with expectation 1
    and `/rest/bug` with expectation 1, proving inline routing despite A's configured match.
9. Run the two-server test by its exact name. Expect it to execute and fail specifically because
   the inline-destination mismatch warning is absent; routing to B and saved server A already pass.
10. Implement `active_server_hostname` by parsing the optional URL with `url::Url`. If hostname
    extraction fails, preserve the existing configured/default parser path so connection setup
    retains malformed-inline error ownership. Compare a valid hostname independently of configured
    lookup, suppress warning/error on equality, and emit
    `URL hostname '<imported>' does not match inline server hostname '<active>'; using inline server`
    on a mismatch. If no active hostname is available, run the existing default/error branches.
11. Change only the bug-search call site from `None` to
    `ctx.inline_server().map(|server| server.url.as_str())`; query and fuzz callers remain `None`.
12. Re-run the two-server test by its exact name; expect the same routing and saved-server checks
    plus the mismatch warning assertion to pass.
13. Run `make test-one T=inline_server`; expect all matching/mismatching inline tests to pass. Run
   `make test-one T=from_url`; expect all URL-import regression tests to pass.
14. Run `make lint`; expect exit 0 with no formatting, clippy, layout, or shell findings.
15. Commit as `fix(cli): honor inline server when importing search URLs`.

**Acceptance:** matching inline searches work without persisted/default config; mismatch guidance
describes the inline destination; configured/default and sanitization tests remain green.

## Task 2: Reproduce the production-shaped stateless path functionally

**Files:** modify `tests/functional/phases/08-bugs.sh`; direct helper changes are allowed only if an
existing assertion cannot express absence from stderr.

**Interfaces:** use existing `run_bzr_raw`, `BZR_STDERR`, `assert_success`, JSON helpers, `BZ_URL`,
and `FUNC_CONFIG_DIR`. No new production interface is introduced.

1. Re-read the concrete matching-host case added in Task 1 step 1 and confirm its assertions require
   exit 0, at least one returned bug, and no inline-host mismatch diagnostic in `BZR_STDERR`.
   Unrelated warnings emitted by real Bugzilla versions remain outside this contract.
2. Add a second empty-config invocation whose imported URL uses `localhost:$BZ_PORT` while the
   inline URL remains `127.0.0.1:$BZ_PORT`. Assert exit 0, real results, and stderr substrings naming
   `localhost`, `127.0.0.1`, and `using inline server`.
3. Run `make functional-test`; expect zero failures on the default Bugzilla version.
4. Run `make functional-test-all`; expect zero failures on bz50, bz52, and bz53, recording each
   version's passed/failed/skipped counts.
5. Run `make lint` and `make test`; expect exit 0 and zero warnings/failures.
6. Commit as `test(functional): cover stateless inline URL search`.

**Acceptance:** the real-server test cannot be satisfied by earlier persisted configuration and
proves the credentialless inline request path on every supported Bugzilla version.

## Rollback

Both commits are reverted normally. No persisted schema, dependency, or server-side state beyond
ordinary functional-test fixtures is introduced. Functional cleanup continues to remove the
temporary config directory through the existing trap.
