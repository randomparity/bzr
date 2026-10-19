# Alias and see-also bug reads (#887)

## Problem and authority

Bugzilla returns `alias` and `see_also`, but the typed bug model discards them
and the field registry rejects selections. Scope is frozen in
[#887 WORK:SCOPE](https://github.com/randomparity/bzr/issues/887#issuecomment-6195988501).
The operator approved the exclusions and subsequently approved the typed-array
design on 2026-10-15. Complexity M fixes the design denominator at 250 changed lines.

## Scope and design

Extend the existing owners: `Bug`/`BugWire` and its serializer retain two
`Vec<String>` fields; missing fields default to empty arrays, as keywords do.
Live Bugzilla 5.0.6 REST and XML-RPC reads confirmed populated and empty arrays.
The shared `BugField` registry owns canonical names, headers, and selection;
REST view and shared search request lists request these fields by default.
The XML-RPC adapter extracts the lists using its existing string-array mapper.
JSON/NDJSON preserve lists; table columns join values with comma-space; detail
rows use existing StringList rendering and omit empty lists. Terminal escaping
continues at the existing writer boundary. Default table columns stay unchanged.

Update `schemas/bug.json` to declare string-array properties and document the
read shape, selection, and empty values in `docs/bzr-cli.md`. Adjust direct Bug
constructors and registry/order expectations. There is no owner transition,
obsolete path, new facade, migration, dependency, or new mutation behavior.

Other missing Bugzilla fields belong to the backlog. Mutation behavior changes
require an operator scope decision if they block verification. Existing mutation
commands provide live round-trip fixtures. These are the approved exclusions.

## Success

- C1: REST and XML-RPC preserve populated arrays, order, and empty values for
  alias and see_also; omitted values serialize as empty arrays.
- C2: shared include/exclude selection works for both names in JSON, NDJSON,
  list tables and detail rows, with terminal controls escaped for human output.
- C3: real Bugzilla tests set alias and add/remove see_also through existing
  mutation commands, then read both transports with authenticated and public
  configurations; selection and empty-array checks run against the live server.
- C4: the published schema declares both string arrays and the CLI reference
  describes their output and projection. The additive output change increments
`SCHEMA_VERSION` to `3.0.10` under the existing JSON Output Stability policy;
functional and documentation version examples follow that value.

C1–C4 trace to the frozen charter; missing-value behavior and unchanged default
columns additionally trace to the operator-approved design.

## Failure model

- Actors and deployments: local CLI operators and automation reading Bugzilla
  REST/XML-RPC servers; authenticated and credentialless reads of public bugs;
  supported functional fixtures on Bugzilla 5.0/5.2/5.3.
- Invariants/assets: preserve server-returned valid lists without loss or reordering;
  retain projection semantics, JSON array shape, and terminal-output escaping.
- Accepted classes: malformed XML-RPC members retain the existing get_str_array
  tolerance (non-string entries ignored); no broader mapper hardening is promised.
- Covered elsewhere: server authorizes reads/mutations; existing response limits
  bound transport input; existing output escaping handles terminal controls.

## Threat model

- Added boundaries: none. Widened boundary: two server-controlled string arrays
  now enter Bug and flow to machine/human output.
- Actors: a remote server or bug author controls values. The CLI trusts server
  authorization, but treats returned text as untrusted for terminal output.
- Controls: REST serde enforces string arrays; XML-RPC uses existing mapper
  tolerance; bounded response pipeline limits input; writer escaping prevents raw
  terminal controls. Values are data, never fetched URLs or executed commands.
- Out of scope: server authorization defects and unrelated transport/mapper
  hardening remain with their existing owners; mutations retain their contract.

## Validation

Focused tests named `alias_see_also` cover model round trips, empty/missing and
REST wrong-type behavior, XML-RPC conversion, REST default field requests,
projection/rendering/escaping, and schema declarations. Run red before adding
production support, then green after. Extend `tests/functional/phases/08-bugs.sh`
for mutation/read round trips in REST/XML-RPC, public reads, include/exclude,
JSON/table, and empty arrays. Run `make functional-test` (default 5.0) before PR.
Run `make lint` and full `make test` before committing implementation; the
mandatory pre-push hook owns final candidate full local verification. CI adds
its configured platform/feature gates. Documentation prose needs human review,
not a wording-snapshot test. Revert the feature commit to roll back; no persisted
state migration is introduced.
