# RHBZ functional image and lifecycle — implementation plan

**Goal.** Provide an explicit, real-server RHBZ functional smoke arm for #675
without changing the stock version matrix.

**Architecture.** The existing lifecycle remains the owner of image building,
container naming, port discovery, readiness, and cleanup. A new version directory
contains the pinned RHBZ image and entrypoint. A dedicated runner uses existing
functional helpers to run one extension-advertisement smoke phase. Stack: Bash,
GNU make, podman or docker, Fedora image tooling, MariaDB, Apache, and `jq`.

**Global Constraints.** Shell files use four-space indentation and `set -euo
pipefail`; Make recipes use tabs. Preserve the default `bz50` and the exact stock
arrays `bz50 bz52 bz53`. Do not add bzr runtime dependencies or behavior, proxy proof,
or catalogue rows. The source is exactly
`https://pagure.io/Red-Hat-Bugzilla/rh-bugzilla.git` at
`167ccca1b256f9462cb4aebcb1edbced7e4663d5`. Run `make check-shell`, `make lint`,
`make test`, and `make functional-compare-rhbz`. Decision: ADR 0073.

Expected implementation size: 250–430 changed lines (L) — one image/entrypoint,
three lifecycle/runner/Make edits, one smoke phase, and focused shell fixtures.

**File map.** Create `tests/functional/versions/rhbz/Containerfile`,
`tests/functional/versions/rhbz/entrypoint.sh`,
`tests/functional/run-rhbz-compare.sh`, and
`tests/functional/compare/rhbz/07-rhbz-smoke.sh`. Modify
`tests/functional/setup-bugzilla.sh`, `Makefile`, and
`tests/functional/pybz/container-tests.sh`.

## Task 1 — add a pinned, disposable RHBZ image

**Interfaces.** Consumes lifecycle paths `versions/${BZ_VERSION}/Containerfile`
and `entrypoint.sh` from `setup-bugzilla.sh`. Provides an HTTP service on port 80
whose `/rest/version` endpoint is ready and whose extension endpoint is available.

**Verification.**

- Contract: the image source cannot float. Mode: focused-test; add a fixture that
  reads the Containerfile and fails red on the base because it is absent, then green
  only when the literal remote and full revision occur in the checkout command.
- Contract: setup provisions an isolated database and starts the fork. Mode:
  focused-test; fixture reads the entrypoint and requires MariaDB startup,
  `checksetup.pl`, and foreground Apache. Red on the base because the file is absent;
  green after creation.

**Steps.**

1. Create the Containerfile from the Fedora and package conventions used by
   `versions/bz50/Containerfile`. Clone the pinned remote, run
   `git checkout --detach 167ccca1b256f9462cb4aebcb1edbced7e4663d5`, then verify
   `git rev-parse HEAD` equals that full revision; a checkout failure stops the build.
   Copy the fork into `/var/www/html/bugzilla` only when its checkout layout requires it.
   Install only packages that the fork's `checksetup.pl`
   reports as required during the real build; do not retain a best-effort dependency
   install that hides a missing prerequisite.
2. Create the entrypoint based on `versions/bz50/entrypoint.sh`: start MariaDB, create
   the image-local `bugs` database/user, run the fork's setup with noninteractive admin
   answers, configure the functional API key only if the fork exposes the same table,
   then `exec httpd -D FOREGROUND`. Preserve failure exits from setup.
3. Run the focused fixture and `make check-shell`; expect exit 0. Run
   `BZR_BZ_VERSION=rhbz tests/functional/setup-bugzilla.sh build`; expect the printed
   source revision and a successful image build.

**Acceptance.** A clean image build cannot select an unpinned RHBZ revision and has no
dependency on a host database or stock image filesystem.

## Task 2 — admit `rhbz` only to the selected lifecycle

**Interfaces.** Consumes `BZ_VERSION` from `container-env.sh`; provides `rhbz` as a
valid case in `setup-bugzilla.sh` while leaving `run-all-versions.sh` and
`run-compare-all.sh` untouched.

**Verification.**

- Contract: `rhbz` receives a timeout and unknown versions still fail. Mode:
  focused-test; a stub runtime invokes lifecycle `status` with `BZR_BZ_VERSION=rhbz`
  and observes no “Unknown” error; `BZR_BZ_VERSION=nope` remains red with that error.
