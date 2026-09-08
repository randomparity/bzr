# ADR 0067: The single-version functional tier reclaims its container

## Status

Accepted

## Context

Both functional entry points install an EXIT trap; only one stops a container. `cleanup_all`
(`tests/functional/run-all-versions.sh:12-18`) runs `setup-bugzilla.sh stop` per version, so
`make functional-test-all` cleans up after itself. `cleanup` (`tests/functional/run-tests.sh:69-73`)
removes `$FUNC_CONFIG_DIR` and tmpfiles and touches no container, so every `make functional-test`
leaves one running.

The leak is unrecoverable rather than untidy. `bugzilla_container_name`
(`tests/functional/container-env.sh:34-43`) embeds `bugzilla_checkout_id` (`:25-30`), a `cksum` of
the resolved checkout path (ADR 0030). Once that checkout is gone the id cannot be recomputed, so
nothing distinguishes the container from one in active use. Issue #739 observed 17 on one machine,
14 from checkouts that no longer exist, together about 2.4 GiB. The cost is not the memory; it is
that the set is unreclaimable by inspection.

Reuse is presumably why the tier never stopped: a cold start pays an image start plus a readiness
wait bounded at 90s for bz50 and 240s for bz52/bz53 (`setup-bugzilla.sh:24-32`), and ADR 0058
consequence 3 records a developer holding a warm container for `make functional-test`.

## Decision

The tier reclaims what it used. `run-tests.sh`'s EXIT trap invokes `setup-bugzilla.sh stop` for the
version it ran, whatever the run's outcome, unless `BZR_FUNC_KEEP` is non-empty. The trap reports a
failed removal and never changes the run's exit status, so a cleanup problem cannot turn a passing
tier red or a failing one green.

The stop lives in the runner rather than the `functional-test` recipe because every single-version
entry point goes through the runner: `make functional-test`, the three pinned `functional-test-bz5*`
targets, and a direct invocation. `BZR_FUNC_KEEP=1` is the opt-out that keeps the warm loop.

### Amendment to ADR 0058: `cmd_stop` verifies its own removal

ADR 0058 gave `cmd_reset` a removal check and left "`stop` … permissive behaviour for its other
callers" (`0058:60-65`). That held while no caller depended on `stop` having worked; this decision
creates one, and a trap-driven stop that reports "Container removed." regardless leaks exactly as
before while claiming otherwise. The check moves from `cmd_reset` into `cmd_stop`, which suppresses
that line and returns non-zero when the container survives; `cmd_reset` reads the status, and a
caller wanting permissiveness discards it as `run-all-versions.sh:14` and `run-compare-all.sh:15`
already do.

ADR 0058's motivating case — podman refusing to remove a container a running one depends on through
`--network container:<name>`, which is what a leftover python-bugzilla sidecar is
(`tests/functional/lib.sh:406`) — is runtime-specific. verified: under docker 29.7.2, `docker rm -f`
removes the base container with a live dependent attached and exits 0. So the check is not a podman
dependency-refusal detector; it makes the exit status honest for any refused removal.

## Consequences

- The second and later `make functional-test` in a checkout pays a container start again unless
  `BZR_FUNC_KEEP=1` is exported, and a failing run no longer leaves its container for post-mortem
  inspection — `CONTRIBUTING.md` already directs a diagnostic run to start from `reset`, so that
  was never the documented path. ADR 0058's consequence that `make functional-compare` destroys a
  container held for `make functional-test` now applies only under `BZR_FUNC_KEEP`.
- Two runs sharing a checkout id — which `CLAUDE.md` documents as unsupported, and which covers
  `make functional-test` beside `make functional-compare` in one checkout — now destroy each other's
  container instead of merely interfering with its data. `BZR_FUNC_KEEP=1` or a second checkout is
  the way to run them together; no coordination machinery is added for a case already documented as
  unsupported.
- `make functional-stop` now exits non-zero when removal is refused instead of reporting success,
  and `functional-stop-all` becomes a loop so one refused version still leaves the others attempted.
  The runner's own reclaim deliberately does not propagate that status — a refused reclaim is a
  stderr warning no gate reads, which diverges from `run-compare.sh`'s convention. A cleanup failure
  reddening a green tier, or greening a red one, would cost more than the leak it reports, and the
  leak now announces itself on the stream the developer is watching.
- `cleanup_all` becomes belt-and-braces; its `2>/dev/null || true` hides the new diagnostic, which
  the runner already printed where the developer was watching. Already-orphaned containers are not
  reclaimed here — `tests/functional/README.md`'s orphan procedure clears them, now once rather
  than recurrently.
- `setup-bugzilla.sh` joins `make check-shell`'s lists; it was in neither, so this change would
  otherwise land unlinted. `run-all-versions.sh` is equally unlinted but untouched here, so it stays
  out of this change.

## Considered & rejected

- **Do nothing.** verified: `cleanup` (`run-tests.sh:69-73`) touches no container, so the leak is
  one per run and unbounded. judgment: cleanup that requires enumerating live checkouts at that
  instant is not a resting state.
- **`make functional-clean` reclaiming containers whose checkout id is not live.** verified:
  `bugzilla_checkout_id` is a one-way `cksum` of the checkout path, so "not live" can only be
  decided by enumerating every live checkout and re-hashing each. judgment: the manual task the
  issue names as the cost, moved into a target, with the leak rate unchanged.
- **Record the container name inside the checkout.** verified: the trace lives in the thing that
  gets deleted — `git worktree remove` takes it with the name — so the unrecoverable case, 14 of the
  17 observed, is the one it does not cover.
- **Record it in a host-global registry outside every checkout**, the repair for the bullet above.
  judgment: it survives deletion, but it is new lifecycle state to write, prune, and keep honest
  against containers removed behind its back.
- **Label the container with its checkout path at `run` time.** judgment: permanently reclaimable
  by inspection, but only for containers created afterwards, and a tier that stops leaking leaves
  the label nothing to reclaim. Revisit if `BZR_FUNC_KEEP` becomes the common setting.
- **Stop in the `functional-test` recipe.** verified: the three `functional-test-bz5*` targets
  (`Makefile:215-225`) invoke `run-tests.sh` directly and would keep leaking, and `make` abandons
  the recipe when the runner exits non-zero.
- **Stop unconditionally, no opt-out.** judgment: the warm container is a workflow ADR 0058 already
  records, and one environment variable is cheaper than making the developer drive
  `setup-bugzilla.sh` and `run-tests.sh` as separate steps.
