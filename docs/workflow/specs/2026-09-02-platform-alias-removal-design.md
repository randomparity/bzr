# Platform compatibility alias removal

## Outcome

At schema major version 3.0.0, remove the one-release `rep_platform` compatibility
aliases from bug output, bug-create JSON, field projection, and bug create/clone CLI
parsing. Keep template configuration's persisted `rep_platform` field, as required by
ADR 0034.

## Design

The canonical `platform` field remains unchanged in domain types, wire requests, output
and command help. Delete only the compatibility serialization entry, serde alias, field
selection alias, and clap aliases. Bump `SCHEMA_VERSION` to 3.0.0, remove deprecated
properties and the create-input exclusivity rule from the two bug schemas, and update
documentation, embedded examples, fixtures, and functional phases to use `platform`.

Tests assert canonical behavior and rejection of each removed spelling. Template tests
remain unchanged. No transport, authentication, persistence, or dependency behavior is
changed.

## Verification

Run focused Rust tests, `make lint`, `make test`, and `make functional-test-all`.
