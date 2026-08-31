# Dynamic functional-test container port assignment — implementation plan

**Goal:** Replace the fixed host port and fixed container name that
`tests/functional/setup-bugzilla.sh` and `tests/functional/run-tests.sh` use
today with a runtime-assigned port and a checkout-scoped container name, so
concurrent checkouts (worktrees, clones) can run the functional-test suite at
the same time without colliding.

**Architecture:** `setup-bugzilla.sh` publishes the container on `-p 80` (an
OS-assigned ephemeral port) instead of a fixed `8089`/`8090`/`8091`, under a
name suffixed with a `cksum` of the checkout's own absolute path instead of a
bare `bzr-func-test-<version>`. `run-tests.sh` and `tools/record-demo.sh`
independently recompute the same name and query the running container's
actual published port — the container itself is the shared state, so the two
processes never need to pass anything between them directly.

**Tech stack:** Bash (`set -euo pipefail` in every touched script except
`run-all-versions.sh`, which is untouched), `podman`/`docker` CLI, `cksum`
(POSIX, no new dependency).

**Global Constraints** (transcribed from the design spec,
`docs/workflow/specs/2026-08-31-dynamic-functional-test-ports-design.md`):

- Bash (not POSIX sh) — matches every script in `tests/functional/`.
  `setup-bugzilla.sh`, `run-tests.sh`, `keyring-test.sh`, and
  `tools/record-demo.sh` use `set -euo pipefail`; `run-all-versions.sh` uses
  `set -uo pipefail` (no `-e`) and is unchanged by this plan.
- No new external dependency. `cksum` and the container runtime's own `port`
  subcommand cover everything needed.
- Preserve `BZR_FUNC_PORT` / `BZR_FUNC_CONTAINER` / `BZR_FUNC_TIMEOUT`
  override semantics exactly as documented today.
- `tests/functional/versions/*/Containerfile`'s internal `EXPOSE 80` is
  unchanged; only the header comment and the host-side publish flag change.
- ADR: `docs/adr/0028-dynamic-functional-test-container-ports.md` (Accepted).
  Design: `docs/workflow/specs/2026-08-31-dynamic-functional-test-ports-design.md`.

Every task below binds to these constraints; they are not repeated per task.

---

## Task 1 — Add `tests/functional/container-env.sh`

**Creates:** `tests/functional/container-env.sh` (new file).

**Interfaces this task defines** (consumed by Tasks 2, 3, 4):

- `BZ_VERSION` — global, set to `"${BZR_BZ_VERSION:-bz50}"` if not already
  set by an earlier `source`.
- `container_runtime()` — prints `podman` or `docker` to stdout, returns 1 if
  neither is on `PATH`.
- `bugzilla_checkout_id()` — prints a short numeric id derived from
  `SCRIPT_DIR`'s parent-of-parent absolute path; returns 1 if `SCRIPT_DIR` is
  unset (under `set -u`, referencing it aborts the caller first).
- `bugzilla_container_name()` — prints `$BZR_FUNC_CONTAINER` if set, else
  `bzr-func-test-${BZ_VERSION}-<checkout-id>`; returns 1 if
  `bugzilla_checkout_id` fails.
- `bugzilla_container_port <runtime> <container>` — prints the host port
  published for the container's `80/tcp`; returns 1 if the runtime reports no
  mapping.

