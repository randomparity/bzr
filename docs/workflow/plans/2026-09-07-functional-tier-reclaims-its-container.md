# Functional tier reclaims its container — implementation plan

**Goal.** Stop `make functional-test` leaking one Bugzilla container per run, and make a stop-based
reclaim verifiable instead of silently reported as successful.

**Architecture.** `tests/functional/setup-bugzilla.sh` owns the container lifecycle; every
single-version run goes through `tests/functional/run-tests.sh`, whose existing EXIT trap gains one
guarded call into that owner. No new script, no new lifecycle state. Stack: Bash 3.2+ (macOS
default), GNU make, podman or docker.

**Global Constraints.** 4-space indent in `tests/functional/*.sh`, tabs in Makefile recipes. Both
scripts run under `set -euo pipefail`, so every command added inside an EXIT trap is guarded and
cannot change the script's exit status. `make check-shell` stays green. Do not touch
`tests/functional/phases/*`, the body of `run-all-versions.sh`, `src/**`, or `docs/adr/README.md`.
Decision record: `docs/adr/0067-functional-tier-reclaims-its-container.md`.

Expected implementation size: 45–80 changed lines (S) — from the file map: two shell edits of about
fourteen lines each, a four-line Makefile recipe, two Makefile list lines, two Makefile comments,
four documentation lines.

**File map.** Modified: `tests/functional/setup-bugzilla.sh` (`cmd_stop` verifies, `cmd_reset` reads
its status), `tests/functional/run-tests.sh` (`cleanup` reclaims), `Makefile` (`check-shell` lists,
`functional-stop-all` recipe, two comments), `tests/functional/README.md`, `CONTRIBUTING.md`.
Created: none.

## Task 1 — `stop` verifies its own removal, and its file joins the shell gate

**Interfaces.** Consumes `container_exists()` and `err()`, defined in `setup-bugzilla.sh:96-99` and
`:54-57`. Provides: `cmd_stop` exits 0 only when the container is gone, 1 otherwise. Task 2 depends
on that status.

**Verification.**

- Contract: `stop`'s exit status reflects whether the container is gone. Mode: `focused-test`, run
  against a stub runtime so it works on podman, docker, and CI alike — the real dependency refusal
  is podman-only (docker 29.7.2 removes a base container with a live dependent, exit 0). The stub
  exits 0 for every call, so `rm -f` "succeeds" while `container inspect` reports the container
  still present:

  ```bash
  d=$(mktemp -d); printf '%s\n' '#!/bin/sh' 'exit 0' >"$d/podman"; chmod +x "$d/podman"
  PATH="$d:$PATH" tests/functional/setup-bugzilla.sh stop; echo "exit=$?"
  ```

  Red on the base commit: `Container removed.` and `exit=0`. Green after: the survived-removal
  diagnostic and `exit=1`. Then rewrite the stub body — replacing its `exit 0` line, not appending
  after it — as `[ "$1" = container ] && exit 1; exit 0`, so it reports the container gone, and
  re-run for `Container removed.` and `exit=0` on both commits. Remove `$d` afterwards.
- Contract: `check-shell` covers `setup-bugzilla.sh`. Mode:
  `focused-test`. Red on the base commit: appending `echo $UNDEFINED_UNQUOTED` to
  `setup-bugzilla.sh` leaves `make check-shell` exiting 0. Green after: the same edit makes it exit
  non-zero naming `setup-bugzilla.sh` (SC2086 on shellcheck 0.11.0 — assert the status and the
  filename, not the code). Revert the injected line. The file is SC1091-clean only inside the gate's
  combined invocation, which passes `container-env.sh` as an input too.

**Steps.**

1. Replace `cmd_stop` (`setup-bugzilla.sh:151-156`), taking over the check `cmd_reset` performs
   after calling it:

   ```bash
   cmd_stop() {
       log "Stopping and removing container ${CONTAINER_NAME}..."
       $CONTAINER_RT rm -f "$CONTAINER_NAME" 2>/dev/null || true
       # `rm -f` failure is discarded because "no such container" is routine, so the
       # container itself is the check, and it makes this exit status honest for any
       # refused removal. podman refuses to remove one a running container depends
       # on through `--network container:<name>` — a leftover python-bugzilla
       # sidecar; docker removes it anyway (ADR 0067, amending ADR 0058).
       if container_exists; then
           err "Container ${CONTAINER_NAME} survived removal. A dependent container" \
               "is probably holding it -- most likely a leftover python-bugzilla" \
               "sidecar from a comparison run whose cleanup did not fire. Remove it" \
               "(\`${CONTAINER_RT} rm -f <sidecar>\`) and retry."
           return 1
       fi
       log "Container removed."
       return 0
   }
   ```

2. Replace `cmd_reset` (`:186-201`), deleting the comment block and `container_exists` guard it
   holds today:

   ```bash
   cmd_reset() {
       cmd_stop || return 1
       cmd_start
       return 0
   }
   ```

3. Replace `functional-stop-all`'s three recipe lines (`Makefile:235-237`) so one refused version no
   longer abandons the rest, while the target still fails if any did not come down:

   ```make
   	@status=0; for v in bz50 bz52 bz53; do \
   	  BZR_BZ_VERSION=$$v tests/functional/setup-bugzilla.sh stop || status=1; \
   	done; exit $$status
   ```

4. Add `tests/functional/setup-bugzilla.sh` after `tests/functional/run-compare-all.sh` on both the
   `shellcheck -s bash` list (`Makefile:149`) and the `bash -n` list (`:150`). Leave
   `run-all-versions.sh` alone: it is equally unlinted, but this change does not edit it.

