# Inline URL Server Warning Implementation Plan

**Goal:** make `bug search --from-url` diagnostics agree with the explicit inline request
destination and prove the stateless behavior against real Bugzilla containers.

**Architecture:** URL parsing receives optional active-server context but continues to own imported
URL validation, sanitization, and configured-server matching. The bug-search command supplies its
inline URL from `CommandContext`; shared connection setup remains the sole routing owner.

**Tech stack:** Rust 1.89.0, `url`, Tokio tests with wiremock, Bash functional harness, Docker or
Podman Bugzilla containers.

## Global constraints

- Host equality uses `url::Url` normalized hostnames; schemes and ports do not participate.
- Credential stripping, URL sanitization, named/default resolution, TLS, and routing remain intact.
- User-facing output uses tracing/Writers conventions; tests live in sibling `_tests.rs` files.
- Branch `feat/inline-url-server-warning-593`; base `main`.
- Guardrails are `make test-one T=<name-substring>`, `make test-fast`, `make lint`, `make test`, and
  `make functional-test-all`.

## Task 1: Make parser diagnostics inline-aware

**Files:** modify `src/commands/runtime/input/url_parser.rs`,
`src/commands/runtime/input/url_parser_tests.rs`, `src/commands/bug/search.rs`, and
`src/commands/bug/search_tests.rs`.

**Interfaces:** change
`parse_bugzilla_url(url_str: &str, config: &Config) -> Result<ParsedUrl>` to
`parse_bugzilla_url(url_str: &str, config: &Config, active_server_url: Option<&str>) -> Result<ParsedUrl>`.
`search::resolve_client_and_params` supplies
`ctx.inline_server().map(|server| server.url.as_str())`. Query and fuzz callers supply `None` as a
direct compilation dependency. Later functional coverage relies on matching inline hostname with
`query.server == None` and successful connection through `CommandContext`.

1. Add a parser test using an empty `Config`, imported
   `https://bugzilla.example.com/buglist.cgi?product=Firefox`, and active
   `https://bugzilla.example.com:8443`. Assert success and no saved server name.
2. Run `make test-one T=parse_url_hostname_matches_inline_server_without_default`; expect the new
   test to fail because the current signature has no active-server input or current parsing returns
   the no-default configuration error.
3. Add a mismatched-inline parser test with an empty config and assert parsing succeeds with no
   named server. Retain the existing no-inline/no-default error test.
4. Add table-driven parser cases showing identical hostnames remain silent across scheme and port
   differences. Add malformed-active cases proving configured match and empty/default-less error
   precedence remain unchanged.
5. Implement `active_server_hostname` by parsing the optional URL with `url::Url`. If hostname
   extraction fails, preserve the existing configured/default parser path so connection setup
   retains malformed-inline error ownership. Compare a valid hostname independently of configured
   lookup, suppress warning/error on equality, and emit
   `URL hostname '<imported>' does not match inline server hostname '<active>'; using inline server`
   on a mismatch. If no active hostname is available, run the existing default/error branches.
6. Update every call site: bug search passes the inline URL; query and fuzz entry points pass
   `None`; existing tests pass `None` except the new active-server cases.
7. Add a Tokio search test with an empty explicit config path and
   `CommandContext::with_inline_server(Some(InlineServer { url: mock.uri(), ... }))`; import the
   same mock hostname and assert the expected REST request occurs and succeeds credentiallessly.
8. Add a two-server precedence test: configured server A matches the imported hostname, while
   inline server B differs. Assert mismatch guidance through `TracingCapture`, request receipt by B
   and not A, and saved-query server name A after `--save-as`.
9. Run `make test-one T=inline_server`; expect all matching/mismatching inline tests to pass. Run
   `make test-one T=from_url`; expect all URL-import regression tests to pass.
10. Commit as `fix(cli): honor inline server when importing search URLs`.

**Acceptance:** matching inline searches work without persisted/default config; mismatch guidance
describes the inline destination; configured/default and sanitization tests remain green.

## Task 2: Reproduce the production-shaped stateless path functionally

**Files:** modify `tests/functional/phases/08-bugs.sh`; direct helper changes are allowed only if an
existing assertion cannot express absence from stderr.

**Interfaces:** use existing `run_bzr_raw`, `BZR_STDERR`, `assert_success`, JSON helpers, `BZ_URL`,
and `FUNC_CONFIG_DIR`. No new production interface is introduced.

1. Add a test after the credentialless inline bug-list case. Choose a config path beneath
   `FUNC_CONFIG_DIR` that has not been created. Invoke
   `run_bzr_raw --json --config "$path" --server-url "$BZ_URL" bug search --from-url
   "${BZ_URL}/buglist.cgi?product=FuncTestProd&bug_status=NEW" --limit 1`.
2. Before the implementation, run `make functional-test`; expect this case to fail with the
   no-default configuration error. This demonstrates that the test bites and that the prior
   configured fixture was the blind spot.
3. After Task 1, assert exit 0, at least one returned bug, and byte-empty `BZR_STDERR`.
4. Add a second empty-config invocation whose imported URL uses `localhost:$BZ_PORT` while the
   inline URL remains `127.0.0.1:$BZ_PORT`. Assert exit 0, real results, and stderr substrings naming
   `localhost`, `127.0.0.1`, and `using inline server`.
5. Run `make functional-test`; expect zero failures on the default Bugzilla version.
6. Run `make functional-test-all`; expect zero failures on bz50, bz52, and bz53, recording each
   version's passed/failed/skipped counts.
7. Run `make lint` and `make test`; expect exit 0 and zero warnings/failures.
8. Commit as `test(functional): cover stateless inline URL search`.

**Acceptance:** the real-server test cannot be satisfied by earlier persisted configuration and
proves the credentialless inline request path on every supported Bugzilla version.

## Rollback

Both commits are reverted normally. No persisted schema, dependency, or server-side state beyond
ordinary functional-test fixtures is introduced. Functional cleanup continues to remove the
temporary config directory through the existing trap.