**Where this fits:** `tests/functional/lib.sh` currently defines
`container_runtime()` and `bugzilla_container_name()` inline and installs
`mktemp`/`trap` side effects as soon as it is sourced. `tools/record-demo.sh`
(Task 4) needs the container-lookup functions but must not inherit those side
effects — sourcing all of `lib.sh` there clobbers its own `EXIT` traps (bash
traps don't stack) and leaks `lib.sh`'s temp files. This task carves the four
functions out into their own file with no source-time side effects, so both
`lib.sh` (Task 2) and `tools/record-demo.sh` (Task 4) can source it safely.

### Steps

1. Write the failing check. Run:

   ```sh
   bash -c 'source tests/functional/container-env.sh' 2>&1
   ```

   Expected: `tests/functional/container-env.sh: No such file or directory`
   (the file does not exist yet).

2. Create `tests/functional/container-env.sh`:

   ```bash
   #!/bin/bash
   # Container-lookup helpers shared by the functional-test lifecycle
   # scripts and tools/record-demo.sh. Source this file; do not execute
   # directly. No source-time side effects (no mktemp, no trap) — callers
   # that need those (lib.sh) add them separately.

   BZ_VERSION="${BZ_VERSION:-${BZR_BZ_VERSION:-bz50}}"

   container_runtime() {
       if command -v podman >/dev/null 2>&1; then
           printf '%s' podman
           return 0
       fi
       if command -v docker >/dev/null 2>&1; then
           printf '%s' docker
           return 0
       fi
       return 1
   }

   # bugzilla_checkout_id — short numeric id derived from this checkout's
   # own absolute path (tests/functional's parent-of-parent), so concurrent
   # checkouts (worktrees, clones) get distinct ids without coordinating.
   # SCRIPT_DIR must be set by the caller before sourcing this file.
   bugzilla_checkout_id() {
       local root
       root=$(cd "$SCRIPT_DIR/../.." && pwd)
       printf '%s' "$root" | cksum | cut -d' ' -f1
       return 0
   }

   # bugzilla_container_name — $BZR_FUNC_CONTAINER if set, else a name
   # scoped to this checkout and Bugzilla version.
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

   # bugzilla_container_port <runtime> <container> — host port published
   # for the container's 80/tcp, or non-zero (printing nothing) if the
   # runtime reports no mapping (container never started, stopped, or
   # removed). Checks output, not exit status: podman exits 0 with empty
   # stdout for a stopped container; docker exits 1 with a stderr message
   # for the same case. Does not redirect stderr, so a runtime error
   # distinct from "not running" still reaches the caller.
   bugzilla_container_port() {
       local runtime="$1" container="$2"
       local mapping
       mapping=$("$runtime" port "$container" 80/tcp | head -n1)
       [[ -n "$mapping" ]] || return 1
       printf '%s' "${mapping##*:}"
       return 0
   }
   ```

3. Confirm it fails correctly (`bugzilla_checkout_id` needs `SCRIPT_DIR`):

   ```sh
   bash -c 'set -eu; source tests/functional/container-env.sh; bugzilla_checkout_id'
   echo "exit: $?"
   ```

   Expected: aborts with `SCRIPT_DIR: unbound variable` and a non-zero exit
   (acceptance criterion 6's negative case).

4. Confirm the positive case and the two-different-paths-yield-two-different-ids
   property. Use real directories — `cd "$SCRIPT_DIR/../.."` silently produces
   the empty string (and thus the same fixed checksum) for a path that does not
   exist, so an illustrative nonexistent path would defeat this check:

   ```sh
   a=$(mktemp -d); mkdir -p "$a/tests/functional"
   b=$(mktemp -d); mkdir -p "$b/tests/functional"
   bash -c "SCRIPT_DIR=$a/tests/functional; source tests/functional/container-env.sh; bugzilla_checkout_id"
   echo
   bash -c "SCRIPT_DIR=$b/tests/functional; source tests/functional/container-env.sh; bugzilla_checkout_id"
   rm -rf "$a" "$b"
   ```

   Expected: two different numeric ids printed (acceptance criterion 6's
   positive case). Two runs with the **same** `SCRIPT_DIR` value must print
   the same id (determinism).

5. Confirm `bugzilla_container_name()` composes correctly:

   ```sh
   c=$(mktemp -d); mkdir -p "$c/tests/functional"
   bash -c "SCRIPT_DIR=$c/tests/functional; source tests/functional/container-env.sh; bugzilla_container_name"
   echo
   BZR_FUNC_CONTAINER=pinned-name bash -c "SCRIPT_DIR=$c/tests/functional; source tests/functional/container-env.sh; bugzilla_container_name"
   rm -rf "$c"
   ```

   Expected: first prints `bzr-func-test-bz50-<id>`; second prints
   `pinned-name` (override honored).

6. Confirm `bugzilla_container_port()` against a real container (requires
   podman or docker):

   ```sh
   RT=$(command -v podman >/dev/null 2>&1 && echo podman || echo docker)
   "$RT" run -d --name bz-port-test -p 80 docker.io/library/busybox:latest sleep 60
   bash -c "source tests/functional/container-env.sh; bugzilla_container_port $RT bz-port-test"
   echo
   "$RT" stop bz-port-test >/dev/null
   bash -c "source tests/functional/container-env.sh; bugzilla_container_port $RT bz-port-test; echo \"exit: \$?\""
   "$RT" rm -f bz-port-test >/dev/null
   ```

   Expected: first invocation prints a numeric port and exits 0; second
   (stopped container) prints nothing and exits non-zero.

7. `git add tests/functional/container-env.sh && git commit` with message
   `feat(functional): add side-effect-free container lookup helpers`.

**Acceptance criteria for this task:**

- `tests/functional/container-env.sh` exists, is syntactically valid bash
  (`bash -n tests/functional/container-env.sh` exits 0), and defines exactly
  the four items listed under Interfaces.
- Steps 3–6 above all produce their documented expected output.
- No `mktemp` or `trap` call anywhere in the file.

---

## Task 2 — Update `tests/functional/lib.sh` to source `container-env.sh`

**Modifies:** `tests/functional/lib.sh`.

**Interfaces consumed:** Task 1's `container-env.sh` (all four items).

**Interfaces preserved:** `run_bugzilla_sql_file()` (existing, lines
486–493) calls `container_runtime()` and `bugzilla_container_name()` — both
must keep working identically after this change.

### Steps

1. Write the failing check — confirm the current duplication exists:

   ```sh
   grep -n "^container_runtime()" tests/functional/lib.sh
   ```

   Expected: one match (the inline definition at the current line 466).

2. Edit `tests/functional/lib.sh`:
   - Remove the `# ── Version ──` block's own `BZ_VERSION="${BZR_BZ_VERSION:-bz50}"`
     line (current line 6) — this now comes from `container-env.sh`.
   - Immediately after the file's opening comment block (after current line
     3, before the removed `BZ_VERSION` line), add:

     ```bash
     SCRIPT_DIR="${SCRIPT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
     # shellcheck source=tests/functional/container-env.sh
     source "$SCRIPT_DIR/container-env.sh"
     ```

     The `${SCRIPT_DIR:-...}` guard means `lib.sh` self-derives `SCRIPT_DIR`
     when a caller hasn't already set it (defensive; every current caller
     already sets it before sourcing `lib.sh`, so this is a no-op fallback,
     not a new contract).
   - Delete the existing inline `container_runtime()` function definition
     (current lines 466–476).
   - Delete the existing inline `bugzilla_container_name()` function
     definition (current lines 478–481) — the version in `container-env.sh`
     replaces it with identical external behavior for
     `run_bugzilla_sql_file`'s existing call (it only ever calls it with
     `BZ_VERSION` already set from the module-level source, so the
     `BZR_FUNC_CONTAINER`-or-checkout-id fallback is unaffected from that
     caller's point of view — it never depended on the old
     `bzr-func-test-${BZ_VERSION}` fallback's literal shape, only that it's
     a deterministic name).

