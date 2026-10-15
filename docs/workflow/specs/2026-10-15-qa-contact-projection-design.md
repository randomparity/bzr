# QA-contact projection

Issue: #881. Authority: WORK:SCOPE token `q881-92a05e3f`; operator approved
scope/exclusions and the optional built-in field design on 2026-10-15.

## Problem

`bug search --fields qa_contact` fails locally: the shared registry does not
recognize the field. REST decoding discards it; XML-RPC conversion omits it.
Accepting the token alone cannot display the server value.

## Scope

Extend the existing owners: `Bug`/`BugWire` and serialization in
`src/types/bug.rs`, the built-in registry in `src/types/bug/fields.rs`,
`value_to_bug` in `src/xmlrpc/resources/bug.rs`, and shared output in
`src/output/resources/bug.rs`. Add `qa_contact: Option<String>` alongside
`assigned_to`, with canonical token `qa_contact`, table header `QA_CONTACT`,
and detail label `QA Contact`. JSON carries the unshortened string or null;
table output uses the existing assignee email-shortening convention.
The default list table columns remain unchanged. Include the field in the
shared default search request so unprojected JSON can carry the server value.
Add the field to the separate REST `BUG_VIEW_DEFAULT_FIELDS` whitelist too.
Search/list/view/local-query callers reuse the shared projection; no new owner,
facade, compatibility path, or dependency is needed. Update direct constructors,
`schemas/bug.json`, field documentation, and regression/functional tests.
Bump `src/output/mod.rs::SCHEMA_VERSION` from 3.0.8 to 3.0.9 under the
existing additive-field patch policy (ADR 0007).
Do not add other fields or change filtering/mutation semantics; those remain
separate work owned by the repository maintainer. The operator additionally
requested filing a follow-up if another field has the same verified defect.

## Success

- C1: The issue's field selection reaches the server and displays its QA contact.
- C2: REST and XML-RPC carry optional QA contact through table/detail/JSON/NDJSON
  projection; include/exclude selection uses the existing shared registry.
- C3: Tests cover populated, missing, null and empty values, from-URL search,
  and real authenticated/credentialless reads through REST and XML-RPC.
- C4: The published bug schema declares the nullable string; CLI docs show usage.

## Failure model

- Actors and deployments: local CLI operators, authenticated and anonymous
  reads against supported Bugzilla REST/XML-RPC servers; CI fixture tests.
- Invariants and assets: correct field selection and user identity, stable
  default table columns, no field-value loss or terminal-control execution.
- Accepted failure classes: a server may omit QA contact or return an empty
  value; display follows the adjacent optional assignee field behavior.
- Covered elsewhere: existing transport/auth/error handling and table escaping
  remain owned by their shared runtime and output guardrails.

## Threat model

- Boundaries widened: existing server bug-response decoding and CLI projection.
  No new network route, credential, permission, or mutation boundary.
- Actors: remote server controls returned strings; local operator selects fields.
- Controls: serde optional-string validation on REST; existing XML-RPC string
  extraction; shared table/detail terminal escaping; ordinary JSON encoding.
- Out of scope: compromised servers lying about identities and existing
  authentication policy; this read-only field does not alter their authority.

## Global Constraints

Rust 1.89.0; current-thread Tokio; no new dependencies; sibling test files;
existing release targets and default features preserved.

## Validation

- Model/registry/projection: focused-test; `qa_contact_projection` cases in
  sibling tests; red: unknown field, discarded value, absent serialized key,
  or missing schema property; green: `make test-one T=qa_contact_projection`.
- From-URL request and REST decode: focused-test; command test checks requested
  fields and rendered contact; same focused command, red is issue #881 error.
- Default REST view: focused-test; `qa_contact_projection` request/decode case
  asserts the no-selection request includes the field and preserves its value;
  same focused command, red is the missing field in the default whitelist.
- XML-RPC: focused-test; value conversion with present/absent/empty contact;
  same focused command, red is discarded returned value.
- Live contract: focused-test; extend `tests/functional/phases/08-bugs.sh`
  using an explicitly assigned QA contact and credentialless reads, with both
  API modes; green: `make functional-test` (default supported server).
- Documentation: task-test-not-applicable; prose usage example has no executable
  contract beyond the command regression and functional cases above.
- Integration: `make lint`, `make test`; mandatory hooks remain enabled.
