# Red Hat multi-valued bug fields design

## Scope and goal

Issue #589 requires bug reads to accept stock scalar and Red Hat array forms of
`component` and `version`, retain strict malformed-value rejection, publish a
deterministic representation, and prove the production-shaped path against a
real local Bugzilla transport. ADR 0025 governs the representation.

The change does not alter mutation payloads, introduce runtime vendor modes, or
normalize unrelated fields.

## Contract

`Bug.component`, `Bug.version`, and `BugAdjacencyBug.version` become
`Option<Vec<String>>`. Their JSON schema is `null` or an array of strings.
Deserialization maps a non-empty scalar string to one element, retains the
existing absent interpretation of an empty scalar, and preserves arrays,
including empty and multi-value arrays, in server order. Missing fields remain
`None`. Explicit null and every non-string/non-string-array shape fail instead
of being coerced. Table and detail renderers join present values with `, `.

## Components and data flow

A private `deserialize_optional_string_list` helper in `src/types/bug.rs` owns
the strict wire normalization and is reused by strict adjacency parsing. Bug
serialization remains derived from the normalized public fields. Output
renderers consume slices and do not inspect the original wire shape.

The functional phase starts a Python stdlib reverse proxy in front of the
already-running Bugzilla container. The proxy forwards actual REST GETs and
POSTs, rejecting invalid or greater-than-1-MiB request bodies before reading,
parses successful JSON responses, and rewrites only bug-object `component` and
`version` values into captured Red Hat-style arrays. A readiness endpoint must
pass before `bzr bug list` and `bzr bug adjacency` target the proxy. Cleanup is
composed with and restored to the existing phase lifecycle, including an early
phase failure. Stock functional arms continue unchanged.

## Error handling

The deserializer reports the field as requiring a string or an array of
strings. Array order and duplicates are preserved because neither the issue nor
Bugzilla defines set semantics. Proxy startup readiness, upstream failure, and
malformed upstream JSON fail explicitly. The phase records the proxy log path
on failure and asserts that the proxy process is gone after cleanup.

The normalized array representation is a breaking public JSON retype, so ADR
0007 requires `SCHEMA_VERSION` `0.6.2` to advance to `1.0.0`; current docs,
embedded-skill consumers, and functional assertions move with it.

## Threat model

- Boundary: an untrusted Bugzilla response enters serde. Control: accept only
  strings or arrays whose every member is a string; reject null and all other
  JSON types through the existing deserialize error path.
- Boundary: the test proxy receives loopback GET and POST requests from the
  test process and forwards to the selected loopback container port. Control:
  fixed loopback bind/target arguments, a 1-MiB request-body limit with strict
  `Content-Length` validation, and no authentication logging.
- Actors: a configured Bugzilla server controls response JSON; a local test
  operator controls proxy arguments. No new production listener is added.
- Out of scope: validating the semantic existence of returned component or
  version names, and reproducing every Red Hat extension behavior.

## Acceptance tests

- Unit tests demonstrate red-before-green for missing, scalar, empty, single,
  multi, null, numeric, object, nested, and mixed-element inputs.
- Resource/command tests cover Red Hat-shaped search/list output.
- Strict adjacency tests cover scalar and array `version` plus malformed data.
- Schema tests require array-or-null contracts.
- Proxy self-tests cover rewriting, readiness, malformed upstream JSON, and
  upstream failure. The phase verifies cleanup and names the diagnostic log.
- Functional tests run stock coverage and a Red Hat-shaped proxy scenario over
  a real local Bugzilla for list/search and adjacency.

## Workflow checkpoint

Branch: `feat/red-hat-multivalue-fields-589`; base: `main`. Guardrails:
`make test-one T=<name-substring>`, `make test-fast`, `make lint`, `make test`,
and `make functional-test-all`.
