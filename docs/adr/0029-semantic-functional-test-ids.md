# ADR 0029: Identify functional tests by phase and semantic slug

## Status

Accepted

## Context

The functional suite prints a manually maintained global number at the start of every test
description. New coverage inserted beside related tests has exhausted that sequence: later tests
use letter and digit suffixes, and the same reference now appears in more than one phase. At commit
`3ea672ca168b6da131894a5d78c997d49e0e8d02`,
`rg -n '^test_begin "[0-9]+[a-z0-9-]*\.' tests/functional/phases | wc -l` finds 402
numbered call sites, while grouping their prefixes finds repeated references including `8h`,
`15c`, `120`, and `152`. Inserting a test in logical order therefore requires either renumbering
unrelated tests or extending an identifier whose relationship to its phase is implicit.

Issue #602 requires an insertion-friendly reference associated with the test's group. The
repository operator selected semantic stable IDs over phase-scoped numeric IDs.

## Decision

Identify a functional test as `<phase>/<slug>`. The phase is the existing phase filename without
`.sh`, supplied by `run-tests.sh` immediately before it sources that phase. Each `test_begin` call
supplies two arguments: a lowercase kebab-case semantic slug and the human-readable description.
For example, phase `08-bugs` and slug `create-first-bug` produce
`08-bugs/create-first-bug`. Controlled loop variables may occupy a complete slug segment when each
runtime expansion is lowercase kebab-case and denotes a distinct semantic case.

`test_begin` validates the expanded phase and slug, rejects a repeated full ID during the run, and
prints the full ID separately from the description. A repository guard checks every phase call
site for the two-argument shape, the phase and slug-template grammar, runner-to-file
correspondence, duplicate templates within one phase, and remnants of the numeric format.
Mutually exclusive call sites still use distinct semantic slugs that name the reported outcome;
the runtime duplicate check is not replaced by a static exception. The guard has fixture tests,
runs from `make lint`, runs in the installed pre-commit hook, and is an individually named
pull-request CI step.

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

The static guard checks source templates, while runtime validation checks their expansions. This
split keeps the checker dependency-free and covers dynamic loop cases without attempting to parse
or execute arbitrary Bash.

## Considered & rejected

- **Keep the global numeric sequence.** verified: the commands in Context find 402 numbered call
  sites and repeated references at commit `3ea672ca168b6da131894a5d78c997d49e0e8d02`; the current
  sequence no longer provides unique, insertion-friendly references.
- **Use phase-scoped numbers with gaps.** judgment: gaps postpone renumbering but do not make an ID
  stable under repeated insertion, and the operator explicitly selected semantic stable IDs.
- **Repeat the phase in every call site.** judgment: the runner already owns the phase order and
  source path, so repeating it across every test adds a second value that can drift.
- **Derive IDs from descriptions.** judgment: editorial wording changes would rename references,
  while normalization can collapse distinct descriptions to the same slug.
- **Validate only statically.** judgment: a line-oriented source check cannot prove the values of
  controlled loop expansions or detect duplicate expanded IDs across executed branches.
- **Validate only at runtime.** judgment: a skipped or otherwise unexecuted malformed call site
  could merge unnoticed, so the pull-request guard must also inspect source templates.
