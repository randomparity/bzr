# ADR 0035: Normalize XML-RPC resource timestamps at the mapper boundary

## Status

Accepted

## Context

Bugzilla XML-RPC serializes datetimes as compact `YYYYMMDDTHH:MM:SS`, while its REST API emits
canonical UTC RFC3339 `YYYY-MM-DDTHH:MM:SSZ`. The shared XML-RPC resource mapper currently passes
both `dateTime.iso8601` and string values through unchanged. Bug, comment, and attachment commands
therefore publish different timestamp strings for the same server data depending on transport.
Attachment list requests also omit `flags` even though the mapper consumes them, so XML-RPC list
and view disagree. Verified against Bugzilla 5.0's `XMLRPC.pm` and `JSONRPC.pm` serializers and
`src/xmlrpc/resources/{mappers,attachment}.rs` at commit
`9c70c1d0947ff619bedad0e721a9ccf102565c3d`.

## Decision

Normalize timestamps inside `get_datetime_str`, the shared boundary used by XML-RPC bug, comment,
and attachment mappings. Convert the exact compact form `YYYYMMDDTHH:MM:SS` and dashed ISO form
`YYYY-MM-DDTHH:MM:SS` into `YYYY-MM-DDTHH:MM:SSZ`, and preserve already-canonical
`YYYY-MM-DDTHH:MM:SSZ` values byte for byte.
Preserve other non-empty string values rather than broadening this fix into a new response-error
contract. Use explicit ASCII shape checks and string assembly; add no date-time dependency.

Add `flags` to `ATTACHMENT_LIST_FIELDS`, keeping flag parsing in the existing shared mapper.
Functional coverage on Bugzilla 5.0 compares the timestamp projections from REST and XML-RPC for
bug view, comment list, and attachment list, then compares a flagged attachment's XML-RPC list and
view flag arrays.

## Consequences

The three resource paths publish the same UTC spelling for server-generated timestamps. Invalid or
extension-specific non-empty values retain the compatibility behavior they had before this change.
The normalizer is intentionally structural: Bugzilla is the date-validity authority, while the CLI
only repairs the documented transport spelling. XML-RPC attachment lists request one additional
metadata field and expose flags they already know how to decode. REST behavior, schema version, and
published field names do not change.

## Considered & rejected

- **Do nothing.** verified: issue #622 and `src/xmlrpc/resources/{mappers,attachment}.rs` at commit
  `9c70c1d0947ff619bedad0e721a9ccf102565c3d` show this preserves compact XML-RPC timestamps and an
  attachment list request that omits the `flags` field consumed by its mapper.
- **Normalize in each XML-RPC resource mapper.** judgment: three copies would encode one transport
  rule and invite the resource outputs to drift again.
- **Normalize in the output writers.** verified: `src/xmlrpc/resources/bug.rs`, `comment.rs`, and
  `attachment.rs` at commit `9c70c1d0947ff619bedad0e721a9ccf102565c3d` all use
  `get_datetime_str`, while the same public types also carry REST values; an output-layer rewrite
  would lose the transport boundary and unnecessarily touch REST output.
- **Normalize every XML-RPC `Value::DateTime` in the protocol decoder.** judgment: the protocol
  type is used beyond these public resource fields, so changing it would widen the behavioral
  surface beyond issue #622.
- **Add a date-time parsing dependency.** verified: `Cargo.toml` at commit
  `9c70c1d0947ff619bedad0e721a9ccf102565c3d` has no date-time dependency, and the two accepted
  fixed-width ASCII forms need only bounded shape checks and assembly.
