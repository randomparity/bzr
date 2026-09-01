# Server capability conformance implementation plan

**Goal:** Normalize server capability response shapes and prove the public behavior on all
supported Bugzilla containers.

**Architecture:** Keep wire normalization at the server resource's serde boundary by
delegating to ADR 0033's shared unsigned adapter. Preserve ADR 0005's optional,
credential-gated fetch and use the existing response-shape proxy for production-shaped
end-to-end proof.

**Tech stack:** Rust 1.89+, serde, wiremock, Bash functional harness, Python 3 stdlib proxy.

Expected implementation size: 170–280 changed lines (M) — derived from four Rust behaviors and one bounded proxy/functional proof.

## Global Constraints

- Host is arm64 macOS; targets are x86_64/aarch64 Linux, powerpc64le Linux, s390x Linux,
  aarch64 macOS, and x86_64/aarch64 Windows; relationship is different.
- `BASE_BRANCH` is `main` at `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b`.
- Keep ADR 0005's credentialless `null` and skip-request contract.
- Reuse ADR 0033's `u64_from_number_or_string`; do not implement #634.
- Keep schemas and `SCHEMA_VERSION` unchanged.
- Tests remain in sibling `*_tests.rs` files; user-facing output goes through existing
  output paths.
- Guardrails: `make test-one T=<focused-name>`, `make test-fast`, `make lint`,
  `make test`, `make functional-test-all`.
- ADR index is not CI-coupled and remains campaign-owned.

## Task 1: Pin and normalize server response behavior

**Files:** modify `src/client/resources/server.rs` and
`src/client/resources/server_tests.rs`.

**Interfaces:** consume
`crate::types::deserialization::u64_from_number_or_string`; preserve
`BugzillaClient::server_capabilities() -> Result<ServerCapabilities>` and the published
`ServerCapabilities` shape. Task 2 and the functional proof depend on unchanged public
capability fields.

1. Change the parameter fixture to string `"1000"`, add a distinct numeric-compatibility
   case, add string/number/omitted field-type cases (omitted remains `unknown`), and add an
   empty-name transition to the status fixture with an assertion that it is absent. Add
   DEBUG `TracingCapture` assertions for malformed `"not-a-number"` parameters
   (`reason=response_shape`) and HTTP 401 (`reason=request`); both stay null and exclude
   the test API key from captured output. Put the raw test key and a non-secret marker in
   one controlled response body; require the trace to retain the marker while redacting
   the key.
2. Run `make test-one T=server_capabilities_normalizes_attachment_size_to_bytes`; expect
   failure because string `maxattachmentsize` deserializes to `None` through the broad
   best-effort error arm.
3. Add private `UnsignedWire(u64)` with a zero `Default` and delegate its serde
   implementation to:

   ```rust
   u64_from_number_or_string(
       deserializer,
       "a non-negative integer or decimal numeric string",
       "expected a non-negative integer",
   )
   .map(Self)
   ```

   Apply it to optional `maxattachmentsize` and to `field_type` while retaining
   `#[serde(default)]`; use checked conversion before `field_type_name`, and filter
   `from.is_empty()`.
4. Split `attachment_size_limit` errors into a `BzrError::Deserialize` arm with
   `reason = "response_shape"` and a remaining arm with `reason = "request"`; preserve
   `None` from both.
5. Run `make test-one T=server_capabilities`; expect all matching tests green.
6. Commit with `fix(api): normalize server capability wire values` after relevant hooks
   pass.

**Acceptance:** stock string attachment limit parses, numeric compatibility remains,
field types accept both shapes, empty transitions are absent, 401 remains non-fatal and
classified separately from response decoding, and anonymous clients make no parameters
request.

## Task 2: Parse evidenced bare version suffixes

**Files:** modify `src/client/version.rs` and `src/client/version_tests.rs`.

**Interfaces:** consume and preserve private
`version_to_api_mode(version: &str) -> ApiMode`; the proxy in Task 3 depends on `5.2+`
selecting REST.