- Contract: stock arrays exclude RHBZ. Mode: focused-test; fixture requires exact
  `VERSIONS=("bz50" "bz52" "bz53")` in both stock all-version runners.

**Steps.**

1. Add an `rhbz)` timeout case in `setup-bugzilla.sh` with a documented build/start
   allowance derived from the first real run. Update its error text to include `rhbz`.
2. Do not edit either stock all-version runner. Add the focused fixture calls to
   `container-tests.sh`, including a stub no-container path that avoids requiring a
   local runtime.
3. Run the fixtures through their existing test entrypoint and `make check-shell`;
   expect exit 0. Start and stop the real lifecycle with `BZR_BZ_VERSION=rhbz` and
   confirm `/rest/version` readiness plus container removal.

**Acceptance.** Explicit RHBZ lifecycle commands work; ordinary stock commands retain
their exact selected versions.

## Task 3 — run and prove the isolated extension smoke phase

**Interfaces.** Consumes `container_runtime`, `bugzilla_container_name`,
`bugzilla_container_port`, `test_begin`, `test_pass`, `test_fail`, and `test_summary`
from `lib.sh`. Provides `run-rhbz-compare.sh` and the stable test ID
`compare/07-rhbz-smoke/extensions`.

**Verification.**

- Contract: smoke accepts all three advertised extensions and rejects each absence.
  Mode: focused-test; stub `curl` output with complete JSON is green, then remove one
  name at a time and observe a red `test_fail` reason naming the missing extension.
- Contract: runner order is readiness then only the smoke phase. Mode: focused-test;
  fixture captures sourced phase paths and rejects stock comparison phase names.

**Steps.**

1. Add `rhbz/07-rhbz-smoke.sh`, using a fixed endpoint and `jq -e` predicates for the three
   fixed extension names. Call `test_begin "extensions" ...` before network work and
   call exactly one terminal test helper.
2. Add `run-rhbz-compare.sh`: reject any selected version other than `rhbz`, resolve the
   runtime-selected container port with existing helpers, create private temp state,
   source only the smoke phase after lifecycle readiness has already completed, render
   the result, and clean temporary state without changing the phase’s exit status.
3. Extend `container-tests.sh` with the two fixtures. Run its existing command and
   `make check-shell`; expect exit 0. Exercise complete extension JSON, then each
   missing required key as a controlled-red fixture.

**Acceptance.** The smoke route supplies live, stable evidence of reachability and all
three extension names, and it does not run stock comparison phases or sidecar setup.

## Task 4 — expose a cleanup-safe Make target and complete proof

**Interfaces.** Consumes lifecycle `reset`/`stop` and `run-rhbz-compare.sh`; provides
`functional-compare-rhbz` as an explicit target.

**Verification.**

- Contract: target attempts stop when smoke fails. Mode: focused-test; extract the Make
  recipe into a controlled shell with a failing runner and recording stop stub; red on
  the base because no target exists, green after with a nonzero target result and one
  stop record.
- Contract: new scripts are gate-covered. Mode: focused-test; append a temporary invalid
  shell token to each new script and observe `make check-shell` fail; remove each token
  before the green run.

**Steps.**

1. Add `functional-compare-rhbz` beside comparison targets. Its complete recipe is one
   shell so Make cannot skip cleanup after a failing line:

   ```make
   functional-compare-rhbz:

	@status=0; \
	BZR_BZ_VERSION=rhbz tests/functional/setup-bugzilla.sh reset || status=1; \
	if [ $$status -eq 0 ]; then \
	  BZR_BZ_VERSION=rhbz tests/functional/run-rhbz-compare.sh || status=1; \
	fi; \
	BZR_BZ_VERSION=rhbz tests/functional/setup-bugzilla.sh stop || status=1; \
	exit $$status
   ```

   Add both new shell files to shellcheck and `bash -n` lists.
2. Add the target fixture and run the red/green shell-gate observation. Run `make lint`
   and `make test`; expect exit 0.
3. Run `make functional-compare-rhbz`; expect a passing extension smoke result and no
   remaining RHBZ container. Record the observed duration and any exact Fedora package
   additions required by the fork.

**Acceptance.** One explicit target builds/starts/smokes/stops RHBZ and does not alter
stock runner selection or leave a container behind.