3. Confirm no duplicate definitions remain:

   ```sh
   grep -c "^container_runtime()\|^bugzilla_container_name()" tests/functional/lib.sh
   ```

   Expected: `0` (both now live only in `container-env.sh`).

4. Confirm `run_bugzilla_sql_file`'s dependencies still resolve:

   ```sh
   bash -c 'SCRIPT_DIR=/tmp/checkout-c/tests/functional; source tests/functional/lib.sh; type run_bugzilla_sql_file container_runtime bugzilla_container_name'
   ```

   Expected: all three print as shell functions (no "not found" errors).

5. `bash -n tests/functional/lib.sh` exits 0.

6. `git add tests/functional/lib.sh && git commit` with message
   `refactor(functional): source container-env.sh from lib.sh`.

**Acceptance criteria for this task:** `lib.sh` no longer defines
`container_runtime` or `bugzilla_container_name` itself; both resolve
correctly through the new `source`; `run_bugzilla_sql_file` is unaffected
(same call sites, same signature).

---

## Task 3 — Update `tests/functional/setup-bugzilla.sh`

**Modifies:** `tests/functional/setup-bugzilla.sh`.

**Interfaces consumed:** Task 1/2's `container_runtime`,
`bugzilla_container_name`, `bugzilla_container_port` (via `source
"$SCRIPT_DIR/lib.sh"`, which now transitively sources `container-env.sh`).

