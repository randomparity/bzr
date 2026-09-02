# ADR 0037: Remove unsupported component update

## Status

Accepted

## Context

`bzr component update` sends `PUT /component/{id}`, but stock Bugzilla 5.0 and
5.2 publish only component creation. verified: Bugzilla's
`WebService/Component.pm` lists only `create` as public and its REST component
resource routes only `POST /component`, as cited in issue #624. The live functional
phase confirms the mismatch by receiving API error 32614 and skipping the command.

ADR 0003 records why this command remained outside the shared admin-mutation seam.
It assumes the command itself remains valid, so removing the command supersedes that
decision.

## Decision

Remove `bzr component update` and every current published or executable surface that
exists only for it: CLI parsing and dispatch, request construction, client method and
type, dry-run capability, JSON input schema, current help/reference text, and tests.
Do not retain a hidden command, fork compatibility path, or deprecation alias.

The removed spelling is rejected by clap as an invalid subcommand. Component list,
view, and create remain the complete component command surface.

## Consequences

Stock Bugzilla users no longer see a command that cannot succeed. Completion and
schema discovery match the actual implementation. A user of a Bugzilla fork that
implemented the non-standard endpoint must use another client or maintain the removed
surface downstream. Reversion requires only reverting this code change; there is no
data migration.

## Considered & rejected

- **Retain the command with an actionable unsupported-server error.** verified: the
  stock servers cited in issue #624 provide no successful execution path, so this keeps
  a public command solely for unspecified forks and conflicts with the repository's
  no-phantom-features rule.
- **Hide or deprecate the command before removal.** judgment: another release of an
  unusable stock-server command adds transition surface without user value, and the
  operator explicitly selected immediate removal.
- **Keep only dry-run support.** judgment: a preview for an operation the supported
  server cannot commit is another misleading public contract.
