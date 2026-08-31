# Signed Bugzilla metadata sort keys design

## Goal

Accept negative Bugzilla ordering weights in field and product metadata without widening resource
identifier types or letting the published JSON schemas drift from runtime behavior. This implements
issue #594 and [ADR 0028](../../adr/0028-signed-metadata-sort-keys.md).

## Scope and invariants

The change covers exactly three nullable metadata properties: field-value, version, and milestone
`sort_key`. Each stores `i128` with a shared serde adapter enforcing `[i64::MIN, u64::MAX]`, so
negative, zero, and positive integer weights deserialize and serialize unchanged while the
previously accepted `u64` domain remains representable. The interval is the union of serde_json's
negative `i64` and positive `u64` `Value` representation used by projected output. Tests pin both
endpoints and one value outside each bound. Product, version, and milestone IDs remain `u64`;
classification metadata and all unrelated validation remain unchanged.

The field-value and product schemas continue to require the same properties and integer JSON type.
Their bounds become `minimum: -9223372036854775808` and
`maximum: 18446744073709551615`. The two nested product sort-key schemas also become
`type: ["integer", "null"]`, matching the existing `Option` fields and their serialized nulls;
field-value already declares that nullable type. Required-key lists remain unchanged. Schema
conformance tests use null, negative, and endpoint examples so nullability or bounds drift fails.
Because the repository's generic schema matcher does not interpret array-valued `type` unions, a
direct schema test must also assert that each of the three nodes has exactly
`["integer", "null"]`; a controlled wrong-union assertion proves that test can fail.

## Contract versioning

The signed domain is a breaking payload retype under accepted ADR 0007. A value newly emitted as
negative cannot be consumed under the prior unsigned 1.0.0 contract, even though accepting the
server response is a bug fix. `output::SCHEMA_VERSION` advances to `2.0.0`; the envelope shape is
unchanged. Direct hard-coded pins advance in the same change: output tests; current CLI
documentation and the current README example; the installed dependency collector's runtime
`BZR_SCHEMA_VERSION`; its current test fixtures and tests; current bzr-reference skill pages; and
functional assertions. A bounded search must leave no non-historical 0.6.0 or 1.0.0 schema-contract
pin. Historical ADRs and completed design records keep the version they documented at their own
decision point.

The public Rust fields also change from `Option<u64>` to `Option<i128>`. Resource IDs remain `u64`.

## Runtime flow

Bugzilla JSON continues through the existing serde resource models without a transport
normalization layer or intermediate model. The three fields use one shared serialize-and-
deserialize serde adapter to enforce `[i64::MIN, u64::MAX]`; supported signed values are retained at
the public type boundary, then emitted unchanged by JSON output. Commands that summarize field
metadata may discard the ordering weight from their final view, but they must still deserialize the
complete server payload successfully. Serialization tests construct one value beyond each bound so
public in-crate construction cannot bypass the output invariant silently.

Malformed types, floating-point values, integers below `i64::MIN`, and integers above `u64::MAX`
fail serde parsing. Negative resource IDs continue to fail through their existing unsigned types.

## Production-fidelity functional coverage

The stock functional containers prove endpoint and command integration, but the checked-in phase
and fixtures only asserted non-negative sort keys. The all-version matrix therefore never
constructed or required the production input that fails `u64`, regardless of what future stock
image data may happen to contain.

The existing loopback production-shape proxy will add one narrow response transform. On successful
`/rest/field/bug` and `/rest/product` responses, it replaces existing metadata ordering weights with
a deterministic cycle of negative, zero, and positive integers. It does not synthesize resource
IDs, bypass authentication, or replace the backend response. Proxy self-tests prove the transform.
Phase 3 then requires negative sort keys in compiled-CLI `field list status` and JSON `product list`
output through that same live proxy before requiring `server capabilities` success. After each
transform, the proxy writes and flushes a route-specific `metadata-sort-keys shaped` line with the
number of changed values. The phase requires positive field and product counts in that log, so a
future backend that naturally gains negative values cannot make a missing handler route false-green.
Immediately before `server capabilities`, the phase records the field-route event count with `awk`;
afterward it requires the count to increase, proving that invocation consumed a shaped field
response rather than relying on the earlier `field list` event.

This makes production wire-shape variation an explicit test dimension while retaining the real
container for routing, authentication, and surrounding Bugzilla behavior.

## Threat model

### Boundary inventory

- Existing boundary: a configured Bugzilla controls JSON metadata decoded into public Rust types.
  The change widens only three integer sign domains; serde still enforces integer kind and `i64`
  bounds used by the existing serde_json number representation.
- Test-only boundary: HTTP responses from a loopback Bugzilla container pass through the existing
  shape proxy. The proxy already bounds request bodies, filters hop-by-hop headers, and forwards
  only to its explicit loopback backend.

No production entry point, credential flow, or authorization rule is added.

### Actors and controls

The remote Bugzilla controls metadata values. Signed Rust integers preserve its ordering value while
rejecting non-integers and out-of-range integers. The local test harness controls the proxy; its
path-specific transform changes only successful JSON metadata responses and returns a 502 on
malformed backend JSON under the proxy's existing policy.

### Out of scope

This change does not validate whether a server's chosen ordering weights are sensible, emulate
every production customization, or broaden unobserved metadata fields. Those concerns are not
needed to accept the observed signed wire domain.

## Verification

- Type tests prove negative, zero, `i64::MIN`, and `u64::MAX` round trips for all three fields;
  values one outside either endpoint fail, and identifiers remain unsigned.
- Schema tests conform negative runtime values; published schemas have no zero minimum on the three
  signed properties and retain it on IDs; exact-node assertions prove the nullable integer union.
- Envelope and direct pinned-consumer tests report `SCHEMA_VERSION` 2.0.0; current CLI documentation
  uses the same value while historical records remain unchanged.
- Installed dependency-analysis collector tests run with `BZR_SCHEMA_VERSION` 2.0.0, and a bounded
  search finds no non-historical 0.6.0 or 1.0.0 schema-contract pin.
- Proxy self-tests prove targeted rewriting, unrelated payload preservation, and the three-value
  sign cycle.
- Functional phase 3 observes negative `field list` and `product list` metadata and proves
  `server capabilities` through the real-container production-shape proxy, then verifies positive
  route-specific rewrite counts in the flushed proxy log, including a field-event delta around the
  server-capabilities invocation.
- `make test-one T=<focused name>`, `make test-fast`, `make lint`, `make test`, and
  `make functional-test-all` pass.

## Durable workflow context

- Branch: `feat/signed-metadata-sort-keys-594`
- Base branch: `main`
- Host: arm64; shell unknown; BSD userland; tool-steering names `LC_ALL`, `LANG`, `GH_PAGER`
- Declared release targets: x86_64, aarch64, powerpc64le, and s390x Linux; aarch64 macOS; x86_64
  and aarch64 Windows. The recorded host/target relationship is `different`; this wire-model change
  is architecture-insensitive and CI covers the declared targets.
- Guardrails: `make test-one T=<name-substring>`; `make test-fast`; `make lint`; `make test`;
  `make functional-test-all`.
- ADR index coupling: not coupled; index row pending for the campaign orchestrator.
