# Bug search role-negation and paging design

Issue: #628  
Decision: [ADR 0041](../../adr/0041-role-negation-and-zero-limit-paging.md)

## Goal

Make role-filter negation exclude exactly the bugs that the corresponding
positive substring includes, and reject the zero-limit/offset window Bugzilla
silently discards.

## Contract

The role-valued filters `assigned_to`, `creator`/`reporter`, and `qa_contact`
use Bugzilla's positive substring compatibility behavior. Their `!value` form
must therefore emit `nowordssubstr`, the inverse of `anywordssubstr`.
Exact-match fields continue to use `notequals`; native substring fields such as
whiteboard and URL continue to use `notsubstring`.

After URL, saved-query, and CLI overrides have been resolved, a search with an
effective `limit` of zero and an effective nonzero `offset` fails locally with
`BzrError::InputValidation`. The error names both flags and explains that zero
means unbounded and cannot be combined with a nonzero offset. `offset=0` is
equivalent to no offset and remains valid. Count mode remains valid because it
already clears offsets before issuing its `limit=0` request.

## Components and data flow

`NegationOp` gains a role-complement variant whose wire value is
`nowordssubstr`. The three role rows in `FIELD_MAPPINGS` select it; the existing
request builder remains the single encoder of boolean-chart triples.

The shared paging module gains a validator over resolved `SearchParams`.
Callers invoke it immediately before paging/search execution and before making
a network request. `bug list` and `bug my` validate the base params they build;
the shared search execution path covers `bug search` and `query run` after
their URL/saved-query/CLI resolution. This keeps validation at the smallest
shared seams without changing count-mode rewriting.

CLI long help and per-flag help state that assignee, creator, and QA-contact
values are substring role matches and that `!` excludes those substring
matches. Paging help states that a zero limit cannot be paired with a nonzero
offset. The CLI reference mirrors these rules.

## Errors and edge cases

- `limit=Some(0), offset=Some(N)` rejects when `N > 0`.
- `limit=Some(0), offset=None|Some(0)` remains valid and unbounded.
- A positive limit with any representable offset remains valid; existing
  overflow checks still apply.
- URL and saved-query offsets are validated only after normalization removes
  duplicate raw `offset` parameters.
- Mixed positive and negated filters retain their existing AND/OR composition;
  only each role negation's operator changes.

## Verification

Focused request tests pin `nowordssubstr` for assigned-to, creator, and
QA-contact while retaining `notequals` for exact fields. Shared paging tests
prove the invalid pair fails before transport and the three valid boundary
cases remain accepted. Command tests cover all four consumers, including a
saved-query or URL-derived effective pair.

The functional suite creates a role login whose address contains a stable
substring and proves the positive assignee form includes its bug while the
negated form excludes it. It also proves zero-limit/nonzero-offset rejection
and successful ordinary paging for every shared consumer. Both defect tests
must be observed failing against pre-fix production code before implementation.

Required gates are `make lint`, `make test`, and
`make functional-test-all` across Bugzilla 5.0, 5.2, and 5.3.

## Threat model

The local operator controls filter and paging values crossing the CLI/config
boundary into HTTP query parameters. Existing typed `u32` parsing bounds the
numbers, reqwest owns URL encoding, and the new validator fails before
transport without echoing server data or credentials. The change adds no
network endpoint, authorization rule, secret handling, or permission. A
malicious Bugzilla server and server-side search correctness beyond the two
documented behaviors remain outside scope.
