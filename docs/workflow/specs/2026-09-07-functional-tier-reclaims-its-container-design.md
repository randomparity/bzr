# Functional tier reclaims its container — design

Issue #739. Decision and mechanism:
[ADR 0067](../../adr/0067-functional-tier-reclaims-its-container.md), amending
[ADR 0058](../../adr/0058-suppress-tracing-ansi-off-terminal.md). Steps and code:
[the plan](../plans/2026-09-07-functional-tier-reclaims-its-container.md).

## Problem

`make functional-test` leaves its Bugzilla container running on every run and nothing reclaims it.
Names embed a one-way `cksum` of the checkout path, so once the checkout is deleted the name cannot
be recomputed and the container is orphaned permanently.

## Scope

Four changed contracts, mechanism in the ADR and the plan rather than repeated here.

1. **Runner cleanup** — `run-tests.sh`'s trap reclaims the container unless `BZR_FUNC_KEEP` is set,
   guarded against altering the run's exit status, with the caller's `XDG_CONFIG_HOME` and the
   run's `BZR_BZ_VERSION` restored for the child.
2. **Verified stop** — `cmd_stop` returns 1 when the container survives instead of reporting
   success; `cmd_reset` reads that, and `functional-stop-all` still attempts every version.
3. **Guardrail coverage** — `setup-bugzilla.sh` and `run-all-versions.sh` join `check-shell`.
4. **Documentation** — five sites, listed in plan Task 2 steps 3-5, including the
   `functional-compare` comment at `Makefile:200-203` whose premise this change falsifies.

Out of scope per the frozen charter: `run-all-versions.sh`/`cleanup_all`, ADR 0058's
`functional-compare` reset behaviour, CI-side container collection, `tests/functional/phases/*`.

## Success

- `make functional-test` leaves no container; with `BZR_FUNC_KEEP=1`, exactly the one it used.
- A refused removal is reported rather than announced as success, and moves the exit status in
  neither direction.
- `make lint` (including `check-shell`), `make test`, and `make functional-test` are green.

## Validation

The plan's inventories carry five `Verification` entries across these four contracts — contract 1
splits into reclaim behaviour and exit-status preservation. Contracts 1-3 are `focused-test` with
runtime-independent red observations; contract 4 is `task-test-not-applicable`.
