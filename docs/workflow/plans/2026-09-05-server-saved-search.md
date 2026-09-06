# Server-side saved search — implementation plan

Goal: `bzr bug search --saved-search <NAME> [--sharer <ID>]` detects Red Hat saved-search
extension support before dispatch, sends `savedsearch`/`sharer_id` when present, and fails with
an actionable exit-15 error when absent.

Spec: `docs/workflow/specs/2026-09-05-server-saved-search-design.md` — decision:
[ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md).
Branch `feat/saved-search-670`, base `main`, scope token `q670-1c1b8eb2`.

Expected implementation size: 380–480 changed lines (M) — summed from the file map.

This plan states intent and contracts. It does not dictate line-level code: the previous cycle
showed that a long prescriptive plan generates defects faster than it prevents them, and this
repository's guardrails (`make lint` with clippy `-D warnings`, `make test`,
`make functional-test`, `bash tests/functional/pybz/container-tests.sh`) carry the mechanics.
Follow the conventions already in each file.

## Global Constraints

- Guardrails run **bare** — no pipes, no `|| true`. `make lint`; `make test` (~3 min);
  `make test-one T=<substr>` to iterate; `make functional-test` (~10 min, Docker);
  `bash tests/functional/pybz/container-tests.sh` (no container needed). **Never bare
  `cargo test`.** Background long runs and read once.
- Unit tests live in sibling `<name>_tests.rs` files; inline `mod tests {}` in `src/` is
  forbidden. API tests use `#[tokio::test]`.
- Output goes through `Writers`; never `println!`/`eprintln!` in `src/`.
- Do not touch: `src/cli/bug/create.rs`, `src/cli/bug/update.rs`, `src/commands/bug/update/`,
  `src/types/bug/payload.rs`, `schemas/bug-*-input.json`, `schemas/batch-result.json`,
  `schemas/compound-create-result.json`, or any `comment-tag`/`minor-update` site in
  `tests/functional/pybz/container-tests.sh` including line 962. All belong to issue #672.
- `src/output/mod.rs:10` `SCHEMA_VERSION` `3.0.1` → `3.0.2` — a known one-line collision with
  #672 making the identical bump. Make it; do not bump to `3.0.3`; the orchestrator reconciles.
- `docs/adr/README.md` is not ours. ADR 0052 is already committed; report `index row pending`.

## File map

| File | | Answerable for |
|---|---|---|
| `src/error.rs` (+ sibling) | changed | `UnsupportedServerCapability`, exit 15, `error_type`, structured detail |
| `schemas/error.json` | changed | `exit_code` max 15, `capability` key |
| `src/output/mod.rs` | changed | `SCHEMA_VERSION` 3.0.2 |
| `src/client/auth/mod.rs` (+ sibling) | changed | `DetectedServerSettings.extensions` |
| `src/client/resources/server.rs` | changed | extension probe used by detection |
| `src/commands/runtime/shared/connection/detect.rs` (+ sibling) | changed | persisting the capability |
| `src/config/model.rs` (+ sibling) | changed | `ServerConfig.server_extensions` |
| `src/types/bug/search.rs` (+ sibling) | changed | `saved_search`, `sharer_id`, both predicates |
| `src/client/resources/bug.rs` (+ sibling) | changed | REST parameter emission |
| `src/xmlrpc/resources/bug.rs` (+ sibling) | changed | XML-RPC member emission |
| `src/cli/bug/search.rs` (+ sibling) | changed | the two flags and their constraints |
| `src/commands/bug/search.rs` (+ sibling) | changed | capability gate, routing, error |
| `docs/bzr-cli.md` | changed | command tree, prose, options, exit-code table |
| `docs/dev/python-bugzilla-parity.md` | changed | reworded saved-search row |
| `tests/functional/lib.sh` | changed | `expect_gap` unaffected; see Task 5 |
| `tests/functional/compare/01-bug-lifecycle.sh` | changed | probe expected-exit parameter; saved-search block |
| `tests/functional/pybz/container-tests.sh` | changed | `saved-search` stub arms and parity row 960 only |
| `tests/functional/phases/08f-bug-saved-search.sh` | created | detect-and-error against a real container |
| `tests/functional/run-tests.sh` | changed | sourcing the new phase |

## Task 1 — error variant and published contract

**Verification.** Contract: an unsupported-capability error exits 15 with
`error_type` `unsupported_server_capability` and carries the capability in its structured
detail. Mode: focused-test, in `src/error_tests.rs` following the file's existing per-variant
cases. Red: the variant does not exist (compile failure). Green:
`make test-one T=unsupported_server_capability`.

