# 0080: Persist checkout credentials only for Git consumers

## Status
Accepted (2026-10-15).

## Context
Issue #867 requests removing needless checkout credential persistence and
documenting genuine sibling exceptions. Checkout v7.0.1 stores credentials
under RUNNER_TEMP and configures later Git commands to use them.

## Decision
Set false on audited checkout steps without authenticated Git consumers.
Explicit true is retained for release homebrew's tap push and post-release
version-bump branch push, with comments naming those consumers.

## Consequences
Build code loses unnecessary checkout-configured credential access. Existing
trusted release Git pushes retain their authentication. Registry tokens and
GitHub release API tokens are independent and unchanged.

## Considered & rejected
- Leave implicit defaults: judgment: the consumer exception would remain unauditable.
- Disable persistence on push consumers too: verified: release.yml at faf34e31
  runs git push in homebrew's tap checkout and post-release-bump's checkout.
- Add a reusable credential wrapper: judgment: unnecessary surface for two trusted consumers.