**Interfaces defined:** `resolve_bz_port()` — sets the global `BZ_PORT`;
returns 0 on success, 1 if no port is published for `$CONTAINER_NAME`, 2 if
`$BZR_FUNC_PORT` is set but disagrees with the actual published port. Used
by `cmd_start` and `cmd_status` later in this same file.

### Steps

1. Write the failing check — confirm today's fixed-port behavior:

   ```sh
   grep -n 'DEFAULT_PORT=8089\|CONTAINER_NAME="\${BZR_FUNC_CONTAINER' tests/functional/setup-bugzilla.sh
   ```

   Expected: matches the current fixed `DEFAULT_PORT`/`CONTAINER_NAME`
   lines (current lines 24, 41).

2. Edit `tests/functional/setup-bugzilla.sh`:

   a. After the existing `SCRIPT_DIR="$(cd ... && pwd)"` line (current line
      7), add:

      ```bash
      # shellcheck source=tests/functional/lib.sh
      source "$SCRIPT_DIR/lib.sh"
      ```

   b. In the `case "$BZ_VERSION" in ... esac` block (current lines 22–39),
      remove every `DEFAULT_PORT=...` assignment (keep `DEFAULT_TIMEOUT`
      unchanged in each branch):

      ```bash
      case "$BZ_VERSION" in
      bz50)
          DEFAULT_TIMEOUT=90
          ;;
      bz52)
          DEFAULT_TIMEOUT=240
          ;;
      bz53)
          DEFAULT_TIMEOUT=240
          ;;
      *)
          echo "ERROR: Unknown BZR_BZ_VERSION=$BZ_VERSION (expected bz50, bz52, or bz53)" >&2
          exit 1
          ;;
      esac
      ```

   c. Replace the current lines 41–44:

      ```bash
      CONTAINER_NAME="${BZR_FUNC_CONTAINER:-bzr-func-test-${BZ_VERSION}}"
      IMAGE_NAME="${BZR_FUNC_IMAGE:-localhost/bzr-func-${BZ_VERSION}:latest}"
      BZ_PORT="${BZR_FUNC_PORT:-$DEFAULT_PORT}"
      HEALTH_TIMEOUT="${BZR_FUNC_TIMEOUT:-$DEFAULT_TIMEOUT}"
      ```

      with:

      ```bash
      BZR_FUNC_PORT="${BZR_FUNC_PORT:-}"
      CONTAINER_NAME=$(bugzilla_container_name) || {
          echo "ERROR: [$BZ_VERSION] could not derive the Bugzilla container name" >&2
          exit 1
      }
      IMAGE_NAME="${BZR_FUNC_IMAGE:-localhost/bzr-func-${BZ_VERSION}:latest}"
      BZ_PORT=""
      HEALTH_TIMEOUT="${BZR_FUNC_TIMEOUT:-$DEFAULT_TIMEOUT}"
      ```

      This call site sits at the script's top level, before `err()` is
      defined in the `# ── Helpers ──` block below — unlike `resolve_bz_port`
      and the other `err` call sites, which only run once the case-statement
      dispatcher below invokes a function, by which point `err` has already
      been defined by normal top-to-bottom execution. A bare `echo ... >&2`
      here matches the pattern the file's own existing top-level failure
      paths (container-runtime detection, the version case statement's `*)`
      branch above) already use for the same reason.

   d. In the `# ── Helpers ──` section, immediately after the `wait_for_ready`
      function (current lines 57–76), add:

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

   e. In `cmd_start()`, the "already running" branch currently reads
      (current lines 104–108):

      ```bash
          if $CONTAINER_RT inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null | grep -q true; then
              log "Container is already running."
              wait_for_ready
              return 0
          fi
      ```

      Change to:

      ```bash
          if $CONTAINER_RT inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null | grep -q true; then
              log "Container is already running."
              resolve_bz_port || exit 1
              wait_for_ready
              return 0
          fi
      ```

   f. `cmd_start()`'s container-start block currently reads (current lines
      119–125):

      ```bash
          log "Starting container ${CONTAINER_NAME} on port ${BZ_PORT}..."
          $CONTAINER_RT run -d \
              --name "$CONTAINER_NAME" \
              -p "${BZ_PORT}:80" \
              "$IMAGE_NAME"

          wait_for_ready
      ```

      Change to:

      ```bash
          log "Starting container ${CONTAINER_NAME}..."
          if [[ -n "$BZR_FUNC_PORT" ]]; then
              $CONTAINER_RT run -d \
                  --name "$CONTAINER_NAME" \
                  -p "${BZR_FUNC_PORT}:80" \
                  "$IMAGE_NAME"
          else
              $CONTAINER_RT run -d \
                  --name "$CONTAINER_NAME" \
                  -p 80 \
                  "$IMAGE_NAME"
          fi

          resolve_bz_port || exit 1
          log "Container listening on host port ${BZ_PORT}."
          wait_for_ready
      ```

   g. `cmd_status()` currently reads (current lines 135–150):

      ```bash
      cmd_status() {
          if container_exists; then
              $CONTAINER_RT inspect --format \
                  'Name: {{.Name}}  State: {{.State.Status}}  Running: {{.State.Running}}  Pid: {{.State.Pid}}' \
                  "$CONTAINER_NAME"
              # Check REST API
              if curl -sf "http://127.0.0.1:${BZ_PORT}/rest/version" >/dev/null 2>&1; then
                  echo "REST API: reachable"
              else
                  echo "REST API: not reachable"
              fi
          else
              echo "Container ${CONTAINER_NAME} does not exist."
              return 1
          fi
      }
      ```

      Change the body of the `if container_exists; then` branch to:

      ```bash
          if container_exists; then
              $CONTAINER_RT inspect --format \
                  'Name: {{.Name}}  State: {{.State.Status}}  Running: {{.State.Running}}  Pid: {{.State.Pid}}' \
                  "$CONTAINER_NAME"
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
          else
              echo "Container ${CONTAINER_NAME} does not exist."
              return 1
          fi
      ```

   h. Update the usage text (current lines 179–183):

      ```bash
      *)
          echo "Usage: $0 {build|start|stop|status|reset|logs}"
          echo "  Set BZR_BZ_VERSION=bz50|bz52|bz53 (default: bz50)"
          exit 1
          ;;
      ```

      Add one line after the `BZR_BZ_VERSION` line:

      ```bash
          echo "  Host port is runtime-assigned by default; set BZR_FUNC_PORT to pin one"
      ```

