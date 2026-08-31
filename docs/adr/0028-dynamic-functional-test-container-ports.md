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
`docker`/`podman` allocate the actual host port as part of `run` (verified:
`podman run -d --name bzr-port-probe -p 80 <image>` then `podman port
bzr-port-probe 80/tcp` → `0.0.0.0:41049`, podman 5.x; repeated across five
containers, each got a distinct ephemeral port). The caller never chooses a
port up front, so there is no separate probe-then-claim step in this
script's own logic for another process to win a race against — unlike the
rejected socket-probe alternative below. Whether the runtime's own internal
allocation is atomic against a concurrent unrelated bind is the runtime's
concern, not verified here.

Give the container a name unique to the checkout it runs from, unless
`BZR_FUNC_CONTAINER` overrides it: `bzr-func-test-<version>-<checkout-id>`,
where `<checkout-id>` is the checksum field of `cksum` of the absolute path
`tests/functional/../..` resolves to for the running script — `cksum`
prints two space-separated fields (checksum, byte count), and only the
first is used: `printf '%s' "$path" | cksum | cut -d' ' -f1` (verified:
`printf '%s' "/home/dave/src/bzr" | cksum | cut -d' ' -f1` → `967091103`).
Use `printf '%s'` piped to `cksum`, not a bash here-string (`cksum
<<<"$path"`) — the here-string appends a trailing newline `printf` does
not, so it hashes different bytes and produces a different checksum
(verified: `cksum <<<"/home/dave/src/bzr"` → `954492732 19`, not
`967091103 18`). So the derived name never contains the embedded space or
second field. `cksum` is POSIX and
already present everywhere this suite runs; it needs no new dependency. The
id is keyed on the checkout's filesystem path, not on git
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
and nothing to go stale. Verified against both supported runtimes: podman
prints one line (`0.0.0.0:41049`); docker 29.7.2 prints two — one per
address family (`0.0.0.0:35053` and `[::]:35053` in this test), both
carrying the same port number. `bugzilla_container_port()` takes the first
line and the text after its last `:`, which is correct on both.

`container_runtime()` and `bugzilla_container_name()` already exist in
`tests/functional/lib.sh`, sourced by `run-tests.sh`; `setup-bugzilla.sh`
currently duplicates equivalent logic inline instead of sourcing `lib.sh`.
This decision has `setup-bugzilla.sh` source `lib.sh` and use the shared
functions — extended with the checkout id and a new
`bugzilla_container_port()` — instead of keeping its own copy; that change
lands in the same PR as this record, not before it.

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
- `run-all-versions.sh` needs no changes: it sequentially calls
  `setup-bugzilla.sh start` then `run-tests.sh` per version, within one
  process, exporting only `BZR_BZ_VERSION`. Neither updated script depends
  on inherited port/name state from that process — each independently
  derives the checkout id from its own path and queries the runtime for the
  port — so looping over versions in one script continues to work
  unchanged.
- `TLS_PORT`/`REDHAT_SHAPE_PORT` in `lib.sh` keep deriving from the resolved
  backend port by fixed offset (`+1000`/`+2000`); no change is needed there,
  since a fixed offset from an already-unique dynamic port stays unique in
  practice. This assumes the runtime never hands back a backend port within
  2000 of the ephemeral range's ceiling — true for every supported CI runner
  and every host checked during this design (`ip_local_port_range` tops out
  at 60999, well under the 65535 the offset could reach), but not proven for
  every possible host configuration; a widened local range remains a latent
  edge case this decision accepts rather than guards against. The offset is
  also a direct, unnegotiated `bind()` with no probe or retry (unlike the
  container's own `-p 80` publish, which asks the kernel for a free port), so
  besides the ceiling case it can collide with any other socket already
  holding that exact port number — another concurrent checkout's own
  container or derived port, or transient ephemeral-connection churn. This
  decision accepts that latent, low-probability risk rather than guarding
  against it, for the same reason the ceiling case is accepted above.
- `AGENTS.md`'s fixed-port caution note (quoted in Context) describes
  behavior this decision removes. It is updated, in the same PR as this
  record, to state the new default — concurrent checkouts no longer need
  manual port/name coordination — so the note does not ship as a stale
  caveat.
- A container left running or stopped by a checkout that is later deleted
  (worktree removed, clone `rm -rf`'d) is no longer discoverable by any other
  checkout, because its name is a function of a path that no longer exists.
  The prior fixed-name scheme gave every checkout the same discovery point,
  so an orphan self-healed the next time anyone ran the suite; that no longer
  holds. An operator notices and prunes by hand (e.g. `podman/docker ps -a
  --filter name=bzr-func-test-`); this decision does not add automatic
  garbage collection for it.

## Considered & rejected

- **Probe a free port with a throwaway socket bind (bind to port 0, read it,
  close it, then run the container on that port).** judgment: leaves a
  TOCTOU window in this script's own logic between the probe and the
  container actually claiming the port; the runtime's own `-p 80` allocation
  has no separate probe-then-claim step for another process to win a race
  against here, and is no more code. (Whether the runtime's own internal
  allocation is atomic against a concurrent unrelated bind is not itself
  verified — see the Decision section — so this rejection rests on the
  narrower, verified property: no probe step exists in this script to race
  against, not on runtime-internal atomicity.)
- **Coordinate the chosen port through a state file** (e.g. under the
  checkout's `.git` directory), written by `setup-bugzilla.sh` and read by
  `run-tests.sh`. judgment: adds a stateful file with its own staleness and
  cleanup concerns; querying the running container's own port mapping is
  always current and needs no cleanup, since the container itself is the
  state.
- **Derive the checkout id from the git branch name instead of the checkout
  path.** judgment: two checkouts on the same branch (two plain clones both
  checked out to the same branch — an ordinary occurrence; a linked
  `git worktree add` to a branch already checked out elsewhere is refused by
  git without `--force` — verified: `git worktree add <path> <branch>`
  against a branch already checked out in this repo fails with `fatal:
  '<branch>' is already used by worktree at '<path>'` — so that variant is
  not itself an ordinary trigger) would still collide; the path is what
  actually distinguishes concurrent checkouts.
- **Do nothing; document that operators must set `BZR_FUNC_PORT`/
  `BZR_FUNC_CONTAINER` by hand per checkout.** judgment: this is the status
  quo the issue is asking to remove; it requires every concurrent invocation
  to be manually coordinated, which is the friction `$campaign`-style
  parallel work hits in practice.
- **Have `lib.sh` itself pick a free port for the TLS/Red Hat-shape proxies**
  — either by having the proxy bind port 0 and report back, or by having
  `lib.sh` probe a free port the same way the rejected socket-probe
  alternative above would for the container — instead of deriving
  `TLS_PORT`/`REDHAT_SHAPE_PORT` by fixed offset. judgment: verified: both
  `tests/functional/tls-proxy.py` and `tests/functional/redhat-shape-proxy.py`
  take their listen port as a required positional argument (`sys.argv[1]`)
  and bind it directly in `main()`; `lib.sh`'s
  `tls_fixture_start`/`redhat_shape_start` already compute a port value and
  pass it as that argument before launching the process, so the
  caller-probes-then-passes-it-in variant needs no proxy-script changes —
  it is not more code than today. The real reason to keep the offset scheme
  is the one already accepted in Consequences above: a caller-side probe
  reintroduces the same TOCTOU window this design otherwise avoids for the
  container's own port, for a residual risk (an offset landing on another
  live socket, or past the ephemeral ceiling) judged acceptably rare, plus
  it keeps the two proxy ports predictable for a human attaching a
  debugger to a known offset.
</content>
