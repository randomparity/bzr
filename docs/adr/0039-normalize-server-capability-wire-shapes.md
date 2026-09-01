# ADR 0039: Normalize server capability wire shapes at deserialization

## Status

Accepted

## Context

`server capabilities` currently models `/rest/parameters.maxattachmentsize` and
`/rest/field/bug.fields[].type` as JSON numbers. Stock Bugzilla stringifies every
parameter. verified: `Bugzilla/WebService/Bugzilla.pm` at upstream commit
`276673ab67562bd27676d8c3a3770ec9897637b8` assigns each parameter through
`type('string', ...)`. Issue #626 also authorizes compatibility with deployments that
stringify field type codes; that shape is a compatibility policy proved with a synthetic
proxy fixture, not a claim that stock Bugzilla emits it. The parameter mismatch makes
credentialed attachment limits permanently `null`; a string field type can fail the
entire command.

Bugzilla also emits an empty-name status pseudo-entry. verified:
`Bugzilla/WebService/Bug.pm` at upstream commit
`276673ab67562bd27676d8c3a3770ec9897637b8` prepends `{ name => '' }` in the
`bug_status` legal-values branch. The local `src/client/resources/server.rs` at base
commit `fa230aec233a9d61609c11d8d0a3df6ac9b72e8b` filters only null names and reports
one debug message for response-shape, HTTP, permission, and transport failures.

Accepted ADR 0033 already owns the exact non-negative integer-or-decimal-string wire
contract. Accepted ADR 0005 requires the parameter fetch to remain credential-gated and
best-effort, with `null` on the credentialless path.

## Decision

Deserialize both capability integer fields through a server-local wrapper around ADR
0033's shared unsigned adapter. Keep the wrapper local because it exists only to attach
that adapter to optional and named response fields; do not add a generalized registry.
Give the wrapper a zero default for `FieldDef.field_type`, preserving the existing behavior
where an omitted `type` maps to `unknown`; malformed present values remain fatal. Map
field type values outside `i64` to the same existing `unknown` output instead of failing a
whole capability document.

Classify an optional attachment-limit failure at the existing best-effort boundary:
deserialization errors emit a `response_shape` reason and all request, HTTP, API,
permission, and transport errors emit a `request` reason. Both still degrade to `null`.
The messages remain API-key-redacted through `BzrError` display.

Discard status values whose decoded name is absent or empty. Accept the suffix only when
the complete version is exactly `major.minor+`: if ordinary minor parsing fails, require
exactly two dot-separated components and one trailing `+` around decimal digits. Thus
`5.0+` remains Hybrid and `5.1+` and later remain REST while bare `5`, `5.1++`, and
`5.1+.2` retain their existing malformed fallback. Multi-component versions such as
`5.3.3+` already parse their ordinary minor component and remain REST.

Extend the existing production-shape proxy with one bounded, explicitly enabled
server-capability transform:
stringify `maxattachmentsize`, expose a string-typed synthetic custom field, insert an
empty status pseudo-entry, and emit a bare `5.2+` version. The functional auth phase will
enable that mode and assert the resulting public document and proxy evidence. Existing
proxy consumers retain the current default behavior.

## Consequences

Credentialed stock servers expose their attachment limit in bytes, while credentialless
servers still skip `/rest/parameters`. A malformed optional parameter remains non-fatal,
but its failure class is observable. Status transitions never publish an empty source.
The published JSON keys, nullability, and domains do not change, so schemas and
`SCHEMA_VERSION` remain unchanged.

The proxy fixture owns a deliberately synthetic custom field because stock fixtures need
not define one; its name makes that test-only provenance explicit. Proxy self-tests and log
assertions prevent a passing functional command from masquerading as shape coverage.

## Considered & rejected

- **Change the two fields directly to `String` and parse later.** judgment: this rejects
  number-shaped compatible servers and duplicates ADR 0033's contract.
- **Add a generic optional-number adapter registry.** judgment: issue #634 owns that wider
  abstraction; one local wrapper is the smaller surface for this response.
- **Return parameter failures instead of degrading to `null`.** verified: accepted ADR
  0005 explicitly makes only `max_attachment_size` best-effort and nullable.
- **Treat every non-numeric minor suffix as ignorable.** judgment: accepting only the
  evidenced trailing `+` avoids silently classifying malformed versions as modern.
- **Record the version item as R1-exempt without proxy proof.** verified: the existing
  `tests/functional/redhat-shape-proxy.py` already rewrites successful REST responses and
  can exercise `/rest/version` with a small bounded extension, so proof is proportionate.
