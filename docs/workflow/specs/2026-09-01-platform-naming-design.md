# Platform naming correction design

## Scope and outcome

Issue #621 requires `platform` to be the canonical Bugzilla hardware-field name on
REST and XML-RPC reads, create/update writes, CLI input, field projection, and published
JSON. It also requires schema 2.1.0, a one-release compatibility alias, corrected fixtures,
and live readback across Bugzilla 5.0, 5.2, and 5.3. ADR 0034 records the compatibility
decision. Unrelated epic #616 conformance work and template persistence migration are out
of scope.

## Design

The `Bug` domain and REST wire representation expose `platform`. REST default fields ask
for `platform`; XML-RPC reads the same key. `BugField::Platform` is canonical while
`rep_platform` remains a selection alias for one release. Manual serialization emits
both `platform` and deprecated `rep_platform` with identical values during schema 2.1.x.

Create and update payloads serialize `platform`. Create CLI/JSON, clone overrides, and
update CLI/JSON use `platform`. The legacy CLI spelling remains a hidden clap alias, so
existing scripts work without advertising the deprecated spelling. Create JSON accepts
the legacy key as a serde alias; clap/serde reject conflicting duplicate input rather than
choosing an order-dependent winner. The published create schema encodes the same at-most-one
constraint for both a single object and each array item, with schema validation tests for
canonical-only, alias-only, and conflicting inputs.

Search already exposes `--platform`; its mapping is corrected so every external and
internal name is `platform`. Output headers remain the user-friendly `PLATFORM`.

The schema version becomes 2.1.0. Bug output schemas document both keys for the transition,
with `platform` canonical and `rep_platform` deprecated. Create input documents the
canonical key and the deprecated accepted alias. CLI reference, README examples, embedded
release-readiness guidance, and its fixture move to the canonical name.

## Error handling and compatibility

No new transport error is introduced. Bugzilla errors remain structured `BzrError`
responses. Supplying both canonical and legacy JSON names is rejected as a duplicate
field. `--rep-platform` remains accepted but hidden for exactly the published alias
transition; removal is deferred to the next schema release.

## Verification

Unit tests first correct the clone fixture to `platform`, demonstrating failure against
the pre-fix deserializer. Serialization tests require both 2.1.x keys to agree; payload
and CLI tests require canonical write names and the legacy CLI alias. Focused tests run
through `make test-one T=<substring>`, then `make test-fast`, `make lint`, and `make test`.
Functional phase assertions create, view, update, and clone a platform and read every
value back. The existing cross-transport sequence also requires identical populated
`.platform` values through REST, Hybrid, and forced XML-RPC. `make functional-test-all`
executes those checks on bz50, bz52, and bz53.
