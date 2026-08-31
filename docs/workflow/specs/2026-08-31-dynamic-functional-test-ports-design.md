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
  port published for the container's `80/tcp`, by running
  `<runtime> port <container> 80/tcp`, taking the first line, and printing
  the text after the last `:`. Returns non-zero (printing nothing) if the
  runtime command fails or prints nothing, so callers can distinguish "not
  running yet" from a resolved port.

### `tests/functional/setup-bugzilla.sh` (container lifecycle)

- Sources `lib.sh` (new `source "$SCRIPT_DIR/lib.sh"` near the top, after
  `SCRIPT_DIR` is computed and before `BZ_VERSION` is used), and drops its
  own inline `CONTAINER_NAME="${BZR_FUNC_CONTAINER:-bzr-func-test-${BZ_VERSION}}"`
  in favor of `CONTAINER_NAME=$(bugzilla_container_name)`.
- Drops `DEFAULT_PORT` entirely (no per-version fixed port is defined
  anymore) but keeps `DEFAULT_TIMEOUT` per version unchanged — timeout
  tuning is unrelated to port assignment.
- `BZ_PORT` becomes empty by default (`BZ_PORT="${BZR_FUNC_PORT:-}"`) and is
  resolved to a concrete value only once the container is confirmed to be
  running, via a new `resolve_bz_port` helper (defined in
  `setup-bugzilla.sh`, not `lib.sh`, since it also owns the "not running
  yet" error message specific to this script):

  ```bash
  resolve_bz_port() {
      if [[ -n "$BZR_FUNC_PORT" ]]; then
          BZ_PORT="$BZR_FUNC_PORT"
          return 0
      fi
      if ! BZ_PORT=$(bugzilla_container_port "$CONTAINER_RT" "$CONTAINER_NAME"); then
          err "could not determine published port for ${CONTAINER_NAME}"
          return 1
      fi
      return 0
  }
  ```

- `cmd_start`'s "already running" branch calls `resolve_bz_port` before
  `wait_for_ready` (replacing the removed fixed `BZ_PORT`).
- `cmd_start`'s "start a new container" branch publishes with
  `-p "${BZR_FUNC_PORT}:80"` when `BZR_FUNC_PORT` is set, else bare `-p 80`;
  immediately after `run -d` succeeds, calls `resolve_bz_port` (the
  container now exists and has a port assigned, running or not) before
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