3. Confirm the whole file still parses: `bash -n tests/functional/setup-bugzilla.sh`
   exits 0.

4. Confirm `resolve_bz_port` end to end against a real container:

   ```sh
   tests/functional/setup-bugzilla.sh build   # first run only; skip if image exists
   tests/functional/setup-bugzilla.sh start
   tests/functional/setup-bugzilla.sh status
   ```

   Expected: `start` logs `Container listening on host port <N>.` with a
   real numeric port, `status` reports `REST API: reachable`.

5. Confirm the override mismatch path:

   ```sh
   BZR_FUNC_PORT=19999 tests/functional/setup-bugzilla.sh status
   ```

   Expected: prints `REST API: unknown — BZR_FUNC_PORT does not match the
   running container's actual port` (container from step 4 is still running
   on its own dynamic port, not 19999).

6. `tests/functional/setup-bugzilla.sh stop` to clean up.

7. Do **not** commit yet. At this point `setup-bugzilla.sh` publishes a
   runtime-assigned port but `run-tests.sh` (Task 4) still assumes the old
   fixed `DEFAULT_PORT` table — a commit landed here alone would leave
   `make functional-test` genuinely red for anyone who checks out or
   bisects into it, working against `AGENTS.md`'s per-commit bisect-hygiene
   policy. Task 4 step 7 commits both files together as one change.