1. Add unit cases for `5.0+ -> Hybrid`, `5.1+ -> Rest`, `5.2+ -> Rest`, bare `5 ->
   Hybrid`, and an unrelated malformed minor retaining Hybrid fallback.
2. Run `make test-one T=version_to_mode_5_1_plus`; expect failure for the new suffixed
   assertion.
3. Preserve ordinary minor parsing. On its failure only, accept a trailing `+` when the
   complete version contains exactly two components and the suffix surrounds decimal
   minor digits. Reject `5.1++` and `5.1+.2`; preserve `5.3.3+` through the ordinary
   numeric-minor path.
4. Run `make test-one T=version_to_mode`; expect every version mapping test green.
5. Commit with `fix(api): parse bare Bugzilla version suffixes` after relevant hooks pass.

**Acceptance:** evidenced trailing-plus versions map across the existing 5.1 boundary and
no broader suffix leniency is introduced.

## Task 3: Prove production shapes through the functional proxy

**Files:** modify `tests/functional/redhat-shape-proxy.py` and
`tests/functional/phases/02-server-auth.sh`.

**Interfaces:** consume `redhat_shape_start`, `redhat_shape_stop`,
`REDHAT_SHAPE_PORT`, and `REDHAT_SHAPE_LOG` from `tests/functional/lib.sh`; emit stable
`server-capability shaped route=<route> count=<n>` evidence lines. No later task consumes
new shell globals.

1. Add proxy self-tests before implementation for opt-in parameters stringification,
   field-type string injection, empty status injection, bare version rewrite, default-mode
   preservation of capability/version routes, and unrelated payload preservation.
2. Run `python3 tests/functional/redhat-shape-proxy.py --self-test`; expect the new tests to
   fail because the transformer is absent.
3. Add one explicit `server-capabilities` proxy mode and one route-aware transformer
   returning the rewritten body plus named counters. Call it only for successful responses
   in that mode, log each non-zero route counter, reuse existing malformed-JSON 502
   handling, and leave current default behavior unchanged for every existing caller.
4. Run the proxy self-test again; expect all tests green.
5. Strengthen the stock credentialed capability assertion with non-null attachment size
   and no empty transitions; retain the credentialless null assertion.
6. Add one credentialed inline proxy case that starts explicit capability mode.
   Immediately after proxy startup, install
   `trap 'cleanup; redhat_shape_stop' EXIT`; assert non-null attachment size, the
   test-named custom field's mapped type, no empty transitions, REST mode from `5.2+`, and
   all four rewrite evidence lines. On the normal path require `redhat_shape_stop` to
   succeed, then restore `trap cleanup EXIT`.
7. Run `make functional-test-bz52`; expect the phase and full arm green. Then run
   `make functional-test-all`; expect bz50, bz52, and bz53 green.
8. Commit with `test(functional): prove server capability response shapes` after relevant
   hooks pass.

**Acceptance:** every public criterion has live-container proof; proxy self-tests prove
the requested shapes were injected; schemas and `SCHEMA_VERSION` remain unchanged.

## Task 4: Final integration verification

**Files:** no new files; inspect the full branch diff and schema hashes.

**Interfaces:** consumes Tasks 1–3; produces a guardrail-clean branch for review.

1. Verify `git diff --exit-code main -- schemas src/output/mod.rs`; expect no output and
   exit 0.
2. Run `make test-fast`; expect the unit suite green.
3. Run `make lint`; expect formatting, clippy, layout, functional ID, spawn, release,
   security-note, and shell checks green with zero warnings.
4. Run `make test`; expect all quiet unit and integration suites green.
5. Run `make functional-test-all`; expect all supported Bugzilla arms green.
6. Re-read `git diff main...HEAD` for scope and commit only review-driven corrections as
   separate conventional commits.

**Acceptance:** every named guardrail exits 0; no schema or schema-version diff exists;
the worktree is clean and all changes are inside the frozen surface.
