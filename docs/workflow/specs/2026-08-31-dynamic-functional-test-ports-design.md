# Dynamic functional-test container port assignment — design

Issue: [#606](https://github.com/randomparity/bzr/issues/606)
ADR: [0030](../../adr/0030-dynamic-functional-test-container-ports.md)

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

- Bash (not POSIX sh) — matches every script in `tests/functional/`.
  `setup-bugzilla.sh`, `run-tests.sh`, and `keyring-test.sh` use
  `set -euo pipefail`; `run-all-versions.sh` uses `set -uo pipefail` (no
  `-e` — it handles each version's failure explicitly via if/exit-code
  checks) and is unchanged by this design.
- No new external dependency. `cksum` (POSIX) and the container runtime's
  own `port` subcommand (already required for `container_exists`,
  `container_runtime` etc.) cover everything needed.
- Preserve `BZR_FUNC_PORT` / `BZR_FUNC_CONTAINER` / `BZR_FUNC_TIMEOUT`
  override semantics exactly as documented in `setup-bugzilla.sh`'s usage
  text and `run-tests.sh`'s header comment.
- `tests/functional/versions/*/Containerfile` internal `EXPOSE 80` is
  unchanged; only the host-side publish flag changes.

## Components

### `tests/functional/versions/*/Containerfile` (header comment only)

Each Containerfile's `# Run:` header comment documents the exact manual
invocation this design removes, e.g. (`bz50`; `bz52`/`bz53` are the same
pattern with their own version-specific name/port):

```
# Run:    podman run -d --name bzr-func-test-bz50 -p 8089:80 localhost/bzr-func-bz50:latest  (or docker)
```

Update it to point at the supported entry point instead of a fixed
name/port a maintainer could otherwise copy-paste and recreate the
multi-checkout collision issue #606 removes:

```
# Run:    tests/functional/setup-bugzilla.sh start   (BZR_BZ_VERSION=bz50 if run outside the make target)
```

No other content in these files changes (`EXPOSE 80` and everything else
is untouched, per Global Constraints above).

### `tests/functional/container-env.sh` (new: side-effect-free shared functions)

`container_runtime()` and `bugzilla_container_name()` currently live in
`lib.sh`, which also unconditionally creates three `mktemp` temp files and
installs `trap _cleanup_tmpfiles EXIT` as soon as it is sourced (existing
`lib.sh` lines ~107-118) — side effects only `run-tests.sh`'s phase-file
callers need. `tools/record-demo.sh` needs only the container-lookup
functions, and bash `EXIT` traps do not stack: a later `trap ... EXIT`
installed by `record-demo.sh` (it installs one in each of its four
`--drive*` orchestrator branches) silently replaces `lib.sh`'s trap, so
`_cleanup_tmpfiles` never runs and its three temp files leak on every
invocation (verified: reproduced the exact trap-clobbering mechanism in an
isolated script matching this structure — the first trap's cleanup command
does not fire and its temp file remains on disk after exit).

So this design adds a new file, `tests/functional/container-env.sh`,
holding the four container-lookup functions with **no source-time side
effects** (no `mktemp`, no `trap`), and both `lib.sh` and
`tools/record-demo.sh` source it directly instead of `record-demo.sh`
sourcing the whole of `lib.sh`:

- `BZ_VERSION="${BZR_BZ_VERSION:-bz50}"` — an ordinary variable default
  (not a `mktemp`/`trap` side effect), moved here from `lib.sh`'s own top
  so every direct sourcer of `container-env.sh` is self-sufficient.
  Without this, `bugzilla_container_name()`'s bare `"${BZ_VERSION}"`
  reference is only safe for callers that source `lib.sh` in full (which
  sets it); `tools/record-demo.sh` sources `container-env.sh` directly and
  never sets `BZ_VERSION` itself, so under `set -u` every normal invocation
  (verified: reproduced the `unbound variable` abort) would fail before
  ever reaching the container lookup. `lib.sh`'s own identical assignment
  becomes redundant but harmless once it also sources this file.
- `container_runtime()` — moved here unchanged from `lib.sh`.
- `bugzilla_container_name()` — moved here, extended (see below).
- New `bugzilla_checkout_id()`.
- New `bugzilla_container_port()`.

`lib.sh` sources `tests/functional/container-env.sh` near its own top (so
existing callers of `container_runtime()`/`bugzilla_container_name()`, e.g.
`run_bugzilla_sql_file`, are unaffected) and keeps its `mktemp`/`trap`
side effects exactly as they are today — only `setup-bugzilla.sh` and
`run-tests.sh` need those, and both already source `lib.sh` in full.

- `bugzilla_container_name()` — change its fallback to compute the checkout
  id in a separate statement first, not embedded inside the `${VAR:-...}`
  default directly — a command substitution used only as a parameter
  default does not propagate a failing/aborting inner command's status
  through `set -e` the way a plain assignment does (verified: an inner
  `bugzilla_checkout_id()` failure, e.g. from an unset `SCRIPT_DIR`, was
  silently swallowed when embedded as `${VAR:-bzr-func-test-...-$(cmd)}`,
  producing a garbage name instead of aborting):

  ```bash
  bugzilla_container_name() {
      if [[ -n "${BZR_FUNC_CONTAINER:-}" ]]; then
          printf '%s' "$BZR_FUNC_CONTAINER"
          return 0
      fi
      local id
      id=$(bugzilla_checkout_id) || return 1
      printf '%s' "bzr-func-test-${BZ_VERSION}-${id}"
      return 0
  }
  ```

- New `bugzilla_checkout_id()` — prints the first field of `cksum` of the
  absolute path `"$SCRIPT_DIR/../.."` resolves to, via
  `printf '%s' "$root" | cksum | cut -d' ' -f1` — not a bash here-string
  (`cksum <<<"$root"`), which appends a trailing newline `printf` does not
  and so produces a different checksum. `SCRIPT_DIR` must be set by the
  caller before sourcing `container-env.sh` (directly, or transitively via
  `lib.sh`) — `setup-bugzilla.sh` and `run-tests.sh` already set it before
  sourcing `lib.sh`; `tools/record-demo.sh`'s new sourcing block below must
  set it too, since it doesn't otherwise need `SCRIPT_DIR` for anything.
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
      mapping=$("$runtime" port "$container" 80/tcp | head -n1)
      [[ -n "$mapping" ]] || return 1
      printf '%s' "${mapping##*:}"
      return 0
  }
  ```

  Deliberately does not redirect the runtime's stderr to `/dev/null`: only
  stdout is piped through `head -n1`, so a runtime error distinct from "not
  running" (permission denied, daemon unreachable) still reaches the
  caller's stderr alongside `resolve_bz_port`'s and `run-tests.sh`'s own
  generic "could not determine port" message, instead of being silently
  discarded.

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
          return 2
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
- `cmd_status` calls `resolve_bz_port` before the existing `curl`
  reachability check and distinguishes its two failure exit codes rather
  than treating both as "not reachable". A bare `resolve_bz_port` call
  followed by a `case $?` under `set -e` aborts the script before the case
  statement runs (verified: reproduced this exact abort), so the exit code
  must be captured explicitly first, the same `cmd || rc=$?` idiom `lib.sh`
  already uses elsewhere in this codebase:

  ```bash
  local port_status=0
  resolve_bz_port || port_status=$?
  case "$port_status" in
  0)
      if curl -sf "http://127.0.0.1:${BZ_PORT}/rest/version" >/dev/null 2>&1; then
          echo "REST API: reachable"
      else
          echo "REST API: not reachable"
      fi
      ;;
  2)
      echo "REST API: unknown — BZR_FUNC_PORT does not match the running container's actual port"
      ;;
  *)
      echo "REST API: not reachable"
      ;;
  esac
  ```

  Exit `1` (no port mapping at all — stopped or never started) and any
  other non-zero, non-`2` status fall through to "not reachable", matching
  today's behavior; exit `2` (a `BZR_FUNC_PORT` mismatch against an
  actually-running container) reports the mismatch explicitly instead,
  since the container may well be reachable at its real port even though
  the requested one is wrong.
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
      _bz_container=$(bugzilla_container_name) || {
          echo "ERROR: could not derive the Bugzilla container name" >&2
          exit 1
      }
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

### `tools/record-demo.sh`

Out of the issue's original named surface, but added here: it hardcodes
`BZ_URL=${BZ_URL:-http://127.0.0.1:8089}` (line 22), which stops pointing at
the running container the moment `setup-bugzilla.sh` stops defaulting to a
fixed port — a real, silent regression for this actively-used dev tool, not
covered by any automated test. Fixed by deriving the default the same way
`run-tests.sh` does, instead of a literal:

```bash
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if [[ -z "${BZ_URL:-}" ]]; then
    SCRIPT_DIR="$REPO_ROOT/tests/functional"
    # shellcheck source=/dev/null
    source "$SCRIPT_DIR/container-env.sh"
    _rt=$(container_runtime) || {
        echo "ERROR: neither podman nor docker found in PATH" >&2
        exit 1
    }
    _name=$(bugzilla_container_name) || {
        echo "ERROR: could not derive the Bugzilla container name" >&2
        exit 1
    }
    _port=$(bugzilla_container_port "$_rt" "$_name") || {
        echo "ERROR: could not determine Bugzilla container port for" \
            "'$_name'; run: make functional-start" >&2
        exit 1
    }
    BZ_URL="http://127.0.0.1:${_port}"
fi
```

replacing the current unconditional `BZ_URL=${BZ_URL:-http://127.0.0.1:8089}`
line. An explicit `BZ_URL` (or the existing `BZR_FUNC_PORT`, indirectly, if
an operator sets it before running `make functional-start`) still overrides
this, unchanged from today's `${BZ_URL:-...}` pattern.

This block replaces the current line 22, which sits before the script's
`--drive`/`--drive-weekly-status`/`--drive-dependency-analysis`/
`--drive-release-readiness`/`--drive-project-manager-reporting` dispatch
checks (each launched by `asciinema` as a fresh subprocess). None of those
driver branches reads `$BZ_URL`, so keeping this block ahead of them makes
every driver invocation depend on a container lookup it doesn't need.
Move the whole block to just after the dispatch checks (immediately before
the block that actually uses `$BZ_URL`), so a driver subprocess launch
never depends on it.

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

Acceptance criteria, each independently checkable — run
`tests/functional/setup-bugzilla.sh stop` between criteria 1 and 2:
`make functional-test` does not stop the container afterward, and
`resolve_bz_port`'s stale-override check (working as designed) rejects
criterion 2's `BZR_FUNC_PORT` against the container criterion 1 left running
under the default name if the two are run back to back without stopping it
first.

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
6. `bugzilla_checkout_id()` (and by extension `bugzilla_container_name()`)
   actually varies with `SCRIPT_DIR`: sourcing `container-env.sh` with two
   different fabricated `SCRIPT_DIR` values in the same shell yields two
   different ids. `set -u` alone is now sufficient to catch a missing
   `SCRIPT_DIR` (e.g. `bash -c 'set -u; source tests/functional/container-env.sh;
   bugzilla_checkout_id'` with `SCRIPT_DIR` unset asserts a nonzero exit):
   `root=$(cd "$SCRIPT_DIR/../.." && pwd -P) || return 1` fails the nounset
   error inside the command substitution and the explicit `|| return 1`
   propagates it out of the plain assignment, without needing `set -e` to do
   so (verified: rc=1 with empty output under `set -u` alone). This is a
   narrow, automatable check (no container needed) that catches a regression
   in the derivation logic itself — the class of bug this design's own review
   caught in the `${VAR:-$(cmd)}` masking case — without needing the manual
   two-checkout exercise in criterion 3 to be run on every change.

No new automated phase script is added under `tests/functional/phases/`:
those phases exercise `bzr` CLI behavior against a running server, and this
change does not alter any `bzr` command's behavior — only how the test
harness stands the server up.
</content>
