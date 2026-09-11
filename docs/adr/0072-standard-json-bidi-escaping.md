# ADR 0072: Standard JSON escapes for bidi controls

## Status

Accepted. Issue #757; operator approved the design and unchanged schema version.
Amends ADR 0065's JSON exclusion; its table policy continues to apply.

## Context

JSON success and error writers emit the twelve bidi controls selected by ADR 0065
literally. A server-controlled string can therefore reorder a JSON document's
visual presentation. ADR 0065 incorrectly states that escaping these characters
requires nonstandard JSON. [RFC 8259 section 7](https://www.rfc-editor.org/rfc/rfc8259#section-7)
permits a backslash, `u`, and four hexadecimal digits for BMP characters.

## Decision

After serialization, replace only the existing twelve bidi controls with standard
JSON escapes, such as `\u202e`, in pretty JSON and NDJSON success and structured
error output. Reuse the existing character set. This covers keys and nested string
values without altering decoded data or record boundaries. Ordinary Unicode and
non-bidi `Cf` remain unchanged, following #758's retained policy.

Keep `SCHEMA_VERSION` unchanged, as explicitly approved by the operator. ADR 0007
versions envelope and payload shapes; their fields, types and decoded values do
not change. This decision settles #757's conditional bump request.

## Consequences

Raw output displays ASCII escape spellings instead of active bidi controls.
JSON parsers recover the original strings, so consumers that decode and display
strings must apply their own presentation escaping. Literal backslash escape text
stays literal. A bounded character scan adds linear work and an output allocation;
no new dependency, schema document, or domain transformation is needed.

## Considered & rejected

- **Rust-style escapes.** verified: RFC 8259 section 7 permits four hexadecimal
  digits after `\u`, not braces; `\u{202e}` would produce invalid JSON.
- **Escape before serialization.** judgment: changes decoded values to escape text
  and confuses literal backslashes with the original control characters.
- **Custom serializer formatter.** judgment: serializer configuration at several
  seams adds machinery where a shared post-serialization scan is sufficient.
- **Document the residual only.** judgment: leaves the reported raw-output display
  hazard present despite an interoperable encoding being available.
