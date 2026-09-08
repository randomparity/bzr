# Functional tier reclaims its container — design

Issue #739. Decision: [ADR 0067](../../adr/0067-functional-tier-reclaims-its-container.md), which
also amends [ADR 0058](../../adr/0058-suppress-tracing-ansi-off-terminal.md).

## Problem

`make functional-test` leaves its Bugzilla container running on every run and nothing reclaims it.
Names embed a one-way `cksum` of the checkout path, so once the checkout is deleted the name cannot
be recomputed and the container is orphaned permanently.

## Scope

Four changed contracts:

1. **Runner cleanup.** `tests/functional/run-tests.sh`'s existing `cleanup` trap gains a container
   stop, skipped when `BZR_FUNC_KEEP` is non-empty. It invokes the single lifecycle owner,
   `setup-bugzilla.sh stop`, with `BZR_BZ_VERSION` pinned to the version the run resolved so the
   child cannot disagree with the parent. Every added command is guarded: the script runs under
   `set -euo pipefail` and the trap must not alter the run's exit status.
2. **Verified stop.** `cmd_stop` in `tests/functional/setup-bugzilla.sh` takes over the
   `container_exists` check `cmd_reset` performs after calling it, suppresses "Container removed."
   on failure, and returns 1. `cmd_reset` reads that status.
3. **Guardrail coverage.** `setup-bugzilla.sh` and `run-all-versions.sh` join the `shellcheck` and
   `bash -n` lists in `check-shell`. Both are clean today, verified by running the gate's own
   commands over them.
4. **Documentation.** `BZR_FUNC_KEEP` gains a row in `tests/functional/README.md`'s environment
   table; the orphan-container note there, the stale-container note in `CONTRIBUTING.md:99-101`, and
   the `functional-test` comment in the Makefile record the new default.

Out of scope per the frozen charter: `run-all-versions.sh`/`cleanup_all`, ADR 0058's
`functional-compare` reset behaviour, CI-side container collection, `tests/functional/phases/*`.

## Success

- `make functional-test` leaves no container behind; with `BZR_FUNC_KEEP=1` it leaves exactly the
  one it used.
- A refused removal is reported rather than announced as success, and never changes the exit status.
- `make lint` (including `check-shell`), `make test`, and `make functional-test` are green.

## Validation

Plan:
[2026-09-07-functional-tier-reclaims-its-container.md](../plans/2026-09-07-functional-tier-reclaims-its-container.md).
Its Verification inventory carries one entry per contract above: 1-3 are `focused-test` with a
runnable red observation, 4 is `task-test-not-applicable`.
