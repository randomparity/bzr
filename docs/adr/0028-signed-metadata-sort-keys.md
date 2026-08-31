# ADR 0028: Model Bugzilla metadata sort keys as signed integers

## Status

Accepted

## Context

Bugzilla uses `sort_key` as an ordering weight rather than an identifier. Issue #594 records
negative values in production Mozilla field-value and product metadata responses; its retained
endpoint-level evidence does not distinguish whether the product value came from a version or a
milestone, and its accepted scope covers both nested product sort-key members. bzr models all three
fields as `Option<u64>`, so serde rejects otherwise valid metadata before `server capabilities` or
`product list` can produce output. The published JSON schemas also declare a zero minimum, so
changing only the Rust types would leave the external contract false.

The checked-in functional fixtures and phase-3 assertions construct only non-negative sort keys.
Running several upstream versions therefore varied server code without asserting the production
dataset shape that triggered the defect; this record does not claim every stock image will always
emit only non-negative metadata.

## Decision

Represent `FieldValue::sort_key`, `Version::sort_key`, and `Milestone::sort_key` as bounded
`Option<i128>` values over `[i64::MIN, u64::MAX]`. A shared serde adapter enforces that interval on
input and output. This accepts the signed production values without losing the previous `u64`
positive domain, and it matches the installed serde_json 1.0.151 `Value` integer representation
used by projected output. Keep resource identifiers and unrelated sort-key fields unchanged.
Replace the zero minimum on the matching field-value and product schema properties with
`minimum: -9223372036854775808` and `maximum: 18446744073709551615`.

This is a breaking payload-domain retype under accepted ADR 0007: a negative integer that is valid
after this change cannot be decoded by a consumer honoring the prior unsigned contract. Advance
`SCHEMA_VERSION` from `1.0.0` to `2.0.0` and update its direct tests, CLI documentation, installed
skill fixtures, current README example, and functional assertions. The JSON envelope shape does not
change. The public Rust fields are also retyped from `Option<u64>` to `Option<i128>`, so downstream
Rust users must adapt.

Extend the existing functional production-shape proxy to rewrite successful `/rest/field/bug`
and `/rest/product` metadata responses with negative ordering weights while preserving the real
container's endpoints and surrounding payload. The functional phase must prove the compiled CLI
accepts both affected command paths through that proxy. Proxy self-tests cover negative, zero, and
positive rewriting independently of the container's current fixture cardinality.

## Consequences

The three public metadata fields round-trip the `[i64::MIN, u64::MAX]` interval, including negative
values and the complete formerly accepted `u64` domain; out-of-range values fail deserialization
before reaching command output, and their schemas match that runtime contract. Envelope-aware
consumers can detect the breaking contract at version `2.0.0`; source consumers must accept the wider signed Rust type.
Unsigned identifiers retain their existing rejection of negative values. The production-shape arm remains an observed-
compatibility fixture rather than a claim that stock upstream datasets represent every deployed
Bugzilla customization.

## Considered & rejected

- **Keep the unsigned status quo.** judgment: this leaves `server capabilities` and `product list`
  unable to consume valid production metadata and preserves schemas that exclude the observed wire
  domain.
- **Keep schema version 1.0.0 because the change fixes rejected input.** verified: accepted ADR
  0007 defines `schema_version` over payload shapes, and the new signed domain permits emitted
  values that the 1.0.0 schema rejects; retaining the version would hide a breaking retype.
- **Coerce negative values to zero.** judgment: this would deserialize the response but destroy
  the ordering semantics the server supplied.
- **Accept any JSON number.** judgment: floating-point ordering weights are not in the observed API
  domain and would weaken the contract beyond issue #594.
- **Use `i64`.** verified: `i64::MAX` is smaller than the previous `u64::MAX` domain, while
  serde_json 1.0.151 accepts and serializes both `-1_i128` and `u64::MAX as i128`; `i128` preserves
  the former positive range without a wrapper type.
- **Accept the complete `i128` domain.** verified: serde_json 1.0.151 without
  `arbitrary_precision` accepts `i128` during typed parsing but its `Value` serializer rejects
  values below `i64::MIN` or above `u64::MAX`; the projected field and product writers use that
  `Value` path, so an explicit bound prevents command-dependent failure.
- **Change every `sort_key` field to signed.** verified: `rg -n 'sort_key: Option<u64>' src/types`
  at commit `0faa3df0d27a0365961485f16b3cb2538809e8b0` also finds classification metadata, while issue
  #594 limits the observed negative domain to field values, versions, and milestones.
- **Rely only on stock multi-version containers.** verified: the existing phase-3 product paths
  and checked-in fixtures at commit `0faa3df0d27a0365961485f16b3cb2538809e8b0` contain only
  non-negative sort keys, so version breadth does not construct the production input.
