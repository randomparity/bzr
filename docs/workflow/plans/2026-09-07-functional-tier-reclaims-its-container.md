# Functional tier reclaims its container — implementation plan

**Goal.** Stop `make functional-test` leaking one Bugzilla container per run, and make a stop-based
reclaim verifiable instead of silently reported as successful.

**Architecture.** `tests/functional/setup-bugzilla.sh` owns the container lifecycle; every
single-version run goes through `tests/functional/run-tests.sh`, whose existing EXIT trap gains one
guarded call into that owner. No new script, no new lifecycle state. Stack: Bash 3.2+ (macOS
default), GNU make, podman or docker.

**Global Constraints.** 4-space indent in `tests/functional/*.sh`. `run-tests.sh` and
`setup-bugzilla.sh` run under `set -euo pipefail`; every command added inside an EXIT trap is
guarded so it cannot change the script's exit status. `make check-shell` runs `shellcheck -s bash`
and `bash -n`; both stay green. Do not touch `tests/functional/phases/*`, the body of
`tests/functional/run-all-versions.sh`, `src/**`, or `docs/adr/README.md`. Decision record:
`docs/adr/0067-functional-tier-reclaims-its-container.md`.

Expected implementation size: 40–70 changed lines (S) — from the file map: two shell edits of about
twelve lines each, two Makefile list lines, one Makefile comment, four documentation lines.

**File map.** Modified: `tests/functional/setup-bugzilla.sh` (`cmd_stop` verifies, `cmd_reset` reads
its status), `tests/functional/run-tests.sh` (`cleanup` reclaims), `Makefile` (`check-shell` lists,
`functional-test` comment), `tests/functional/README.md`, `CONTRIBUTING.md`. Created: none.

## Task 1 — `stop` verifies its own removal, and its file joins the shell gate

**Interfaces.** Consumes `container_exists()` and `err()`, defined in
`tests/functional/setup-bugzilla.sh:96-99` and `:54-57`. Provides: `cmd_stop` exits 0 only when the
container is gone, 1 otherwise. Task 2 depends on that status.

**Verification.**

- Contract: `stop`'s exit status reflects whether the container is gone. Mode: `focused-test`.
  Red on the base commit: with a sidecar attached
  (`podman run -d --name bzr-func-dep-check --network container:<name> <image> sleep 120`),
  `tests/functional/setup-bugzilla.sh stop; echo $?` prints `Container removed.` and `0` while
  `podman ps` still lists the container. Green after: the same command prints the survived-removal
  diagnostic and `1`; with the sidecar removed, `Container removed.` and `0`.
- Contract: `check-shell` covers `setup-bugzilla.sh` and `run-all-versions.sh`. Mode:
  `focused-test`. Red on the base commit: appending `echo $UNDEFINED_UNQUOTED` to
  `setup-bugzilla.sh` leaves `make check-shell` green. Green after: the same edit fails it with
  SC2154. Revert the injected line after observing.

**Steps.**

