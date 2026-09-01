# ADR 0034: Use `platform` for Bugzilla's hardware field

## Status

Accepted

## Context

Bugzilla's REST and XML-RPC APIs expose the hardware field as `platform`, but `bzr`
requests, deserializes, mutates, and publishes it as `rep_platform`. The wrong REST
`include_fields` name suppresses the real field. The published JSON key and the CLI
`--rep-platform` flag are compatibility surfaces governed by ADR 0007's one-release
transition policy.

## Decision

Use `platform` as the canonical name in Bug domain types, REST/XML-RPC adapters,
create/update payloads, field selection, search mapping, output, schemas, and docs.
For schema version 2.1.0, serialized bug output includes canonical `platform` and the
deprecated `rep_platform` alias. Create JSON accepts both names for this release but
rejects supplying both. The CLI accepts canonical `--platform` and hides the deprecated
`--rep-platform` alias; the alias is scheduled for removal with the published key alias
after this one-release transition.

## Consequences

Real servers populate platform on reads, clones preserve it, and updates call the API
with the accepted external name. Published payloads temporarily duplicate the value.
Consumers should move to `platform` during 2.1.x. Templates retain their existing
`rep_platform` configuration field because they are a separate persisted configuration
surface and this issue does not authorize a template migration.

## Considered & rejected

- **Keep `rep_platform` internally and rename only at the wire.** judgment: retaining
  two meanings for one field would preserve the naming drift that caused the defect.
- **Remove aliases immediately.** verified: issue #621 and ADR 0007 require a minor
  release with a one-release alias for a published-key rename.
- **Keep the CLI alias indefinitely.** verified: the operator decision recorded for
  issue #621 requires deprecation and removal on the same one-release schedule.

