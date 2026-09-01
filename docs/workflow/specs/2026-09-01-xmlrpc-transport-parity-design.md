# XML-RPC transport parity design

Issue: [#622](https://github.com/randomparity/bzr/issues/622)
Decision: [ADR 0035](../../adr/0035-normalize-xmlrpc-resource-timestamps.md)

## Goal

Make XML-RPC bug, comment, and attachment timestamps use the same canonical UTC RFC3339 spelling
as REST, and make XML-RPC attachment list preserve the flags already returned by attachment view.

## Scope and constraints

- Rust remains at 1.89.0 and no dependency is added.
- `get_datetime_str` accepts Bugzilla's compact `YYYYMMDDTHH:MM:SS` form and canonical
  `YYYY-MM-DDTHH:MM:SSZ`; both produce `YYYY-MM-DDTHH:MM:SSZ`.
- Only the exact fixed-width ASCII compact form is rewritten. Other non-empty strings retain the
  current pass-through behavior; empty, missing, and non-string/non-datetime values remain `None`.
- `ATTACHMENT_LIST_FIELDS` gains `flags`; list and by-ID mappings continue using `get_flags`.
- REST production code, public field names, schemas, and `SCHEMA_VERSION` remain unchanged.
- Functional proof is specific to Bugzilla 5.0, the supported Hybrid/XML-RPC server. Other matrix
  arms record skips for the new bz50-specific assertions and continue running their existing tests.
- The campaign-assigned ADR is 0035. Its index row remains pending for the campaign orchestrator.

## Components and data flow

`src/xmlrpc/resources/mappers.rs` owns a small private compact-shape recognizer. For both
`Value::DateTime` and a non-empty `Value::String`, `get_datetime_str` passes the value through that
normalizer. A valid compact shape has eight ASCII date digits, `T`, and a colon-delimited six-digit
time. The output inserts the two date dashes and terminal `Z`; a canonical value is unchanged.

`src/xmlrpc/resources/attachment.rs` adds `flags` to the XML-RPC list request's `include_fields`.
The existing `value_to_attachment` path already converts the returned array with `get_flags`, so no
new flag representation or parser is introduced.

The attachment functional phase already creates a bug, attachment, comment history, and a review
flag. Immediately after that flag is established, bz50-only assertions capture canonical JSON
timestamp projections for `bug view`, `comment list`, and `attachment list` under explicit `rest`
and `xmlrpc` modes and compare the resulting bytes. A fourth assertion selects the same flagged
attachment from XML-RPC list and view, requires a non-empty review flag, and compares the complete
sorted flag arrays.

## Error handling and compatibility

The mapper remains infallible and optional. It performs no calendar validation and introduces no
new `BzrError`: Bugzilla remains responsible for producing valid date components. Exact known
transport shapes normalize; unknown non-empty values remain observable rather than disappearing or
turning a previously tolerated response into a command failure.

## Testing and acceptance

- Correct the mapper fixture to the production compact XML-RPC spelling and first run
  `make test-one T=get_datetime_str` against the old implementation. The expected controlled fault
  is compact output where the test requires RFC3339.
- Add unit cases for compact `Value::DateTime`, compact string, already-canonical input, and the
  existing empty/type/missing fallthroughs.
- Strengthen the attachment request test to require `<string>flags</string>` in `include_fields`.
- Run focused mapper and attachment tests, then `make test-fast`, `make lint`, and `make test`.
- Run `make functional-test-all`; the bz50 arm must pass all four new parity assertions and the
  bz52/bz53 arms must report their explicit skips.

## Rollback

Revert the mapper and field-list commit plus its tests. No persisted data, migration, server state,
or schema cleanup is involved; functional fixtures are created and cleaned by the existing harness.