Add `UnsupportedServerCapability { capability: String, detail: String }` to `BzrError` with
`EXIT_CODE_UNSUPPORTED_CAPABILITY = 15`, its `error_type()` arm, and its `structured_detail()`
entry (ADR-0014) exposing `capability`. Raise `schemas/error.json`'s `exit_code` `maximum` to
15 and add the `capability` property with a description naming the variant, matching the
neighbouring entries' style. Bump `SCHEMA_VERSION` to `3.0.2`.

Check whether a schema-conformance or exit-code test enumerates the variants
(`rg -l 'EXIT_CODE_COLLISION\|exit_code' src/ tests/ schemas/`) and extend whatever it finds —
the repository gates published schemas, so an unlisted variant is a failing test, not a silent
gap.

**Acceptance.** `make lint` and `make test` green; the new variant's exit code and type are
asserted; `schemas/error.json` admits 15.

## Task 2 — capability detection and caching

**Verification.** Two contracts, both focused-test in
`src/commands/runtime/shared/connection/detect_tests.rs` and
`src/client/auth/mod_tests.rs` as their existing cases are organised:
(a) a successful probe persists the extension list; (b) a failed probe persists nothing and
leaves any cached value untouched. Red: `extensions` is not a field (compile failure). Green:
`make test-one T=extension`.

Add `extensions: Option<Vec<String>>` to `DetectedServerSettings` with the same documented
contract its `server_version` field already carries — `Some` only when `/rest/extensions`
responded, `None` on transient failure — and populate it in `detect_server_settings` from the
existing `BugzillaClient::server_extensions()`. Sort the names so the persisted value is
stable. Add `server_extensions: Option<Vec<String>>` to `ServerConfig` with
`#[serde(default, skip_serializing_if = "Option::is_none")]`, matching its neighbours, and
persist it in `persist_detected_settings` under the same `is_some()` guard the version state
uses.

A probe failure must not be fatal to unrelated commands: detection already tolerates a failed
version probe, and this follows it.

**Acceptance.** Detection persists a sorted extension list on success and nothing on failure;
existing config round-trip tests still pass; `make lint` and `make test` green.

## Task 3 — the flags and the capability gate

**Verification.**
- Clap constraints: four cases in `src/cli/bug/search_tests.rs` using the file's existing
  `parse_error_kind` — positional-query conflict and `--from-url` conflict
  (`ErrorKind::ArgumentConflict`), missing `--saved-search` (`MissingRequiredArgument`),
  non-numeric `--sharer` (`ValueValidation`). Red: clap returns `UnknownArgument` for all four
  because the flags do not exist, so each assertion fails against its expected kind — do not
  accept `UnknownArgument` as the intended red, these cases exist only to pin the attributes.
  Green: `make test-one T=saved_search_conflicts`, `make test-one T=sharer_re`.
- Wire contract: wiremock tests in `src/client/resources/bug_tests.rs` and
  `src/xmlrpc/resources/bug_tests.rs` asserting `savedsearch`/`sharer_id` reach the request.
  Red: unknown field on `SearchParams` (compile failure).
- Gate contract: in `src/commands/bug/search_tests.rs`, a server advertising no extensions
  must produce exit 15 **without** issuing a `/rest/bug` request — mount that path with
  `.expect(0)`, which is what proves the refusal precedes dispatch. A server advertising
  `RedHat` must dispatch. Red: no gate exists, so the refusal case dispatches and returns `Ok`.
  Green: `make test-one T=saved_search_requires_extension`.
- Predicate contract: add a `saved_search` row to each of the two existing per-field tables in
  `src/types/bug/search_tests.rs` (`search_params_has_filters_for_each_individual_field`,
  `search_params_has_structured_filters_for_each_individual_field`) rather than a standalone
  test, so `saved_search` does not become the one field missing from inventories whose
  assertion messages claim completeness. Red: the rows fail before the predicates learn the
  field.

Add `saved_search: Option<String>` and `sharer_id: Option<u64>` to `SearchParams`; add both to
`has_filters()` and `has_structured_filters()`; emit them from both mappers (`savedsearch` in
each existing option table, `sharer_id` beside `limit`/`offset` in REST and through the
existing `xmlrpc_id` helper in XML-RPC).

