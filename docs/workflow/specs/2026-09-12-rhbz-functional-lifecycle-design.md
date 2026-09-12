# RHBZ functional image and lifecycle (#675)

## Problem and authority

The stock functional images cannot exercise Red Hat Bugzilla extensions. Issue
#675's accepted RHBZ addendum requires a real RHBZ 5.0.4-rh111 server, built
from the pinned public revision `167ccca1b256f9462cb4aebcb1edbced7e4663d5` of
`https://pagure.io/Red-Hat-Bugzilla/rh-bugzilla.git`. The frozen authority is
the issue's WORK:SCOPE token `q675-58930cae`. [ADR 0073](../../adr/0073-isolate-rhbz-functional-lifecycle.md)
records the isolation decision.

## Scope and architecture

`rhbz` becomes a valid `BZR_BZ_VERSION` only for the existing lifecycle's
explicit selection. Its image lives beside stock version images, records the
immutable remote and revision, runs the fork's own setup against its own MariaDB
database, and starts Apache using the existing image conventions. It uses the
existing checkout-scoped version container name and runtime-assigned port, so
parallel worktrees remain isolated.

`tests/functional/setup-bugzilla.sh` supplies the version-specific build context,
image name, start, readiness, and stop behavior unchanged in shape. `rhbz` gets
a suitable explicit readiness timeout but does not change the default version or
the bz50/bz52/bz53 arrays in `run-all-versions.sh` and `run-compare-all.sh`.

`tests/functional/run-rhbz-compare.sh` is a narrow runner rather than a branch
inside `run-compare.sh`: stock comparison phases and the python-bugzilla sidecar
are not RHBZ prerequisites. It resolves the running version-selected container
port, sets the existing test helper context, sources the smoke phase, reports the
test summary, and cleans its private temporary state. The smoke calls the live
extension endpoint and passes only when the response establishes reachability and
advertises `ExternalBugs`, `SubComponents`, and `RedHat`. It runs under the stable
test ID `compare/07-rhbz-smoke/extensions`.

`make functional-compare-rhbz` explicitly resets the RHBZ lifecycle, runs this
runner, and stops the RHBZ container on success or failure. Future catalogue work
may append RHBZ-only phases after this smoke phase; it must not add them to stock
runners.

## Success and failure behavior

- A clean build selects exactly the pinned public revision; a missing or changed
  source revision fails the image build rather than selecting another revision.
- `BZR_BZ_VERSION=rhbz` can build, start, become REST-ready, and stop through
  the normal lifecycle without affecting stock version behavior.
- The RHBZ smoke arm fails on an unreachable endpoint, invalid JSON, or any
  missing required extension, and uses the named stable ID on success or failure.
- The Make target returns the smoke result while still attempting lifecycle
  cleanup. Stock default and all-version arrays remain byte-for-byte limited to
  bz50, bz52, and bz53.

## Boundaries and non-goals

The build boundary consumes a public, pinned source revision; container build
failure is reported by the runtime and no credentials are introduced. The smoke
consumes server-controlled JSON and uses `jq` only to test fixed extension names;
it does not construct shell code, persist data, or expose secrets. Existing
container-runtime selection, local port publication, and cleanup remain their
owners. This change does not implement or classify RHBZ extensions, change bzr
commands/config/schema/transport, use proxy behavior as RHBZ proof, or change
stock all-version runners. #774 and #775 own the extension catalogue phases.

## Validation

Focused shell fixtures will prove that `rhbz` is accepted while unknown versions
remain rejected, that the runner sources only the smoke phase after readiness, and
that missing extension names fail the smoke assertion. `make check-shell` covers
all added shell files. `make lint` and `make test` are required. The real proof is
`make functional-compare-rhbz`, which must build, start, smoke-test, and clean up
the RHBZ container. Rust 1.89.0, the current-thread runtime, and all declared
release targets are unchanged.
