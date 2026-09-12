# RHBZ ExternalBugs catalogue (#774)

## Authority and decision

The frozen `WORK:SCOPE` annotation for #774, token `q774-62865921`, requires
real-RHBZ evidence for add, update, remove, and component-update operations;
it excludes bzr feature implementation. The user approved the dedicated
RHBZ-only phase on 2026-09-12. [ADR 0074](../../adr/0074-catalogue-rhbz-externalbugs-with-real-server-controls.md)
records that boundary.

## Architecture

`run-rhbz-compare.sh` will source `07-rhbz-smoke.sh` then a new
`08-rhbz-externalbugs.sh`, preserving the smoke test as the prerequisite. The
new phase owns a uniquely named product, component, external-tracker fixture,
and bug. It invokes python-bugzilla only through
`python-bugzilla-adapter.py`, which validates JSON requests before calling the
3.3.0 library's `add_external_tracker`, `update_external_tracker`,
`remove_external_tracker`, and `editcomponent` methods.

Each test first proves the python-bugzilla call succeeded over the selected
RHBZ transport, then reads server state through fixed REST endpoints or the
existing resource helpers. Add verifies the created link; update verifies its
changed status/description; remove verifies it is absent; component update
verifies the seeded component's persisted fields. The bzr command probe is
accepted only when it produces the known missing-command parser diagnostic;
that result becomes one `expect_gap 774` classification per test. Any
successful bzr command, changed diagnostic, missing positive control, invalid
response, or unexpected persisted state fails the phase.

The parity report replaces the obsolete stock-only component-update row with
four rows, each naming one stable ID:

- `compare/08-rhbz-externalbugs/add`
- `compare/08-rhbz-externalbugs/update`
- `compare/08-rhbz-externalbugs/remove`
- `compare/08-rhbz-externalbugs/component-update`

## Failure, isolation, and cleanup

The phase uses the runner's disposable RHBZ container and creates names scoped
by a run token, so repeated or parallel worktrees do not share records. It
uses the existing comparison exchange directory for private request captures.
Fixture creation failure prevents a false parity classification; no fallback to
a proxy or local recorder is permitted. The normal lifecycle owns container
cleanup; phase-created server records are disposable with that database.

## Security and non-goals

This adds no new bzr trust boundary, credentials, dependency, or command. The
adapter receives only test-generated JSON and validates its bounded scalar and
mapping shapes before forwarding them to python-bugzilla. API keys remain in
the existing private exchange files and must not be printed. The work does not
add bzr support, alter stock runners, use a shaped proxy as evidence, or cover
the custom-field/sub-component catalogue owned by #775.

## Validation

Focused python-bugzilla fixture tests prove the adapter accepts each supported
operation and rejects malformed requests, while shell fixtures prove the
runner order, all four stable IDs, controlled-gap classification, and report
rows. `make lint` and `make test` cover repository guardrails. The decisive
proof is `make functional-compare-rhbz`, which must build the RHBZ container,
run smoke plus the catalogue phase, and clean up.