1. Replace `cmd_stop` (`tests/functional/setup-bugzilla.sh:151-156`), taking over the check
   `cmd_reset` performs after calling it:

   ```bash
   cmd_stop() {
       log "Stopping and removing container ${CONTAINER_NAME}..."
       $CONTAINER_RT rm -f "$CONTAINER_NAME" 2>/dev/null || true
       # `rm -f` failure is discarded because "no such container" is routine, so the
       # container itself is the check: podman refuses to remove one a running
       # container depends on through `--network container:<name>`, which is what a
       # leftover python-bugzilla sidecar is (ADR 0067, amending ADR 0058).
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

3. In `Makefile`, add `tests/functional/run-all-versions.sh tests/functional/setup-bugzilla.sh`
   after `tests/functional/run-compare-all.sh` on both the `shellcheck -s bash` list (line 149) and
   the `bash -n` list (line 150).

4. Run `make check-shell`; expect exit 0 and no shellcheck output. Then perform the two red/green
   observations above and keep both outputs for the pull-request body.

5. Commit: `fix(functional): make setup-bugzilla stop report a refused removal`.

**Acceptance.** `Container removed.` prints only when the container is gone; `cmd_reset` contains no
`container_exists` call; `make check-shell` is green and names both new files.

## Task 2 — the runner reclaims its container, with a documented opt-out

**Interfaces.** Consumes `cmd_stop`'s exit status from Task 1, plus `SCRIPT_DIR`
(`run-tests.sh:16`) and `BZ_VERSION` (set by `container-env.sh`, sourced through `lib.sh` at
`run-tests.sh:20`), both established before the trap is installed. Provides `BZR_FUNC_KEEP`, read
only here.

**Verification.**

- Contract: the tier removes its container on exit unless `BZR_FUNC_KEEP` is non-empty. Mode:
  `focused-test`. Observable: `podman ps -a --filter name=bzr-func-test- --format '{{.Names}}'`
  after the run. Red on the base commit: `make functional-test` leaves `bzr-func-test-bz50-<id>`
  listed. Green after this task: the same run leaves it unlisted, and
  `BZR_FUNC_KEEP=1 make functional-test` leaves exactly that one name.
- Contract: the trap does not change the run's exit status. Mode: `focused-test`. Observable:
  `make functional-test; echo $?` prints `0` on a passing tier even when the stop emits its
  diagnostic; exercise that by leaving a sidecar attached (Task 1's construction) before the run.
- Contract: `BZR_FUNC_KEEP` is documented in the `tests/functional/README.md` env table and orphan
  note, the `CONTRIBUTING.md` stale-container bullet, and the `functional-test` Makefile comment.
  Mode: `task-test-not-applicable`. Reason: prose with no executable consumer — the repo's only
  documentation drift gate, `agent-skills/tests/flag-drift-check.sh`, parses `docs/bzr-cli.md`'s
  command tree and CLI globals, and no gate parses `tests/functional/README.md` or
  `CONTRIBUTING.md`.

**Steps.**

1. Replace `cleanup` (`tests/functional/run-tests.sh:69-73`):

   ```bash
   cleanup() {
       rm -rf "$FUNC_CONFIG_DIR"
       _cleanup_tmpfiles
       # Reclaim the container this run used (ADR 0067). Guarded: the script runs
       # under `set -e` and an EXIT trap must not change the run's exit status.
       if [[ -z "${BZR_FUNC_KEEP:-}" ]]; then
           BZR_BZ_VERSION="$BZ_VERSION" "$SCRIPT_DIR/setup-bugzilla.sh" stop ||
               echo "WARNING: could not reclaim the Bugzilla container (see above);" \
                   "set BZR_FUNC_KEEP=1 to keep it deliberately." >&2
       fi
       return 0
   }
   ```

2. In `Makefile`, put this comment directly above the `functional-test` target (line 197):

   ```make
   # The runner reclaims its container on exit (ADR 0067). Export BZR_FUNC_KEEP=1 to
   # keep it warm across runs; `functional-start` then reuses it as before.
   ```

3. In `tests/functional/README.md`, add a row after `BZR_FUNC_TIMEOUT` in the environment table:

   ```markdown
   | `BZR_FUNC_KEEP` | `(unset)` | Any non-empty value keeps the container after `run-tests.sh` exits instead of reclaiming it |
   ```

   Then extend the "Orphaned containers left behind by a deleted worktree or clone" paragraph with
   one sentence saying the runner now reclaims its own container, so the procedure applies to
   containers from runs before that change or from runs with `BZR_FUNC_KEEP` set.

4. In `CONTRIBUTING.md`, extend the "**A stale container.**" bullet (`:99-101`) with one sentence
   saying residue now survives only under `BZR_FUNC_KEEP` or a hand-started container, and that the
   `reset` in the command block below still guarantees a clean one.

5. Run `make check-shell`, `make lint`, then `make test`; expect exit 0 from each.

6. Perform both red/green observations above, then run `make functional-test`: expect exit 0, zero
   failures in the summary, and no `bzr-func-test-` container afterwards.

7. Commit: `fix(functional): reclaim the tier's container when the run ends`.

**Acceptance.** `make functional-test` leaves no container; `BZR_FUNC_KEEP=1 make functional-test`
leaves exactly one; a refused removal prints the warning without changing the run's exit status;
`BZR_FUNC_KEEP` appears at all four documentation sites.
