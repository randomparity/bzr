# Implement Red Hat multi-valued bug fields

Goal: normalize stock scalar and Red Hat array response fields into the array
contract in ADR 0025. Rust/serde performs strict normalization; a loopback
response-shaping proxy keeps functional proof on the real Bugzilla HTTP path.
Tech stack: Rust 2021, serde/serde_json already installed, Bash functional
harness, Python 3 stdlib.

## Global constraints

- Preserve missing as `null`; emit every present component/version as an array.
- Accept only strings or arrays containing only strings; preserve order.
- Retain stock bz50, bz52, and bz53 functional arms.
- Add no dependency and no runtime vendor switch.
- Host is arm64; declared release targets remain Linux, macOS, Windows, and
  powerpc64le as configured by repository release workflows.

## Task 1: Normalize and render bug fields

Files: modify `src/types/bug.rs`, `src/types/bug_tests.rs`,
`src/types/bug/adjacency.rs`, `src/client/response.rs`, their sibling tests,
and `src/output/resources/bug.rs` plus tests.

Interfaces: define `pub(crate) fn deserialize_optional_string_list<'de, D>(D)
-> Result<Option<Vec<String>>, D::Error>` for serde fields. `Bug.component`,
`Bug.version`, and `BugAdjacencyBug.version` expose `Option<Vec<String>>` to
the output layer.

1. Add tests parsing scalar, empty/single/multi arrays, and rejecting null,
   numeric, object, nested, and mixed arrays. Run
   `make test-one T=bug_deserializes_multi_value`; expect failure before code.
2. Implement the strict untagged string/list wire decoder and annotate all
   applicable fields. Run the focused tests; expect pass.
3. Update table/detail renderers to join slices with `, ` and update fixtures.
   Run `make test-one T=bug`; expect pass.
4. Commit as `fix(api): normalize multi-valued bug fields`.

Acceptance: both wire forms yield identical array JSON, malformed forms fail,
and readable output retains every value.

## Task 2: Publish and prove the contract

Files: modify `schemas/bug.json`, `schemas/bug-adjacency.json`,
`src/commands/schema_tests.rs`, `src/output/mod.rs`, current schema-version
consumers and assertions, and `docs/bzr-cli.md`; add
`tests/functional/redhat-shape-proxy.py`; modify `tests/functional/lib.sh`,
`tests/functional/phases/18e-release-readiness.sh`, and functional documentation.

Interfaces: the proxy accepts listen port and backend port, forwards HTTP, and
rewrites successful response objects below each top-level `bugs` array. The
phase targets it through an inline server URL and terminates it on completion.

1. Change schema fixtures to array-or-null and add conformance assertions. Run
   `make test-one T=bug_object`; expect failure before implementation and pass
   afterward.
2. Advance the breaking JSON envelope contract from `0.6.2` to `1.0.0` under
   ADR 0007 and update current consumers, assertions, and documentation.
3. Document the normalized output and the captured deployment-profile test.
4. Add proxy unit self-tests for scalar-to-array, empty/multi preservation,
   untouched non-bug data, readiness, malformed upstream JSON, and upstream
   failure; run `python3 tests/functional/redhat-shape-proxy.py --self-test`.
5. Add lifecycle helpers that launch the proxy, wait for readiness, compose
   cleanup with the runner's existing EXIT trap, record the log path on
   failure, terminate the proxy, assert it is gone, and restore the prior trap.
6. Extend release-readiness with list/search and adjacency calls through the
   proxy, asserting arrays and exact values. The phase order is launch,
   readiness, CLI calls, teardown assertion, and trap restoration. Run
   `make functional-test`; expect the new scenario and existing stock phase to
   pass.
7. Run `make test-fast`, `make lint`, `make test`, and
   `make functional-test-all`; expect exit 0. Commit as
   `test(functional): cover Red Hat bug response shapes`.

Acceptance: published schemas describe the normalized contract; all three
stock server versions remain green; the Red Hat-shaped scenario traverses the
real server, proxy, client, command, and output layers.
