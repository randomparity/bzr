# ADR 0038: Normalize user and group values at the wire boundary

## Status

Accepted

## Context

Bugzilla's user and group REST resources expose several values in more than one JSON shape.
Resource IDs and create-result IDs can be JSON numbers or decimal strings, while `can_login` and
`is_active` can be booleans or binary integers. The crate already has shared adapters for exactly
those domains under ADR 0033, but the user, group, create-response, and `whoami` types remain
strict. A parse failure after `user create` or `group create` is particularly harmful because the
server mutation has already committed.

Two request contracts are also wrong independently of deserialization: group membership lookup
sends the ignored singular `group` parameter instead of Bugzilla's recognized repeated `groups`
form, and user updates serialize `real_name` instead of the accepted `full_name` field.

## Decision

Apply ADR 0033's unsigned number-or-string and optional bool-or-binary-int adapters at every
user/group field named by issue #625. Small module-local serde wrappers supply field-specific
diagnostics while delegating the accepted-value domain to the shared adapters. The generic
`IdResponse` adopts the unsigned adapter once, so all endpoints already using that response type
receive the same post-mutation protection.

Send group lookup as `groups=<name>` plus the existing `match=*`, without
`include_disabled=1`. Map the command-domain `real_name` member into `full_name` with a private
wire request in the user client; the CLI, JSON input, dry-run output, and published output key
remain `real_name`.

Extend the existing production-shape proxy with explicit user/group transformations and route
logs. Functional tests must prove the transformed response was observed, cover both create-result
IDs, distinguish native `whoami` on Bugzilla 5.3+/BMO-derived servers from the user-lookup fallback
on 5.0/5.2, and exercise the credentialless group-list path as the stock server's expected access
denial. Successful group shape/filter proofs use credentials because stock Bugzilla rejects
anonymous `match=*` before returning a body. The JSON schema version and schema files do not change
because serialization remains unchanged.

## Consequences

- User and group reads tolerate the observed alternate wire shapes but still reject negative,
  fractional, non-decimal, and out-of-range IDs and non-binary integer booleans.
- Every create endpoint using `IdResponse` accepts a decimal-string ID. This is a necessary effect
  of fixing the existing shared response boundary once rather than specializing user and group
  request helpers.
- `group list-users` continues to hide disabled users, preserving its current visibility behavior.
- The production-shape proxy remains a small sequence of explicit transformations. Issue #634 may
  later generalize its registry, but this change does not depend on that refactor.
- No published JSON key or value domain changes, so `SCHEMA_VERSION` stays `2.0.0`.

## Considered & rejected

- **Add endpoint-specific create response structs for users and groups.** judgment: this would
  duplicate an identical post-mutation parse contract while leaving the existing shared
  `IdResponse` hazard in place for its other callers.
- **Add `include_disabled=1` to group lookup.** verified: issue #625 marks this only as a
  consideration, while its executable criterion requires excluding an enabled non-member; adding
  the parameter would widen visibility without a sourced requirement.
- **Build the generalized proxy registry proposed by #634 first.** verified: #634 is an optional,
  separately owned refactor and the campaign dispatch explicitly excludes it as a blocker; direct
  transformations fit the existing proxy used by the required proof.
- **Rename or serde-alias the command-domain field to `full_name`.** judgment: the same type is
  serialized into dry-run output, so changing its serde name would fix the wire at the cost of an
  unrelated public payload change. A private wire request isolates the two contracts.
