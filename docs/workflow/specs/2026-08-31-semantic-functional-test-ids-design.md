# Semantic functional-test IDs design

## Goal

Replace the functional suite's fragile global numbers with semantic stable references that show
their phase, remain unchanged when tests are inserted or reordered, and are enforced before merge.
This implements issue #602 and [ADR 0029](../../adr/0029-semantic-functional-test-ids.md).

## Identifier contract

Every functional test has the full ID `<phase>/<slug>`:

- `phase` is the sourced phase filename without `.sh` and matches the ASCII regular expression
  `[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*`, covering existing names such as `08-bugs` and
  `18d-dependency-analysis` while rejecting `08--bugs`, `08_Bugs`, and `bugs-08`;
- `slug` is lowercase kebab case, begins and ends with an ASCII letter or digit, and contains only
  ASCII lowercase letters, digits, and single hyphen separators; and
- the full expanded ID is unique during one functional run.

The phase is runner-owned rather than repeated at 416 call sites. Immediately before sourcing a
phase, `run-tests.sh` assigns its loop value to `CURRENT_TEST_GROUP`. A call site changes from one
combined string to `test_begin "create-first-bug" "bug create (bug one)"`. `test_begin` composes
and prints `[08-bugs/create-first-bug] bug create (bug one)`. Phase files may not read, assign, or
otherwise reference `CURRENT_TEST_GROUP`; only the runner and shared helper own it.

Slugs are literal and contain no shell expansion. A loop with mode-specific references uses a
`case` to call `test_begin` with one explicit literal slug per supported mode, then shares the test
body after that selection. The ID describes the tested behavior rather than its current position,
fixture number, issue number, or implementation detail. Descriptions may still interpolate
controlled values and retain their current prose independently.

## Harness behavior and failures

`test_begin` accepts exactly two arguments. It rejects a missing group, wrong arity, malformed
group or expanded slug, or a full ID already observed in the current run. Each failure writes an
actionable message to stderr naming the invalid value or duplicate and returns nonzero; the
runner's existing `set -e` stops before executing a test under an ambiguous reference.

The duplicate registry uses newline-delimited scalar state compatible with the repository's
existing Bash baseline, including the host's Bash 3.2. No associative arrays, external process,
or new dependency is introduced. IDs cannot contain newlines, so exact delimiter matching is
unambiguous. Counters, `CURRENT_TEST`, pass/fail/skip output, cleanup, and test execution otherwise
retain their behavior.

## Static repository guard

`tools/check-functional-test-ids.sh <root>` inspects `tests/functional/phases/*.sh` without
executing them. It requires each `test_begin` invocation to be a single line with a double-quoted
slug followed by a double-quoted description, with optional leading indentation. Slug templates
are not supported: the slug must be literal, and shell expansion is not accepted in an ID.

The guard compares the phase basenames on disk with the phase values enumerated by
`run-tests.sh`: each side must contain the same unique set, and every value must match the exact
phase grammar. It then rejects:

- the legacy number-and-period prefix;
- a malformed literal slug;
- any shell expansion in the slug;
- missing or extra argument text on the call line; and
- the same literal slug appearing twice in one phase file; and
- any `CURRENT_TEST_GROUP` occurrence in a phase file, preventing a sourced phase from replacing
  or deriving the runner-owned group.

Runtime validation remains authoritative for composed full IDs and execution-dependent duplicates.
`tools/check-functional-test-ids-tests.sh` builds isolated fixtures proving valid literal and
explicit loop branches pass and each rejection mode fails. The check has a dedicated
`make check-functional-test-ids` target, belongs to `make lint`, is installed in the pre-commit
hook, and runs as its own step in the pull-request `test-layout` job. This makes the new gate
visible in both local and hosted workflows instead of hiding it behind an aggregate target CI does
not call.

## Migration

All phase files move in one change because mixed one- and two-argument `test_begin` contracts
cannot run safely. Each old number is removed, a semantic slug is chosen from the behavior already
described, and the description text stays byte-for-byte unchanged except where separating the
prefix necessarily removes the number and following space. Mutually exclusive declarations use
distinct slugs that name their outcomes, including the TLS tools-unavailable versus proxy-start
failure paths and the dependency-analysis proxy-unavailable fallbacks. The six existing
mode-interpolated declarations become explicit `rest` and `xmlrpc` literal-ID branches while
retaining their shared test bodies and one runtime test per mode.

No compatibility translation or numeric alias is retained. Repository searches after migration
must find no legacy numeric `test_begin` argument or slug expansion. The baseline has 416 source
call sites; explicitly branching the six mode-dependent declarations may increase the source
count, but the functional run must retain the same test count and order for each environment.

## Documentation and scope

`tests/functional/README.md` documents the printed reference, slug rules, insertion workflow, and
the rule that descriptions may change without renaming IDs. Compiled `bzr` behavior, CLI output,
phase order, test order, assertions, fixtures, and external services are outside this change.

This is not an AI or security-relevant surface. IDs originate in reviewed repository scripts, are
printed as plain text, and are neither evaluated nor used to build commands, paths, URLs, queries,
or authorization decisions.

## Verification

- Checker fixture tests prove every accepted and rejected source shape, including valid and
  invalid phase basenames, runner/file set mismatches, duplicate slugs, and legacy numeric
  labels. Both column-zero and indented fixtures prove indentation cannot hide a valid, legacy,
  malformed, or duplicate-slug call. Separate fixtures reject a direct group assignment and a
  read used to derive and reassign the group from within a phase.
- Focused harness tests prove `test_begin` composes the phase and slug, preserves the description,
  and rejects missing groups, wrong arity, groups outside
  `[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*`, malformed slugs, and duplicates on Bash 3.2-compatible
  code. Checker fixtures reject variable-bearing slugs and accept explicit literal branches for
  loop modes.
- `make check-functional-test-ids`, `make check-shell`, `make lint`, and `make test` pass.
- `make functional-test-all` runs the migrated suite against every supported Bugzilla version and
  reports semantic references with no behavior regression.

## Durable workflow context

- Branch: `feat/semantic-functional-test-ids-602`
- Base branch: `main`
- Host architecture: `arm64`
- Target architectures: none declared by effective repository instructions
- Architecture relationship: `no-target-declared`; the shell-only change is architecture-insensitive
- Host shell: unknown; userland: BSD; tool-steering names: `LC_ALL`, `LANG`, `GH_PAGER`
- Guardrails: `make check-functional-test-ids`; `make check-shell`; `make lint`; `make test`;
  `make functional-test-all`
- ADR index coupling: not CI-coupled; this solo run adds the ADR index row
