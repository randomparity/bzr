# Server capability wire conformance design

Issue: #626
ADR: [0039](../../adr/0039-normalize-server-capability-wire-shapes.md)

## Outcome

Make `bzr server capabilities --json` report the stock-server attachment limit, tolerate
the documented production integer shapes, omit Bugzilla's empty pseudo-status, and choose
REST for bare suffixed 5.1+ versions without changing its published schema.

## Requirements and boundaries

- A credentialed bz50, bz52, and bz53 run reports non-null
  `max_attachment_size`; the value remains normalized from KiB to bytes.
- The credentialless named `public` server still skips `/rest/parameters` and reports
  `null`, as ADR 0005 requires.
- The best-effort error path distinguishes response decoding from request, permission,
  HTTP, API, and transport failures without exposing credentials.
- A controlled string-shaped `maxattachmentsize` fault fails before the fix and passes
  after it.
- No output transition has an absent or empty `from` value.
- `/rest/field/bug` field type accepts JSON numbers and decimal strings through ADR
  0033's adapter. A production-shape proxy invocation proves the string arm reaches the
  public capability output.
- `5.0+` remains Hybrid; `5.1+`, `5.2+`, and later major versions use REST. Bare `5`
  remains Hybrid, and unrelated malformed versions retain the current fallback.
- `schemas/*.json` and `src/output/mod.rs::SCHEMA_VERSION` remain byte-for-byte unchanged.
- #634's generalized registry and all other epic #616 entries are excluded.

## Design

### Response normalization

`src/client/resources/server.rs` adds a private `UnsignedWire(u64)` serde wrapper whose
`Deserialize` implementation delegates to `u64_from_number_or_string`. Both
`ParametersBody.maxattachmentsize` and `FieldDef.field_type` use the wrapper, while
absence continues through `#[serde(default)]` on the optional parameter. `UnsignedWire`
has a zero default, and `FieldDef.field_type` retains `#[serde(default)]`, so an omitted
`type` keeps the existing `unknown` result; a malformed present value remains an error.
The field type mapping performs a checked `u64` to `i64` conversion and uses `unknown`
when the value is outside the existing mapper's domain.

`attachment_size_limit` keeps its `Option<u64>` contract. Its error match splits
`BzrError::Deserialize` from every other error and writes stable structured `reason`
fields to the debug trail. This is observability only: neither arm becomes fatal.

`status_transitions` filters the decoded status name before requiring a transition list.
An empty name is treated the same as the already-intended null pseudo-entry. Legitimate
statuses with no `can_change_to` remain omitted.

### Version parsing

`version_to_api_mode` parses the major and ordinary numeric minor as before. Only when
minor parsing fails does it accept the suffixed form, and then only if the complete
version has exactly two components and the second is decimal digits followed by exactly
one `+`. Unit cases pin the 5.0 boundary, 5.1+ boundary, bare-major behavior,
multi-component `5.3.3+`, and malformed `5.1++` and `5.1+.2` fallback.

### Executable proof

Unit tests replace the false numeric parameter fixture with the stock string shape, prove
the numeric compatibility arm, assert empty transitions are absent, and cover string and
number field types plus the established omitted-type-to-`unknown` fallback. Two tests use
`crate::test_helpers::TracingCapture` at DEBUG: a
malformed authenticated parameter value must remain non-fatal and emit
`reason=response_shape`, while HTTP 401 must remain non-fatal and emit `reason=request`.
At least one controlled response body contains the test API key plus a non-secret marker;
the captured trace must retain the marker while excluding the raw key, proving the
existing `BzrError` display-redaction seam rather than making a vacuous absence assertion.

The existing functional response-shape proxy gains an explicit server-capability mode and
a single transformation function for those endpoints. The default mode leaves capability
routes and `/rest/version` unchanged, preserving existing proxy consumers. In capability
mode it:

1. emits `5.2+` at `/rest/version`;
2. stringifies `maxattachmentsize` at `/rest/parameters`;
3. inserts a test-named custom field with string type at `/rest/field/bug`; and
4. inserts an empty-name status into `/rest/field/bug/bug_status`.

The function returns named counters and logs each applied shape. Proxy self-tests cover
each route, prove default mode leaves capability routes unchanged, and prove unrelated
payloads are unchanged. The auth phase starts the proxy in capability mode, installs a
combined harness/proxy EXIT trap, runs a credentialed inline
`server capabilities`, asserts non-null attachment size, the mapped proxy field, no empty
transition, REST mode, and every expected proxy log, then stops the proxy and restores the
ordinary harness trap. Existing stock and credentialless assertions stay in the same
phase; therefore the all-version run proves both sides on bz50, bz52, and bz53.

## Failure handling

- Malformed proxy backend JSON returns the proxy's existing bounded 502 response.
- Missing credentials short-circuit the optional parameters request and return `null`.
- A malformed parameters payload returns `null` with `reason="response_shape"`.
- Request, permission, API, HTTP, and transport errors return `null` with
  `reason="request"`.
- Out-of-range field type codes produce `unknown`; malformed non-decimal wire values fail
  the required all-fields request rather than inventing a type.
- Proxy startup or missing rewrite evidence fails the functional case.

## Threat model

### Boundary inventory

- Existing boundary widened: remote Bugzilla JSON enters serde for parameters, fields,
  statuses, and version. The design widens only decimal-string acceptance for two
  non-negative integer values and one documented suffix.
- Existing boundary observed: an authenticated API key crosses the inline client-to-proxy
  path and is forwarded to the local Bugzilla container. No new credential store or output
  path is added.
- Test-only boundary added: the local proxy rewrites successful backend JSON and caps
  request bodies with its existing 1 MiB bound.

### Actors and controls

- A remote Bugzilla administrator controls response values. Serde rejects non-decimal,
  negative, fractional, boolean, object, and out-of-range integer shapes; checked
  conversion bounds field type mapping.
- A local operator controls the configured API key. Existing auth application and
  `BzrError` display redaction remain the only credential paths; new logs format the
  redacted error and never the raw response or request headers.
- Functional tests control the loopback proxy. It remains bound to `127.0.0.1`, preserves
  the existing body bound, strips hop-by-hop headers, and rewrites successful JSON only.

### Out of scope

The design does not authenticate arbitrary proxy clients, harden the test proxy for
non-loopback deployment, or change Bugzilla authorization policy. Those paths are not
reachable in the shipped binary.

## Verification

- `make test-one T=server_capabilities`
- `make test-one T=version_to_mode`
- `python3 tests/functional/redhat-shape-proxy.py --self-test`
- controlled fault against the string parameter test, then the same focused test green
- `make test-fast`
- `make lint`
- `make test`
- `make functional-test-all`
