# Scope-qualified dependency preflight design

## Goal

Allow the bundled dependency collector to operate against Bugzilla installations that reject
searches without terms, without weakening its classification of inaccessible bugs or its
structured-error contract. This implements issue #591 and ADR 0026.

## Scope and invariants

The change is limited to the dependency-analysis skill, its installed documentation and tests,
and the functional scenario needed to reproduce the production search policy. It does not change
the native `bzr bug list` contract or probe production hosts.

The collector continues to preflight every declared server exactly once before any resource read.
Each preflight is bounded to one row and contains a term derived from that server's canonical
declared scope. A server is recorded as reachable only after a valid structured success response.
Code 102 is classified as `inaccessible` only for a server with that proof. Any probe failure is
command-fatal; a valid API code 1000 envelope is reported as `api`, never `malformed-output`.

## Probe selection

For each server, the collector first aggregates every bug ID from all matching bug-ID scopes and
uses the global minimum when that set is non-empty. Otherwise it sorts the server's ordinary scopes
by `(scope-kind rank, canonical normalized JSON)` and selects the first. The canonical JSON uses
sorted keys and compact separators, so caller array order cannot break a same-kind tie. An alias is
already unique per server by policy validation. Saved-query, custom-search, product, milestone,
and version scopes reuse their normal enumeration command with
`--limit 1 --offset 0 --fields id --sort bug_id --order asc`.

The optional restriction participates only when no ordinary scope names that server. Policy
validation already guarantees that every declared server appears in the scope/restriction
universe, so probe selection cannot be empty after validation.

For explicit resources, the proof is a filtered search operation, distinct from the later detail
operation. Bugzilla search omits invisible bugs rather than disclosing them, so an empty successful
search still proves server and credential reachability; the detail read may then classify code 102.
If a server instead returns code 102 from the search, the proof did not succeed and collection
stops. A live mixed-access test places the inaccessible ID below a public ID so selection cannot
accidentally avoid this case.

## Error handling

The existing command runner parses versioned success and error envelopes. Probe success validates
the returned ID page and marks the server reachable. Probe failure uses the existing fatal
limitation mapping, preserving `api_code` validation before reducing shareable output to the safe
error type. There is no fallback from code 1000 because every generated probe has a term; receiving
1000 is evidence of another server incompatibility and remains visible as a structured API class.

## Functional production-fidelity scenario

The functional phase installs the real bundled skill and points it at the existing Red Hat behavior
proxy. The proxy returns the same code-1000 REST error shape as production when it sees a termless
`/rest/bug` search, while forwarding scoped requests to the real local Bugzilla container. The
phase first proves the proxy rejects the former command shape, then proves collection of an
explicit public bug succeeds through the proxy.

A second installed-collector case uses a Custom Search URL with an allowlisted but empty search
value. The proxy therefore observes an effectively termless `/rest/bug` request and returns its
production-shaped code-1000 envelope. The assertion requires a partial collection with
`collection-api` and the sanitized `collection failed: api` diagnostic, proving the installed
collector's error path as well as the source fixture.

This makes production policy an explicit compatibility dimension rather than assuming that stock
upstream version coverage represents deployed policy and response behavior.

## Threat model

### Boundary inventory

- Existing boundary: policy-controlled server aliases and scope values become child-process argv.
  The change selects among already validated scope fields and continues argv-form execution without
  a shell.
- Existing boundary: an external Bugzilla response becomes a structured success or error envelope.
  Existing schema and ID-page validation remain mandatory before reachability is recorded.
- Test-only boundary: HTTP requests enter the existing production-behavior proxy. It recognizes
  termless `/rest/bug` searches and otherwise forwards method, path, bounded body, and headers to
  the local container.

No new production entry point or privilege is added.

### Actors and controls

The local operator controls policy input; the configured Bugzilla controls HTTP responses. Existing
policy validation bounds scope shapes and values, subprocess argv avoids shell interpretation, and
structured-envelope validation prevents raw server messages from entering collection artifacts.
The functional proxy is controlled by the repository test harness, runs only in tests, and forwards
only to its explicit loopback backend.

### Out of scope

This design does not emulate every production Bugzilla customization or infer inaccessible bugs
without independent reachability evidence. A compatibility matrix beyond the observed termless-
search policy remains future test-harness work.

## Verification

- Unit tests cover deterministic probe argv for every scope kind, global-minimum selection across
  permuted same-kind scopes, and restriction-only servers.
- Collector tests prove probes are once-per-server, term-qualified, and precede resource reads.
- A code-1000 probe fixture proves the failure remains `collection-api` / `api`.
- Existing code-102 and rejected-credential tests remain green; a mixed inaccessible/public scope
  proves a search for the selected inaccessible ID can establish reachability before detail reads.
- Functional phase 18d proves the installed payload against the local container through the
  production-policy proxy, including both scoped success and structured code-1000 failure.