Add `--saved-search` (`conflicts_with_all = ["query", "from_url"]`) and `--sharer`
(`requires = "saved_search"`, `u64`) to `SearchArgs`, with doc comments that say these are
server-side saved searches, unrelated to `bzr query`, and require the Red Hat extension.
Extend `LONG_ABOUT` and its examples. `SearchArgs` has no `Default`, so every struct-literal
site must gain both fields — there are six, in `src/commands/bug/search_tests.rs` and
`src/commands/bug/mod_tests.rs`; `cargo build --tests` finds any this misses.

In `src/commands/bug/search.rs`, before connecting, widen the no-query-source input error to
name all three sources. After connecting and before dispatching, when `--saved-search` is set,
consult the cached-or-probed extension list and either dispatch or return the Task 1 error —
the three-outcome table in the spec is the contract, and the undetermined case must be
distinguishable from the absent case in its message.

**Acceptance.** All four rejections at parse time (exit 2); exit 15 with no `/rest/bug` request
when the extension is absent; both parameters on the wire when present; `make lint`,
`make test` green.

## Task 4 — documentation

Update `docs/bzr-cli.md`: the `bug search` command-tree continuation line gains
`[--saved-search <NAME>] [--sharer <ID>]`; the `bzr bug search` section gains an example, a
short prose paragraph, two options-table rows, and a note that these require a Bugzilla with
the Red Hat saved-search extension and exit 15 otherwise; the trailing footnote names all three
query sources. Add exit 15 to whatever exit-code table the document carries.

Reword `docs/dev/python-bugzilla-parity.md`'s saved-search row to the pre-approved wording, and
update the byte-identical literal at `tests/functional/pybz/container-tests.sh:960`:

```text
| Server saved search | `bzr bug search --saved-search` | bzr errors; python-bugzilla returns unfiltered results (#670) | `compare/01-bug-lifecycle/saved-search` |
```

**Verification.** `cargo build` then
`BZR_BIN=target/debug/bzr sh agent-skills/tests/flag-drift-check.sh` bare — red before the
command-tree edit (`command tree is missing --saved-search`), green after, exit 0 with no ERROR
lines.

## Task 5 — comparison harness

The gap stays. Only the mechanism by which it is recorded changes, because bzr's failure is now
exit 15 rather than a clap parse error.

Give `lifecycle_bzr_probe` in `tests/functional/compare/01-bug-lifecycle.sh` an expected-exit
parameter defaulting to 2, so `lifecycle_bzr`, `lifecycle_bzr_gap` and
`lifecycle_bzr_xmlrpc_gap` keep their current behaviour for every other block. Point the
saved-search block at exit 15 with the new diagnostic. Keep `lifecycle_expect_gap 670`.

In `tests/functional/pybz/container-tests.sh`, update only the `saved-search` stub arms so the
fake `run_bzr` emits the new diagnostic and exit 15 instead of the unsupported-flag message —
the dedicated injection near line 545 and the unsupported-flag `case` near line 605. The
PASS/FAIL/GAP counts do not change, because the slug is still a gap. Touch no
`comment-tag`/`minor-update` site.

**Verification.** `bash tests/functional/pybz/container-tests.sh` bare. Red after the phase
edit and before the stub edit: the saved-search slug reports FAIL instead of `GAP (#670)`.
Green: exit 0, with counts unchanged from `main`.

## Task 6 — functional phase

Create `tests/functional/phases/08f-bug-saved-search.sh` and add it to the `for _phase in` list
in `tests/functional/run-tests.sh` (`check-functional-test-ids` fails `make lint` until both
agree — that mismatch is the task's red). Header in the style of `08b-bugs-paging.sh`, four to
ten lines, stating that no supported image implements the extension so the *success* path
cannot be exercised here and the wiremock tests carry it.

Nine test ids, none conditional — the phase seeds nothing, so a SKIP would itself be a defect:
`--saved-search` exits 15 with a message naming the capability, over REST and over XML-RPC; the
same credentiallessly via `--server-url`; `--sharer` likewise; and the four parse-time
rejections at exit 2. Use `assert_exit_code` and `assert_stderr_contains`.

**Verification.** `make lint` bare, then `make functional-test` bare in the background, read
once. Expect exit 0 and every new id PASS with no SKIP. Then `make functional-stop` and
`make functional-compare` bare — the comparison must still report
`compare/01-bug-lifecycle/saved-search` as `GAP (#670)`. The stop matters: the compare run's
assertions are corpus-sensitive and `make functional-test` leaves dozens of bugs behind.

## Deferrals

One, no `docs/debt/` directory in this repository: the saved-search comparison remains vacuous
on the python-bugzilla side and corpus-sensitive on both. Owned by **issue #710**
(Red-Hat-shaped proxy fixture), filed by the orchestrator.