**Acceptance criteria for this task:** Steps 3–6 all produce their
documented output; `DEFAULT_PORT` no longer appears anywhere in the file;
`resolve_bz_port` is the sole place `BZ_PORT` is assigned after variable
initialization.

---

## Task 4 — Update `tests/functional/run-tests.sh`

**Modifies:** `tests/functional/run-tests.sh`.

**Interfaces consumed:** Task 1's `container_runtime`,
`bugzilla_container_name`, `bugzilla_container_port` (via the existing
`source "$SCRIPT_DIR/lib.sh"` at current line 19, which now transitively
sources `container-env.sh`).

### Steps

1. Write the failing check:

   ```sh
   grep -n 'DEFAULT_PORT=8089 ;;' tests/functional/run-tests.sh
   ```

   Expected: one match (current line 24).

2. Replace current lines 22–30:

   ```bash
   BZ_VERSION="${BZR_BZ_VERSION:-bz50}"
   case "$BZ_VERSION" in
   bz50) DEFAULT_PORT=8089 ;;
   bz52) DEFAULT_PORT=8090 ;;
   bz53) DEFAULT_PORT=8091 ;;
   *) DEFAULT_PORT=8089 ;;
   esac
   BZ_PORT="${BZR_FUNC_PORT:-$DEFAULT_PORT}"
   BZ_URL="http://127.0.0.1:${BZ_PORT}"
   ```

   with:

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

   (`lib.sh`, sourced above this block at line 19, already sets `BZ_VERSION`
   via `container-env.sh`, so this task's replacement does not need its own
   `BZ_VERSION` assignment.)

3. `bash -n tests/functional/run-tests.sh` exits 0.

4. Confirm end to end (requires a container already running from Task 3's
   verification, or start one fresh):

   ```sh
   tests/functional/setup-bugzilla.sh start
   tests/functional/run-tests.sh
   ```

   Expected: the full phase suite runs against the dynamically-assigned
   port and passes (same pass/fail summary as before this change — no
   behavioral change to any `bzr` command, only how the target URL is
   found).

5. Confirm the override path still works:

   ```sh
   tests/functional/setup-bugzilla.sh stop
   BZR_FUNC_PORT=18089 tests/functional/setup-bugzilla.sh start
   BZR_FUNC_PORT=18089 tests/functional/run-tests.sh
   curl -sf http://127.0.0.1:18089/rest/version >/dev/null && echo "reachable on 18089"
   ```

   Expected: suite passes; the container is reachable on the pinned port.

6. `tests/functional/setup-bugzilla.sh stop` to clean up.

7. `git add tests/functional/setup-bugzilla.sh tests/functional/run-tests.sh
   && git commit` with message `feat(functional): resolve Bugzilla container
   port and name dynamically`, covering both this task's and Task 3's
   changes in one commit — the two scripts implement one coupled behavior
   change (a dynamic port/name on the start side is unusable until the
   discovery side stops assuming the old fixed-port table), and splitting
   them across commits would leave `make functional-test` red at the
   intermediate commit.

