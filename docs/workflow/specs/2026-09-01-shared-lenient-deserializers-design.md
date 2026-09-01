# Shared lenient deserializers design

Issue: [#620](https://github.com/randomparity/bzr/issues/620)
Decision: [ADR 0033](../../adr/0033-share-lenient-deserialization-adapters.md)

## Goal

Consolidate the repeated Bugzilla wire-shape coercions that accept unsigned integers as JSON
numbers or decimal strings and optional booleans as JSON booleans or integer `0`/`1`, without
changing any command-visible behavior.

## Scope and constraints

- Rust remains at the repository floor of 1.89 and no dependency is added.
- The adapters are crate-internal and live beside the existing shared `sort_key` adapter under
  `src/types/`; they do not change the public Rust or JSON schema surface.
- Product IDs continue to reject zero and negative values. Attachment boolean fields continue to
  preserve missing and `null` as `None`, accept only booleans and integer `0`/`1`, and reject
  strings, floats, and other integers.
- General API error codes remain signed and accept arbitrary signed decimal values in numeric or
  string form. They do not use the unsigned adapter.
- Relationship IDs remain a positive integer-or-object union and continue rejecting numeric
  strings. They do not use the string-or-number adapter.
- ADR 0024's strict adjacency resource codes and strict row mappings remain behaviorally
  unchanged. `src/types/sort_key.rs` remains unchanged.
- This is internal-only: no functional phase, CLI reference, schema, or changelog artifact changes.
  The full real-container matrix still runs before delivery.

## Components and data flow

`src/types/deserialization.rs` owns two serde-compatible decoders:

- `u64_from_number_or_string` returns a `u64` from a non-negative JSON integer or a decimal string
  using caller-supplied expectation and invalid-value messages. It rejects booleans, floats, null,
  negative numbers and strings, and overflow while preserving each consumer's established serde
  diagnostics.
- `option_bool_from_int_or_bool` returns `None` for null, `Some(bool)` for JSON booleans, and
  `Some(false)`/`Some(true)` for integer `0`/`1`. The caller's `#[serde(default)]` continues to
  supply `None` for an absent field.

The product-access response keeps its private element wrapper. That wrapper calls the configured
shared unsigned decoder with the existing product expectation and invalid-value messages, then
rejects zero with that same existing invalid-value message. The attachment type points its three
optional boolean fields at the shared optional-bool adapter. No other production consumer changes.

The general error-code parser stays in `client::response` because it accepts signed values and
provides the API error default of `-1`. The relationship parser stays in `types::bug::links`
because it accepts either a positive integer or an object containing `bug_id`, while deliberately
rejecting strings. Each receives a short invariant comment so later work does not merge distinct
wire contracts accidentally.

## Error handling

All malformed inputs fail through serde's deserialization error path. The shared unsigned adapter
uses checked conversion and parsing, so negative and out-of-range values never wrap. Product zero
remains a product-specific failure after successful unsigned decoding. The bool adapter accepts
only exact JSON integer values and never treats a numeric string as a boolean.

## Threat model

### Boundary inventory

- Existing boundary changed: Bugzilla-controlled JSON enters the shared unsigned and optional-bool
  adapters during response deserialization. No new endpoint or caller is added.
- Existing boundaries not widened: strict adjacency parsing, relationship unions, signed error
  codes, and signed sort keys retain their current implementations and accepted shapes.

### Actor model

The untrusted party is a configured Bugzilla server, reverse proxy, or extension that controls the
response body. The design trusts serde_json to classify JSON primitives and trusts Rust's checked
numeric conversion and parsing to reject values outside `u64`.

### Controls

- Primitive-shape visitors admit only named variants; all other serde visits use the default error.
- `u64::try_from` and `str::parse::<u64>` bound numeric input and prevent sign or overflow changes.
- Exact `0`/`1` matching bounds integer booleans; null and absence remain distinct from false.
- Consumer regression tests pin product positivity, attachment optionality, and strict-parser
  rejection. Errors expose only the malformed value already received from the server, never a
  credential or request URL.

### Out of scope

This design does not add response-size limits, authenticate server data beyond the existing TLS
policy, or make strict adjacency and relationship parsers lenient. Those concerns are either owned
by existing transport controls or explicitly excluded by #620 and ADR 0024.

## Testing and acceptance

- New sibling unit tests cover numeric and decimal-string unsigned values; boolean and integer-
  boolean values; null and absent optional booleans; and negative, overflow, float, string, array,
  and out-of-domain malformed values.
- Existing product resource tests remain green for number, string, mixed, zero, negative, and
  malformed IDs. Message-level assertions cover zero, a negative number, malformed and negative
  strings, and an invalid primitive so the refactor cannot change command-visible parse errors.
- Existing attachment tests remain green for boolean, `0`/`1`, null/absence, and malformed values.
- A strict adjacency regression includes a decimal string accepted by the tolerant signed error
  parser but outside ADR 0024's closed `100`–`102` set, and still rejects it.
- Existing relationship tests keep proving string IDs are rejected, and the `sort_key` file and
  its behavior remain untouched.
- `make lint`, `make test`, and `make functional-test-all` pass.

## Rollback

The refactor is reverted as one commit. It changes no persisted data or external contract, so no
migration or cleanup is required.
