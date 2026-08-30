# ADR 0025: Normalize multi-valued bug fields to arrays

## Status

Accepted

## Context

Stock Bugzilla serializes a bug's `component` and `version` as strings. Red Hat
Bugzilla serializes the same requested fields as arrays, including arrays with
more than one value. Treating them as strings rejects valid production reads;
choosing only one array member would discard server data. Both fields are part
of the published `bug` JSON contract; `version` is also part of the narrower
`bug-adjacency` contract.

## Decision

The read model represents `component` and `version` as `Option<Vec<String>>`.
JSON output is therefore `null` when absent and an array when present: a stock
non-empty scalar becomes a one-element array, an empty stock scalar retains its
existing absent/null behavior, an empty server array remains empty, and a
multi-element array preserves its order. Human-readable output joins values
with `, `. A shared strict deserializer accepts only a string or an array of
strings; null, numeric, object, nested-array, or mixed-element values fail.
Because this is a breaking retype of published JSON fields, ADR 0007 requires
the envelope `SCHEMA_VERSION` to advance from `0.6.2` to `1.0.0`.

The functional harness retains its stock Bugzilla runs and adds a stdlib HTTP
response-shaping proxy for a Red Hat compatibility scenario. The proxy forwards
requests to the real local Bugzilla and changes only `component` and `version`
in successful bug response objects, so the complete CLI/client/HTTP path remains
under test without depending on a public production service.

## Consequences

- Consumers receive one deterministic JSON type for every present value.
- Rust callers and published schemas must handle lists instead of scalar text.
- Envelope-aware consumers can detect the breaking contract at version `1.0.0`.
- Stock and Red Hat wire shapes share validation and output behavior.
- The compatibility proxy is an explicit captured deployment profile, not a
  claim that stock Bugzilla itself emits Red Hat's extension shape.

## Considered & rejected

- **Keep scalar output and choose the first array member.** judgment: this
  silently loses valid production data and makes multi-value support misleading.
- **Preserve whichever scalar-or-array shape the server sent.** judgment:
  equivalent data would produce deployment-dependent public JSON types.
- **Call the public Red Hat service from functional tests.** verified: issue
  #589 records the production shape, while `tests/functional/run-all-versions.sh`
  runs hermetic local servers; an external dependency would make that suite
  credential, availability, and fixture-state dependent.