**Acceptance criteria for this task:** Steps 3–5 all produce their
documented output; `DEFAULT_PORT` no longer appears anywhere in either
`setup-bugzilla.sh` or `run-tests.sh`; the single commit covering both
files is the first point in the branch's history where `make
functional-test` is green under the new dynamic scheme.

---

## Task 5 — Update `tools/record-demo.sh`

**Modifies:** `tools/record-demo.sh`.

**Interfaces consumed:** Task 1's `container_runtime`,
`bugzilla_container_name`, `bugzilla_container_port` (via a new, direct
`source "$SCRIPT_DIR/container-env.sh"` — **not** `lib.sh`, to avoid
`lib.sh`'s `mktemp`/`trap` side effects colliding with this script's own
`--drive*` `EXIT` traps).

### Steps

1. Write the failing check:

   ```sh
   grep -n 'BZ_URL=\${BZ_URL:-http://127.0.0.1:8089}' tools/record-demo.sh
   ```

   Expected: one match (current line 22).

2. Remove the current line 22 (`BZ_URL=${BZ_URL:-http://127.0.0.1:8089}`)
   entirely from its current position.

3. Immediately after the closing `fi` of the `--drive-project-manager-reporting`
   block (current lines 189–211, ending `exit 0` / `fi` at lines 211–212,
   directly before the blank line and the `if [[ "${1:-}" == "project-manager-reporting" ]]; then`
   block that currently starts at line 214), insert:

   ```bash
   if [[ -z "${BZ_URL:-}" ]]; then
       SCRIPT_DIR="$REPO_ROOT/tests/functional"
       # shellcheck source=tests/functional/container-env.sh
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

   This placement is after every `--drive*` dispatch check (none of which
   read `$BZ_URL`) and before the four "main" orchestrator blocks
   (`project-manager-reporting`, `release-readiness`, `dependency-analysis`,
   and the default/`--weekly-status` path) that do.

4. `bash -n tools/record-demo.sh` exits 0.

5. Confirm the driver paths are unaffected (they must not touch
   `container-env.sh` at all):

   ```sh
   grep -n 'container-env.sh' tools/record-demo.sh
   ```

   Expected: exactly one match, inside the block added in step 3 (line
   number will have shifted from the file edits above; confirm by reading
   the surrounding context, not by an exact line number).

6. Confirm end to end (requires a running container — reuse one from Task 3
   or Task 4's verification, or start fresh with
   `tests/functional/setup-bugzilla.sh start`):

   ```sh
   cargo build --release
   BZ_URL= tools/record-demo.sh --weekly-status
   ```

   (Interrupting after the first few seconds once recording visibly starts
   is fine — the goal is confirming `BZ_URL` resolves and the connectivity
   check at the top of the orchestrator block passes, not producing a full
   GIF.) Expected: no `ERROR: no Bugzilla at http://127.0.0.1:` failure;
   the script proceeds past its connectivity check.

7. `git add tools/record-demo.sh && git commit` with message
   `fix(tools): derive record-demo.sh's default BZ_URL dynamically`.

**Acceptance criteria for this task:** Steps 4–6 all produce their
documented output; the literal string `8089` no longer appears anywhere in
`tools/record-demo.sh`.

---

## Task 6 — Update `tests/functional/README.md`

**Modifies:** `tests/functional/README.md`.

### Steps

1. In the Environment Variables table (current lines 54–62):
   - `BZR_FUNC_PORT` row: change `Default` from `8089` to
     `(runtime-assigned)`; change the description to
     `Host port mapped to container port 80 (overrides the runtime-assigned default when set)`.
   - `BZR_FUNC_CONTAINER` row: change `Default` from `bzr-func-test` to
     `bzr-func-test-<version>-<checkout-id>`.
   - `BZR_FUNC_TLS_PORT` row: change `Default` from `BZR_FUNC_PORT + 1000`
     to `(resolved backend port) + 1000`.
   - `BZR_FUNC_REDHAT_PORT` row: change `Default` from
     `BZR_FUNC_PORT + 2000` to `(resolved backend port) + 2000`.

2. In the Troubleshooting section, replace the **Port conflict** entry
   (current lines 125–126):

   ```markdown
   **Port conflict:**
   Change the port with `BZR_FUNC_PORT=9089`.
   ```

   with:

   ```markdown
   **Port conflict:**
   No longer occurs by default — each invocation gets a runtime-assigned
   host port. Set `BZR_FUNC_PORT` to pin an exact one if needed (e.g. to
   attach a debugger to a known address).
   ```

3. `git add tests/functional/README.md && git commit` with message
   `docs(functional): update README env-var defaults for dynamic ports`.

**Acceptance criteria for this task:** The literal string `8089` no longer
appears anywhere in `tests/functional/README.md`.

---

## Task 7 — Update `AGENTS.md`

**Modifies:** `AGENTS.md` (`CLAUDE.md` is a symlink to it — no separate
edit).

### Steps

1. Replace the current caution paragraph (lines 35–37):

   ```markdown
   Functional tests start Bugzilla containers with fixed ports. Be cautious
   when running multiple workflows simultaneously as they may interfere with
   each other during this test phase.
   ```

   with:

   ```markdown
   Functional tests start Bugzilla containers on a runtime-assigned host
   port under a name scoped to the checkout's own filesystem path, so
   concurrent worktrees or clones running functional tests at the same time
   no longer need manual port/name coordination. Two invocations from the
   exact same checkout directory at the same time still collide (same
   checkout id, same default container name).
   ```

2. `git add AGENTS.md && git commit` with message
   `docs: update functional-test concurrency caveat for dynamic ports`.

**Acceptance criteria for this task:** `grep -c "fixed ports" AGENTS.md`
returns `0`.

---

## Task 8 — Update Containerfile header comments

**Modifies:** `tests/functional/versions/bz50/Containerfile`,
`tests/functional/versions/bz52/Containerfile`,
`tests/functional/versions/bz53/Containerfile`.

### Steps

1. In each file's line 3, replace:

   ```
   # Run:    podman run -d --name bzr-func-test-bz50 -p 8089:80 localhost/bzr-func-bz50:latest  (or docker)
   ```

   (substituting `bz52`/`8090` and `bz53`/`8091` in the other two files)
   with:

   ```
   # Run:    tests/functional/setup-bugzilla.sh start   (BZR_BZ_VERSION=bz50 if run outside the make target)
   ```

   (substituting `bz52`/`bz53` in the other two files' `BZR_BZ_VERSION=`
   mention).

2. Confirm no fixed ports remain in any header comment:

   ```sh
   grep -n '^# Run:' tests/functional/versions/*/Containerfile
   ```

   Expected: three matches, none containing `8089`, `8090`, or `8091`.

3. `git add tests/functional/versions/*/Containerfile && git commit` with
   message `docs(functional): point Containerfile Run comments at setup-bugzilla.sh`.

**Acceptance criteria for this task:** Step 2's grep matches exactly three
lines, none containing a literal fixed port number. `EXPOSE 80` and every
other line in all three files is byte-for-byte unchanged (diff the rest of
each file against its pre-task state to confirm).

---

## Task 9 — Full functional verification (pre-PR gate)

Not a code change — this task is the mandatory full functional run
`AGENTS.md`'s "any other change" rule requires before the PR opens, plus the
two acceptance criteria (3 and 6) that need a live check rather than a task-
level unit verification.

### Steps

1. `make functional-test` (default `bz50`, no overrides) — expected: green,
   full phase summary passes (acceptance criterion 1).
2. `tests/functional/setup-bugzilla.sh stop` (clears the container criterion
   1 left running).
3. `BZR_FUNC_PORT=18089 make functional-test` then
   `curl -sf http://127.0.0.1:18089/rest/version` — expected: green, curl
   succeeds while the container is up (acceptance criterion 2).
4. `make functional-stop`.
5. `make functional-test-all` — expected: all three versions pass
   sequentially (acceptance criterion 4).
6. `make functional-stop-all`.
7. `tests/functional/setup-bugzilla.sh status` against a stopped/removed
   container — expected: `Container <name> does not exist.`, exit 1
   (acceptance criterion 5, unchanged path).
8. Two-checkout manual exercise (acceptance criterion 3): from a second
   checkout of this repository at the same commit (e.g.
   `git worktree add ../bzr-verify-606 feat/dynamic-container-ports-606`),
   run `tests/functional/setup-bugzilla.sh start` for `bz50` in **both**
   checkouts without stopping either between runs. Expected: neither
   invocation fails with a name or port collision, and
   `tests/functional/setup-bugzilla.sh status` in each checkout reports a
   distinct container name and port. Record the exact commands run and
   their output in the PR description. Clean up: `setup-bugzilla.sh stop`
   in both checkouts, then `git worktree remove ../bzr-verify-606`.
9. Task 1 steps 3–4 already exercised acceptance criterion 6 directly; no
   further action needed here beyond confirming they still pass against the
   final committed `container-env.sh`.
10. `make lint` (fmt, clippy, `check-shell`, and the rest of the guardrail
    suite) — expected: clean. This change touches only shell scripts and
    docs, so `cargo fmt`/`clippy` should be no-ops, but the gate runs
    regardless per repo convention.

**Acceptance criteria for this task:** All ten steps produce their
documented expected result. Step 8's commands and output are pasted into the
PR description verbatim, per the design's own commitment (Testing section,
criterion 3).
</content>
