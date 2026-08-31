# 0028: Functional-test containers use runtime-assigned ports and checkout-scoped names

## Status

Accepted

## Context

`tests/functional/setup-bugzilla.sh` starts each Bugzilla version's container
on a fixed host port (`bz50` → 8089, `bz52` → 8090, `bz53` → 8091) under a
fixed name (`bzr-func-test-<version>`). `tests/functional/run-tests.sh`
independently recomputes the same fixed default so the two separate process
invocations agree on where to find the container.

Two checkouts (a linked `git worktree`, or a second clone) running functional
tests at the same time collide: the second `docker/podman run --name
bzr-func-test-bz50 -p 8089:80 ...` fails outright, because both the container
name and the host port are already taken. `AGENTS.md` documents this as a
known hazard ("Be cautious when running multiple workflows simultaneously").
Issue #606 asks for the container's port to be supplied by the calling test
process instead of hardcoded, so concurrent invocations do not need to
coordinate by hand.

## Decision

Stop requesting a fixed host port. `setup-bugzilla.sh` publishes the
container's port with `-p 80` (no host port given) unless the operator sets
`BZR_FUNC_PORT`, in which case that exact port is requested as before.
`docker`/`podman` allocate the actual host port atomically as part of `run`
(verified: `podman run -d --name bzr-port-probe -p 80 <image>` then `podman
port bzr-port-probe 80/tcp` → `0.0.0.0:41049`, podman 5.x). There is no
window between choosing a port and the container claiming it, because the
runtime does both in one operation.

Give the container a name unique to the checkout it runs from, unless
`BZR_FUNC_CONTAINER` overrides it: `bzr-func-test-<version>-<checkout-id>`,
where `<checkout-id>` is `cksum` of the absolute path
`tests/functional/../..` resolves to for the running script. `cksum` is
POSIX and already present everywhere this suite runs; it needs no new
dependency. The id is keyed on the checkout's filesystem path, not on git
worktree metadata (`git rev-parse --git-dir`) or the branch name, so it
applies identically to a linked worktree, a second plain clone, or any other
means of having two checkouts on disk at once — the scenario issue #606
describes.

Because `setup-bugzilla.sh` and `run-tests.sh` run as separate processes
(the `functional-test` Make target has no `.ONESHELL`), `run-tests.sh` does
not inherit the port `setup-bugzilla.sh` chose. Instead of passing it through
out-of-band state, `run-tests.sh` queries the already-running container for
its own published port (`<runtime> port <name> 80/tcp`) — the running
container is itself the shared state, so there is nothing to keep in sync
and nothing to go stale.

`container_runtime()` and `bugzilla_container_name()` already exist in
`tests/functional/lib.sh`, sourced by `run-tests.sh`; `setup-bugzilla.sh`
duplicated equivalent logic inline. `setup-bugzilla.sh` now sources
`lib.sh` and uses the shared functions (extended with the checkout id and a
new `bugzilla_container_port()`) instead of keeping its own copy.

## Consequences

- Concurrent checkouts (worktrees, clones) can run `make functional-test` /
  `make functional-test-all` at the same time without operator coordination:
  distinct container names avoid the `run --name` collision, and
  runtime-assigned ports avoid the `run -p` collision.
- `BZR_FUNC_PORT` and `BZR_FUNC_CONTAINER` keep working exactly as before for
  an operator who wants a fixed, predictable port or name (e.g. to attach a
  debugger, or to point a long-running local Bugzilla at a known address).
- `cmd_status` and `wait_for_ready` in `setup-bugzilla.sh`, and
  `run-tests.sh`'s Bugzilla URL construction, must resolve the port by
  querying the runtime instead of reading a compile-time constant. This
  slightly widens `setup-bugzilla.sh`'s dependency on `lib.sh`, which was
  previously only sourced by the test runner.
- Two invocations from the exact same checkout directory, run at the same
  time, still collide (same checkout id → same default container name).
  That failure mode is unrelated to what issue #606 asks for and is not
  addressed here.
- `TLS_PORT`/`REDHAT_SHAPE_PORT` in `lib.sh` keep deriving from the resolved
  backend port by fixed offset (`+1000`/`+2000`); no change is needed there,
  since a fixed offset from an already-unique dynamic port stays unique in
  practice.

## Considered & rejected

- **Probe a free port with a throwaway socket bind (bind to port 0, read it,
  close it, then run the container on that port).** judgment: leaves a
  TOCTOU window between the probe and the container actually claiming the
  port; the runtime's own `-p 80` allocation does both atomically and is no
  more code.
- **Coordinate the chosen port through a state file** (e.g. under the
  checkout's `.git` directory), written by `setup-bugzilla.sh` and read by
  `run-tests.sh`. judgment: adds a stateful file with its own staleness and
  cleanup concerns; querying the running container's own port mapping is
  always current and needs no cleanup, since the container itself is the
  state.
- **Derive the checkout id from the git branch name instead of the checkout
  path.** judgment: two checkouts on the same branch (a worktree checked out
  to a branch already checked out elsewhere, or two clones on the same
  branch) would still collide; the path is what actually distinguishes
  concurrent checkouts.
- **Do nothing; document that operators must set `BZR_FUNC_PORT`/
  `BZR_FUNC_CONTAINER` by hand per checkout.** judgment: this is the status
  quo the issue is asking to remove; it requires every concurrent invocation
  to be manually coordinated, which is the friction `$campaign`-style
  parallel work hits in practice.
</content>
