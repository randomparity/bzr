# Query match-type modifiers

## Problem

`bug list` and `bug my` currently select implicit substring or equality
operators, while python-bugzilla exposes explicit Bugzilla boolean-chart match
types. Users need the same operator choice without changing the existing
default or `!` shorthand behavior.

## Scope

Add `--status-whiteboard-type`, `--url-type`, and `--email-type` to the two
commands. Each maps its supported values to boolean-chart triples in both REST
and XML-RPC. Explicit types reject `!` values, which remain the shorthand for
the existing negating defaults. Update the comparison reset control rather
than repointing it to another parity gap. No ownership transition is needed:
the existing CLI args own parsing, `SearchParams` owns transport-neutral state,
and each transport owns serialization.

### Failure model

- Invalid operators fail during clap parsing.
- Explicit type plus `!` fails before either transport sends a request.
- Boolean-chart indexes remain unique when explicit and implicit negations mix.
- The comparison reset control fails if eligibility leaks between probes.

## Success

The listed modifiers accept all nine Bugzilla operators; REST and XML-RPC emit
the same field/operator/value semantics; old untyped filters remain unchanged;
and the functional comparison suite no longer treats #679 as a gap.

## Validation

- REST and XML-RPC focused tests assert boolean-chart triples and `!` refusal.
- CLI parsing tests cover each modifier and the `url` argument identity.
- `make test`, `make lint`, and `make functional-test` exercise the command
  and real-container behavior; comparison fixtures prove the reset control.
