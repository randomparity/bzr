# ADR 0029: Identify functional tests by phase and semantic slug

## Status

Accepted

## Context

The functional suite prints a manually maintained global number at the start of every test
description. New coverage inserted beside related tests has exhausted that sequence: later tests
use letter and digit suffixes, and the same reference now appears in more than one phase. At commit
`3ea672ca168b6da131894a5d78c997d49e0e8d02`,
`rg -n '^[[:space:]]*test_begin "[0-9]+[a-z0-9-]*\.' tests/functional/phases | wc -l`
finds 416 numbered call sites, while grouping their prefixes finds repeated references including
`8h`, `15c`, `120`, and `152`. Inserting a test in logical order therefore requires either
renumbering unrelated tests or extending an identifier whose relationship to its phase is
implicit.

Issue #602 requires an insertion-friendly reference associated with the test's group. The
repository operator selected semantic stable IDs over phase-scoped numeric IDs.

## Decision

Identify a functional test as `<phase>/<slug>`. The phase is the existing phase filename without
`.sh`, supplied by `run-tests.sh` immediately before it sources that phase. Each `test_begin` call
supplies two arguments: a lowercase kebab-case semantic slug and the human-readable description.
For example, phase `08-bugs` and slug `create-first-bug` produce
`08-bugs/create-first-bug`. Slugs are literal: looped cases select between explicit literal-ID
`test_begin` branches so every possible reference is visible to the repository guard.

`test_begin` validates the expanded phase and slug, rejects a repeated full ID during the run, and
prints the full ID separately from the description. A repository guard checks every phase call
site for the two-argument shape, the phase and literal-slug grammar, runner-to-file
correspondence, duplicate slugs within one phase, remnants of the numeric format, and any
`CURRENT_TEST_GROUP` access from a sourced phase. The last rule makes runner ownership real despite
Bash `source` sharing one variable scope. It also requires the runner's group assignment and source
operation to be adjacent canonical lines using the same `_phase` token, so set equality cannot hide
a swapped assignment/source mapping. Mutually exclusive call sites still use distinct semantic
slugs that name the reported outcome; the runtime duplicate check is not replaced by a static
exception. The guard has fixture tests, runs from `make lint`, runs in the installed pre-commit
hook, and is an individually named pull-request CI step.

Before the first harness edit, retain a private transcript of `make functional-test-all`. After
migration, run the same command and normalize both transcripts by stripping only the old numeric or
new bracketed reference prefix from each completed test line. The ordered descriptions, PASS/SKIP
outcomes, and per-version summary counts must match byte-for-byte. Both runs must be green; the
comparison supplements rather than replaces the live functional proof.

Existing phase order, test order, descriptions, assertions, skip behavior, and compiled `bzr`
behavior do not change. The old numeric labels receive no compatibility aliases because they are
internal test-output references rather than a published product contract.

## Consequences

Tests can be inserted or reordered without changing another test's reference. A failure names its
own phase and subject, and duplicate or malformed references fail early with an actionable error.
Authors must choose a unique semantic slug rather than taking the next number. Renaming a test's
description does not rename its ID; intentionally changing what a test represents may warrant an
explicit ID change. Mutually exclusive reports for different outcomes use different references
even when their descriptions remain identical during the migration.

The static guard checks every literal source ID, while runtime validation checks the composed full
IDs and catches execution-dependent repetition. This split keeps the checker dependency-free
without leaving variable expansions outside pre-merge validation.

The one-time transcript oracle makes preservation of executed test count, order, descriptions, and
outcomes falsifiable during migration. It is not retained as a permanent fixture because supported
Bugzilla and optional local-tool outcomes can legitimately change in later feature work.

## Considered & rejected

- **Keep the global numeric sequence.** verified: the commands in Context find 416 numbered call
  sites and repeated references at commit `3ea672ca168b6da131894a5d78c997d49e0e8d02`; the current
  sequence no longer provides unique, insertion-friendly references.
- **Use phase-scoped numbers with gaps.** judgment: gaps postpone renumbering but do not make an ID
  stable under repeated insertion, and the operator explicitly selected semantic stable IDs.
- **Repeat the phase in every call site.** judgment: the runner already owns the phase order and
  source path, so repeating it across every test adds a second value that can drift.
- **Derive IDs from descriptions.** judgment: editorial wording changes would rename references,
  while normalization can collapse distinct descriptions to the same slug.
- **Allow variables in slug segments.** judgment: a line-oriented source check cannot prove every
  possible expansion, so malformed or colliding references could remain hidden in an unexecuted
  branch or zero-iteration loop.
- **Validate only statically.** judgment: source uniqueness cannot detect a full ID repeated by
  execution-dependent control flow.
- **Validate only at runtime.** judgment: a skipped or otherwise unexecuted malformed call site
  could merge unnoticed, so the pull-request guard must also inspect source call sites.
