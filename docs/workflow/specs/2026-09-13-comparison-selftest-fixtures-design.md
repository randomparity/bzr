## Problem

The Python-bugzilla comparison self-test no longer represents the landed auth
and RHBZ-field phases, so it fails before a stock comparison can run.

## Scope

Update only the self-test's auth/report, RHBZ-smoke, and RHBZ-field fixtures.
Align the auth report expectations with the current parity document, provide
every auth input the phase consumes, initialize the smoke fixture's exchange
directory, and model the current RHBZ metadata endpoint and control shape.
Keep the fixture boundary-only: it must not contact a real server. No ownership
transition is needed; the production phases remain their current owners.

### Failure model

A stale report row, missing exchange directory or control value, retired
endpoint, or unsupported API failure classified as a general gap must make the
self-test fail. The sub-component shape error remains the sole allowed RHBZ
gap.

## Success

The bare self-test passes from a clean environment. Its auth and RHBZ fixtures
exercise current phase paths, reject their controlled regressions, and preserve
the intended gap classification.

## Validation

- focused-test: `bash tests/functional/pybz/container-tests.sh` proves the
  self-test contract end to end.
- focused-test: fixture controls prove stale auth/report entries, metadata
  routing, missing RHBZ controls, and unsupported BZR field shape are detected.
