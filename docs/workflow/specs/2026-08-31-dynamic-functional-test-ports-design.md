# Dynamic functional-test container port assignment — design

Issue: [#606](https://github.com/randomparity/bzr/issues/606)
ADR: [0028](../../adr/0028-dynamic-functional-test-container-ports.md)

## Goal

Let functional-test Bugzilla containers run from more than one checkout
(worktree or clone) at the same time, by removing the fixed host port and
fixed container name that currently make a second concurrent invocation fail
outright.

## Architecture

`setup-bugzilla.sh` (container lifecycle) and `run-tests.sh` (test runner)
are separate processes that today agree on a host port and container name
only because both hardcode the same fixed defaults. This change makes both
values dynamic and has each process resolve them independently, using the
running container itself as the shared source of truth:

- **Port**: `setup-bugzilla.sh` asks the container runtime for an
  OS-assigned ephemeral port at `run` time (`-p 80`, no host port);
  `run-tests.sh` later asks the runtime what port it assigned
  (`<runtime> port <name> 80/tcp`). Neither process guesses; both query.
- **Name**: both processes independently compute the same
  `bzr-func-test-<version>-<checkout-id>` name from the checkout's own
  filesystem path, so they agree without communicating.
- **Overrides**: `BZR_FUNC_PORT` and `BZR_FUNC_CONTAINER` continue to pin an
  exact port/name when set, unchanged from today.

## Global Constraints

- Bash (not POSIX sh) — matches every script in `tests/functional/`, which
  already uses `set -euo pipefail` and `[[ ]]`.
- No new external dependency. `cksum` (POSIX) and the container runtime's
  own `port` subcommand (already required for `container_exists`,
  `container_runtime` etc.) cover everything needed.
- Preserve `BZR_FUNC_PORT` / `BZR_FUNC_CONTAINER` / `BZR_FUNC_TIMEOUT`
  override semantics exactly as documented in `setup-bugzilla.sh`'s usage
  text and `run-tests.sh`'s header comment.
- `tests/functional/versions/*/Containerfile` internal `EXPOSE 80` is
  unchanged; only the host-side publish flag changes.

## Components

### `tests/functional/lib.sh` (shared functions)

Already defines `container_runtime()` and `bugzilla_container_name()`
(used today only by `run_bugzilla_sql_file`). Extend it:

- `bugzilla_container_name()` — change its fallback from
  `bzr-func-test-${BZ_VERSION}` to
  `bzr-func-test-${BZ_VERSION}-$(bugzilla_checkout_id)`. The
  `BZR_FUNC_CONTAINER` override path is unchanged.
- New `bugzilla_checkout_id()` — prints the first field of `cksum` of the
  absolute path `"$SCRIPT_DIR/../.."` resolves to, via
  `printf '%s' "$root" | cksum | cut -d' ' -f1` — not a bash here-string
  (`cksum <<<"$root"`), which appends a trailing newline `printf` does not
  and so produces a different checksum. `SCRIPT_DIR` is already set by
  every script that sources `lib.sh` (`tests/functional`'s own absolute
  directory), so this needs no new input.
- New `bugzilla_container_port <runtime> <container>` — prints the host
  port published for the container's `80/tcp`. Checks the *output*
  explicitly rather than trusting the runtime's exit status: podman exits 0
  with empty stdout for a container that exists but is stopped (verified:
  `podman run -d --name p1 -p 80 <image>`, `podman stop p1`, `podman port
  p1 80/tcp` → exit 0, empty stdout/stderr), while docker exits 1 with a
  stderr message for the same case — the two runtimes disagree on exit
  status here, so only the output can be trusted on both:

  ```bash
  bugzilla_container_port() {
      local runtime="$1" container="$2"
      local mapping
      mapping=$("$runtime" port "$container" 80/tcp 2>/dev/null | head -n1)
      [[ -n "$mapping" ]] || return 1
      printf '%s' "${mapping##*:}"
      return 0
  }
  ```

  Returns non-zero (printing nothing) whenever the runtime prints no
  mapping line — container never started, stopped, or removed — so callers
  can distinguish "not running yet" from a resolved port on both runtimes.

### `tests/functional/setup-bugzilla.sh` (container lifecycle)

- Sources `lib.sh` (new `source "$SCRIPT_DIR/lib.sh"` near the top, after
  `SCRIPT_DIR` is computed and before `BZ_VERSION` is used), and drops its
  own inline `CONTAINER_NAME="${BZR_FUNC_CONTAINER:-bzr-func-test-${BZ_VERSION}}"`
  in favor of `CONTAINER_NAME=$(bugzilla_container_name)`.
- Drops `DEFAULT_PORT` entirely (no per-version fixed port is defined
  anymore) but keeps `DEFAULT_TIMEOUT` per version unchanged — timeout
  tuning is unrelated to port assignment.
- Normalizes `BZR_FUNC_PORT="${BZR_FUNC_PORT:-}"` once, alongside the
  existing `CONTAINER_NAME`/`IMAGE_NAME` normalization near the top of the
  script. The script runs under `set -euo pipefail` (unchanged), and every
  later bare `"$BZR_FUNC_PORT"` reference — in `resolve_bz_port` and in
  `cmd_start`'s new-container branch below — depends on this single
  `:-`-guarded assignment having already run; without it, referencing an
  unset `$BZR_FUNC_PORT` aborts the whole script with bash's
  `unbound variable` error on the default (no-override) path, which is
  acceptance criterion #1's exact scenario.
- `BZ_PORT` becomes empty by default and is resolved to a concrete value
  only once the container is confirmed to exist, via a new
  `resolve_bz_port` helper (defined in `setup-bugzilla.sh`, not `lib.sh`,
  since it also owns the error messages specific to this script). It always
  queries the container's actual published port rather than trusting
  `BZR_FUNC_PORT` blindly, so a stale override against an already-running
  container (started earlier without it, or with a different value) fails
  fast with a clear message instead of `wait_for_ready` polling the wrong
  address for the full timeout:

  ```bash
  resolve_bz_port() {
      local actual
      if ! actual=$(bugzilla_container_port "$CONTAINER_RT" "$CONTAINER_NAME"); then
          err "could not determine published port for ${CONTAINER_NAME}"
          return 1
      fi
      if [[ -n "$BZR_FUNC_PORT" && "$BZR_FUNC_PORT" != "$actual" ]]; then
          err "${CONTAINER_NAME} is already running on port ${actual}," \
              "which does not match BZR_FUNC_PORT=${BZR_FUNC_PORT};" \
              "stop it first or unset BZR_FUNC_PORT"
          return 1
      fi
      BZ_PORT="$actual"
      return 0
  }
  ```

- `cmd_start`'s "already running" branch calls `resolve_bz_port` before
  `wait_for_ready` (replacing the removed fixed `BZ_PORT`); this is the
  branch where the mismatch check above actually fires.
- `cmd_start`'s "start a new container" branch publishes with
  `-p "${BZR_FUNC_PORT}:80"` when `BZR_FUNC_PORT` is set, else bare `-p 80`
  (safe under `set -u` because of the top-level normalization above); its
  pre-run `log "Starting container ${CONTAINER_NAME} on port ${BZ_PORT}..."`
  line drops the port mention (`BZ_PORT` is still unresolved at that point)
  — `log "Starting container ${CONTAINER_NAME}..."` — since the actual port
  is only known, and logged, after `resolve_bz_port` runs next: immediately
  after `run -d` succeeds, calls the same `resolve_bz_port` (the container
  now exists and has a port assigned, running or not) before
  `wait_for_ready`.
- `cmd_status` calls `resolve_bz_port` (ignoring a failure — a stopped or
  never-started container has no port to report) before the existing
  `curl` reachability check; skip the `curl` check entirely (report "REST
  API: not reachable") when `resolve_bz_port` fails.
- `wait_for_ready`'s log line and error output already reference `$BZ_PORT`
  by variable, so no change is needed there beyond it now being resolved by
  the caller first.
- Usage/help text and header comment gain one sentence: default port is now
  runtime-assigned; `BZR_FUNC_PORT` still pins an exact one.

### `tests/functional/run-tests.sh` (test runner)

- Drops the `BZ_VERSION`/`DEFAULT_PORT` case statement and the
  `BZ_PORT="${BZR_FUNC_PORT:-$DEFAULT_PORT}"` line (lib.sh, already
  sourced first, sets `BZ_VERSION` itself).
- Replaces them with:

  ```bash
  BZ_PORT="${BZR_FUNC_PORT:-}"
  if [[ -z "$BZ_PORT" ]]; then
      _bz_runtime=$(container_runtime) || {
          echo "ERROR: neither podman nor docker found in PATH" >&2
          exit 1
      }
      _bz_container=$(bugzilla_container_name)
      BZ_PORT=$(bugzilla_container_port "$_bz_runtime" "$_bz_container") || {
          echo "ERROR: could not determine Bugzilla container port for" \
              "'$_bz_container'; is it running?" \
              "(tests/functional/setup-bugzilla.sh start)" >&2
          exit 1
      }
  fi
  BZ_URL="http://127.0.0.1:${BZ_PORT}"
  ```

### `tests/functional/run-all-versions.sh`

No change. It never references a port directly; it already delegates to
`setup-bugzilla.sh start` and `run-tests.sh` per version, both of which now
resolve the port themselves.

### `tests/functional/keyring-test.sh`

No change. It does not start or address the Bugzilla container.

### `tests/functional/README.md`

Its Environment Variables table (currently) documents fixed literal
defaults that this change removes:

- `BZR_FUNC_PORT` row: change the `Default` cell from `8089` to
  `(runtime-assigned)`, and the description to note it overrides the
  default rather than replacing a fixed one.
- `BZR_FUNC_CONTAINER` row: change the `Default` cell from `bzr-func-test`
  (already stale against the current `bzr-func-test-${BZ_VERSION}`) to
  `bzr-func-test-<version>-<checkout-id>`.
- `BZR_FUNC_TLS_PORT` / `BZR_FUNC_REDHAT_PORT` rows: change `BZR_FUNC_PORT +
  1000`/`+ 2000` to `(resolved backend port) + 1000`/`+ 2000` — these were
  always derived from the resolved backend port, not the env var literally,
  and that distinction now matters once the backend port has no fixed
  default.
- Troubleshooting → **Port conflict** section: replace "Change the port
  with `BZR_FUNC_PORT=9089`" with a note that port conflicts no longer
  occur by default (each run gets a runtime-assigned port); `BZR_FUNC_PORT`
  is still available to pin an exact one when needed.

### `AGENTS.md` (`CLAUDE.md` is a symlink to it)

Update the existing caution paragraph:

> Functional tests start Bugzilla containers with fixed ports. Be cautious
> when running multiple workflows simultaneously as they may interfere with
> each other during this test phase.

to state that concurrent worktrees/clones now get distinct container names
and runtime-assigned ports automatically, and note the one remaining
caveat: two invocations from the exact same checkout directory at the same
time still collide.

## Error Handling

- Container runtime missing (`neither podman nor docker found`): unchanged,
  already handled at script top in both scripts.
- Port not yet resolvable (container never started, or `run` failed before
  a name was assigned): `setup-bugzilla.sh` reports it via `err` and exits
  non-zero (`cmd_start`), or degrades `cmd_status`'s REST reachability line
  to "not reachable" without crashing. `run-tests.sh` exits 1 with an
  actionable message pointing at `setup-bugzilla.sh start`.
- Runtime `port` subcommand producing unexpected output (e.g. no mapping
  line at all): `bugzilla_container_port` returns non-zero; callers treat
  that the same as "not running."

## Testing

This is functional-test-harness tooling, not `bzr` CLI surface — the
`AGENTS.md` "user-facing change needs a phase script" rule does not apply
(no command, flag, or output shape of the compiled `bzr` binary changes).
The applicable rule is the "any other change" one: a full functional run
must be green before the PR opens.

Acceptance criteria, each independently checkable:

1. `make functional-test` (default `bz50`) passes end to end with no
   `BZR_FUNC_PORT`/`BZR_FUNC_CONTAINER` set — proves the dynamic default
   path works for the common case.
2. `BZR_FUNC_PORT=18089 make functional-test` passes and the container is
   actually reachable on `18089` (`curl http://127.0.0.1:18089/rest/version`
   succeeds while the container is up) — proves the override path is
   unchanged.
3. Two concurrent checkouts of the repository (e.g. a `git worktree add`
   sibling directory) each run `tests/functional/setup-bugzilla.sh start`
   for `bz50` at the same time without either failing with a name or port
   collision, and `tests/functional/setup-bugzilla.sh status` in each
   checkout reports its own container's own port. This is the scenario
   issue #606 describes; it is exercised manually as part of this change
   (not part of the automated phase suite, which does not run multiple
   checkouts) and the manual steps and result are recorded in the PR
   description.
4. `make functional-test-all` (all three versions, sequential, one
   checkout) still passes — proves per-version container names/ports stay
   distinct within a single checkout as they always have.
5. `tests/functional/setup-bugzilla.sh status` against a stopped/removed
   container still reports "does not exist" (unchanged path, not touched by
   this change) rather than erroring.

No new automated phase script is added under `tests/functional/phases/`:
those phases exercise `bzr` CLI behavior against a running server, and this
change does not alter any `bzr` command's behavior — only how the test
harness stands the server up.
</content>