5. Run `make check-shell`; expect exit 0 and no shellcheck output. Perform both red/green
   observations and keep every output for the pull-request body.

6. Commit: `fix(functional): make setup-bugzilla stop report a refused removal`.

**Acceptance.** `Container removed.` prints only when the container is gone; `cmd_reset` contains no
`container_exists` call; `make functional-stop-all` attempts all three versions; `make check-shell`
is green and names `setup-bugzilla.sh` on both lists.

## Task 2 — the runner reclaims its container, with a documented opt-out

**Interfaces.** Consumes `cmd_stop`'s exit status from Task 1, plus `SCRIPT_DIR` (`run-tests.sh:16`)
and `BZ_VERSION` (from `container-env.sh`, sourced through `lib.sh` at `:20`), both set before the
trap is installed. Defines `FUNC_XDG_ORIG`, the caller's `XDG_CONFIG_HOME` captured beside the
test-isolation redirect at `:62-63`: the trap deletes the directory that redirect points at and the
container runtime reads `$XDG_CONFIG_HOME/containers/containers.conf`, so without it the reclaim
resolves a different runtime configuration than the start did. Provides `BZR_FUNC_KEEP`, read only
here.

**Verification.**

- Contract: the tier removes its container on exit unless `BZR_FUNC_KEEP` is non-empty. Mode:
  `focused-test`. Observable: `podman ps -a --filter name=bzr-func-test- --format '{{.Names}}'` (or
  `docker`) after the run. Red on the base commit: `make functional-test` leaves
  `bzr-func-test-bz50-<id>` listed. Green after: unlisted, while
  `BZR_FUNC_KEEP=1 make functional-test` leaves exactly that one name.
- Contract: the trap moves the run's exit status in neither direction. Mode: `focused-test`, on the
  exact construct `cleanup` uses:

  ```bash
  bash -c 'set -euo pipefail
           cleanup() { false || echo warn >&2; return 0; }; trap cleanup EXIT; exit 3'; echo "exit=$?"
  ```

  Green: `warn` and `exit=3` — a failing reclaim neither masks a failing run nor reddens a passing
  one. Red with the guard dropped (bare `false` under `set -e`): no `warn`, `exit=1`. Confirm on the
  real path: `make functional-test; echo $?` prints `0` on a passing tier.
- Contract: `BZR_FUNC_KEEP` is documented at the five sites in steps 3-5. Mode:
  `task-test-not-applicable`. Reason: prose with no executable consumer — the repo's only
  documentation drift gate, `agent-skills/tests/flag-drift-check.sh`, parses `docs/bzr-cli.md`'s
  command tree and CLI globals, and no gate parses `tests/functional/README.md` or `CONTRIBUTING.md`.

**Steps.**

1. Capture the caller's value immediately before the redirect at `run-tests.sh:62-63`:
   `FUNC_XDG_ORIG="${XDG_CONFIG_HOME:-}"`.

2. Replace `cleanup` (`run-tests.sh:69-73`):

   ```bash
   cleanup() {
       rm -rf "$FUNC_CONFIG_DIR"
       _cleanup_tmpfiles
       # Reclaim the container this run used (ADR 0067). Guarded: the script runs
       # under `set -e` and an EXIT trap must not change the run's exit status.
       # XDG_CONFIG_HOME is restored because the line above deleted the directory
       # the isolation redirect points at, and the runtime reads its config there.
       if [[ -z "${BZR_FUNC_KEEP:-}" ]]; then
           XDG_CONFIG_HOME="$FUNC_XDG_ORIG" BZR_BZ_VERSION="$BZ_VERSION" \
               "$SCRIPT_DIR/setup-bugzilla.sh" stop ||
               echo "WARNING: could not reclaim the Bugzilla container (see above);" \
                   "set BZR_FUNC_KEEP=1 to keep it deliberately." >&2
       fi
       return 0
   }
   ```

3. In `Makefile`, add a two-line comment above the `functional-test` target (`:197`) saying the
   runner reclaims its container on exit (ADR 0067) and that `BZR_FUNC_KEEP=1` keeps it warm across
   runs, with `functional-start` then reusing it as before. Then correct the `functional-compare`
   comment at `:200-203`, whose premise this change falsifies: `functional-start` now reuses a
   container an earlier `functional-test` left only when that run set `BZR_FUNC_KEEP`, or when the
   container was started by hand. Keep the `reset` sentence and its ADR 0058 reference.

4. In `tests/functional/README.md`, add a row after `BZR_FUNC_TIMEOUT` in the environment table:

   ```markdown
   | `BZR_FUNC_KEEP` | `(unset)` | Any non-empty value keeps the container after `run-tests.sh` exits instead of reclaiming it |
   ```

   Then extend the "Orphaned containers left behind by a deleted worktree or clone" paragraph with
   one sentence: the runner now reclaims its own container, so the procedure applies to containers
   from runs before that change or from runs with `BZR_FUNC_KEEP` set.

5. In `CONTRIBUTING.md`, extend the "**A stale container.**" bullet (`:99-101`) with one sentence:
   residue now survives only under `BZR_FUNC_KEEP` or a hand-started container, and the `reset` in
   the command block below still guarantees a clean one.

6. Run `make check-shell`, `make lint`, then `make test`; expect exit 0 from each.

7. Perform both red/green observations, then run `make functional-test`: expect exit 0, zero
   failures in the summary, and no `bzr-func-test-` container afterwards.

8. Commit: `fix(functional): reclaim the tier's container when the run ends`.

**Acceptance.** `make functional-test` leaves no container; `BZR_FUNC_KEEP=1 make functional-test`
leaves exactly one; a refused removal prints the warning without changing the run's exit status;
`BZR_FUNC_KEEP` appears at all five documentation sites.
