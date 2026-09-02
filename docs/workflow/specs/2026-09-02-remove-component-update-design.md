# Remove unsupported component update

## Scope

Issue #624 removes `bzr component update` because stock Bugzilla 5.x has no
component-update REST endpoint. The operator explicitly chose removal over a
fork-only compatibility surface. Component list, view, and create remain unchanged.

ADR [0037](../../adr/0037-remove-unsupported-component-update.md) records the
product decision and supersedes ADR 0003.

## Current behavior

The CLI advertises an `update` subcommand with ID and product/name targets,
JSON-file input, and dry-run support. Its client sends `PUT /component/{id}`.
Stock Bugzilla 5.0 and 5.2 expose only component creation, so the live functional
suite observes API error 32614 and turns the failure into a skip. The checked-in
`component-update-input` schema and reference guide publish the unusable surface.

## Required behavior

- `bzr component update ...` is rejected by clap as an unknown component
  subcommand with exit code 2, before configuration or network access.
- Component help, generated completion, and the canonical embedded `bzr-reference`
  payload expose only `list`, `view`, and `create`.
- The `component-update-input` schema is removed from the registry and filesystem;
  requesting that name follows the existing unknown-schema error path.
- The update command handler, client method, request type, dry-run capability,
  integration fixtures, and stock-server skip probes are deleted.
- Component create/list/view behavior and their schema contracts do not change.
- Historical specifications remain historical. ADR 0003 receives only the required
  supersession banner; current comments and reference documentation stop claiming
  component update exists.

## Components and data flow

Removal starts at `ComponentAction`: without an `Update` variant, dispatch and
capability matching become three-way and cannot reach update code. The now-unreachable
command module, client method, request type, schema file, and their tests are removed.
Documentation, the embedded agent reference, and functional phases then assert the
smaller public surface rather than describing or probing the deleted path.

There is no runtime fallback or migration flow. Existing invocations fail during clap
parsing and never read credentials or contact a server.

## Error handling

No new error type is introduced. The existing clap invalid-subcommand diagnostic and
exit code 2 are the complete failure contract. `bzr schema component-update-input`
continues through the existing unknown-schema input error and exits 7.

## Verification

- A parser test first expects `component update` to be an invalid subcommand; it fails
  before the variant is removed and passes afterward.
- Schema unit and functional tests prove the removed schema is absent and rejected.
- The functional component phase invokes the removed command and requires exit 2,
  proving the user-facing change against the real binary without a server capability
  skip.
- Focused tests, `make lint`, `make test`, and `make functional-test-all` must pass.

## Compatibility and rollback

This intentionally breaks callers using a non-stock fork that implemented the invented
endpoint. Reverting the change restores the command and schema; no persisted data or
server state is migrated.
