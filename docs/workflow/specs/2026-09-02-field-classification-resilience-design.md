# Field and classification read resilience

## Scope and outcome

Issue #629 requires three related read-path corrections: accept omitted `values` for non-select
bug fields, use one model for `/rest/field/bug`, and make disabled classification listing useful
to unprivileged users. It also requires percent-encoding field-name path segments and live
functional coverage. [ADR 0042](../../adr/0042-field-classification-list-resilience.md) records
the ownership and presentation decisions.

`Classification.sort_key` and the schema 3.0 cascade are excluded. Issue #629 explicitly permits
deferral, and ADR 0028 deliberately excluded that unrelated sort key. Issue #628, unrelated
worktrees, and the campaign-owned ADR index are also outside this branch.

## Approach

The chosen approach moves the richer `FieldDef` shape from the server resource into the field
resource as the sole internal `FieldDefinition`, defaulting optional endpoint members there.
`get_field_values` and server-capability custom-field assembly deserialize that same type.
This removes the root duplication with a smaller change than introducing a new public type.

At the classification command boundary, match only `BzrError::Api { code: 900, .. }` from the
whole list enumeration. This preserves the client's existing public result type and avoids hiding
other API failures. ADR 0042 records this command-specific exception to ADR 0015: code 900 means
the optional feature is disabled, rather than an unrelated request failure. For error 900,
raw/table mode prints the existing note on stdout without an empty-table sequel; JSON and NDJSON
write an empty collection on stdout and the note on stderr. A successfully fetched lone
`Unclassified` row preserves the existing row output and stderr note.

Rejected approaches are duplicating the serde default, collapsing error 900 into an
indistinguishable client-side empty vector, and emitting human prose into structured stdout.

## Components and data flow

1. `client::resources::field` owns `FieldDefinition` and `FieldBugResponse`. The definition
   contains `name`, lenient numeric `field_type`, `is_custom`, and defaulted `values`. A
   crate-resource-visible `all_bug_fields` helper supplies server capabilities; field lookup
   resolves aliases, encodes the path segment with the existing `encode_path`, and selects the
   first definition.
2. `client::resources::server` removes its endpoint duplicate and maps the shared definitions to
   `CustomFieldSummary` exactly as today.
3. `commands::classification` converts error 900 into a `write_disabled` path whose destination
   depends on output format. A lone `Unclassified` result and successful multi-row lists retain
   their existing row output; every unrelated error retains current behavior.
4. Functional phase 05 exercises text/date/int fields and credentialless classification listing
   against each supported stock container.

Because the established `encode_path` helper encodes underscore, alias lookup changes the literal
request from `bug_status` to `bug%5Fstatus`. Existing wiremock call-site fixtures and the functional
Red Hat response-shape proxy must accept that encoded spelling. Those are direct verification
dependencies of the required path change, not changes to their command behavior.

The unified definition also keeps `name` required, as the all-fields consumer already did after
#626. Earlier single-field fixtures omitted `name` only because their narrower duplicate never
read it; every existing successful `/rest/field/bug/<name>` fixture must add its real resolved
field name while retaining values and assertions. This includes `classification` fixtures as well
as alias fixtures. Defaulting identity to preserve incomplete mocks would weaken the production
contract.

## Error and output contract

- An existing field whose response omits `values` returns an empty vector. Table output is exactly
  `No values for field '<requested-name>'.` followed by a newline; JSON-family output remains an
  empty collection.
- An empty `fields` array remains `BzrError::NotFound`.
- API code 900 anywhere in classification enumeration is the only error that degrades. Raw/table
  stdout is exactly the existing disabled note plus a newline and stderr is empty. JSON-family
  stdout is an empty valid collection and stderr contains that note.
- A successfully fetched lone `Unclassified` row remains in output and the disabled note remains
  on stderr.
- Other classification errors remain errors with their existing exit codes.
- The resolved field name is one percent-encoded path segment; aliases are resolved before
  encoding. An empty name and exact `.` or `..` names are rejected before URL construction because
  the empty value collapses to the bulk route and the URL parser normalizes encoded dot forms as
  navigation segments.

## Security and trust boundaries

The design adds no entry point or permission. It narrows two existing boundaries:

- A local operator controls the field-name argument, which crosses into URL construction. The
  existing `client::encode_path` is the control; it encodes the complete resolved value as one
  segment, while empty, `.`, and `..` values are rejected before route collapse or URL-parser
  normalization. Tests use slash, percent, question-mark, and space characters and prove invalid
  empty/dot-only inputs send no request.
- A configured Bugzilla server controls JSON response shape and API error codes. Serde defaults
  only the members upstream documents as omitted, while required field identity remains required.
  The degradation match is bounded to numeric code 900 and leaks only the existing generic note.
- Credentials continue through the shared connection/auth pipeline. The new functional
  unprivileged arm uses the already configured credentialless `public` server and never exposes a
  secret.

Out of scope are malicious server payload-size limits and TLS/auth changes; existing transport
controls own them. The negative classification sort-key case remains the accepted ADR 0028
exclusion described above.

## Verification

Tests must be observed failing against the pre-fix implementation before production edits.

- Field resource tests cover omitted `values`, encoded names, unchanged alias resolution, and
  not-found behavior. A recorded all-fields response containing select and non-select rows proves
  the one shared model serves server capabilities. Existing command, integration, and proxy
  fixtures that observe alias requests use the encoded path emitted by the shared helper and carry
  the required field `name` that the consolidated response model reads.
- Classification command tests cover code 900 in table, JSON, and NDJSON modes, exact stream
  placement, preserved lone-`Unclassified` compatibility, and unrelated-error propagation. Empty
  NDJSON output for error 900 is zero records.
- Functional phase 05 covers `short_desc`, a date field, and an integer field, plus credentialless
  `classification list` on default `useclassification=0`, asserting exit 0 and exact raw stdout.
- `make lint`, `make test`, and `make functional-test-all` must pass.

## Durable workflow context

- Branch: `feat/field-classification-resilience-629`
- Base branch: `main` at `f5314bcfa4f9bb8fe0349908ef206ac2d8d5e547`
- Host: arm64 macOS; targets: seven declared release targets; relationship: different.
- Guardrails: focused `make test-one T=<substring>`, `make lint`, `make test`, and
  `make functional-test-all`. The ADR index is not individually hard-gated and remains
  campaign-owned.
