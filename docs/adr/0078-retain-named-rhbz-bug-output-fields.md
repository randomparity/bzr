# ADR 0078: Retain named RHBZ bug output fields

## Status

Accepted

## Context

The generic bug model retains unknown wire fields only when their names begin
with `cf_`. RHBZ exposes `target_release` and `sub_components` as non-`cf_`
extension fields, so their values disappear before bzr's shared projection and
output paths can use them.

## Decision

Recognize exactly `target_release` and `sub_components` as dynamic readable
bug fields alongside `cf_*`. Keep their raw JSON values in the existing map
and reuse the current projection and output machinery. Missing fields on stock
servers retain the existing dynamic-field absence semantics.

## Consequences

Both transports preserve RHBZ's arrays and objects without inventing a local
shape. The accepted read surface grows by two names only; arbitrary non-`cf_`
fields remain unknown, and write behavior is unchanged.

## Considered & rejected

- **Accept every unknown non-`cf_` field.** judgment: that changes the output
  contract beyond the two server-proven names in issue #791.
- **Add typed members to `Bug`.** judgment: callers need the server shapes
  intact, and the existing dynamic-value representation already provides that.
- **Drop unsupported stock responses as an error.** verified: existing dynamic
  `cf_*` projections model absent server fields as absent output values.
