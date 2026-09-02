# ADR 0041: Complement role filters and reject zero-limit offsets

## Status

Accepted

## Context

Bugzilla rewrites positive `assigned_to`, `reporter`, and `qa_contact` searches to
boolean charts using `anywordssubstr`. bzr currently emits `notequals` for the
same fields when their value begins with `!`. A substring such as `alice`
therefore matches `alice@example.com` positively but does not exclude it when
negated.

Bugzilla also removes both `limit` and `offset` when `limit=0`. Sending
`limit=0&offset=N` silently returns results from the beginning instead of the
requested offset.

## Decision

Role-filter negation is the logical complement of the role filter's positive
behavior. bzr emits Bugzilla's `nowordssubstr` boolean-chart operator for
`assigned_to`, `reporter`, and `qa_contact`. Other fields retain their current
exact or substring negation operators.

bzr rejects an effective zero limit combined with a nonzero offset before any
request. This validation applies at the shared paging boundary so `bug list`,
`bug search`, `bug my`, and `query run` behave consistently, including saved
queries and imported URLs. `limit=0` without an offset remains the unbounded
request used by normal searches and `--count`.

## Consequences

- `--assignee alice` and `--assignee '!alice'` become complementary substring
  filters; creator and QA-contact filters follow the same rule.
- Role negation intentionally differs from exact-match non-role filters, so CLI
  help and the reference documentation state the distinction.
- An invalid zero-limit window fails with input-validation exit 7 instead of
  returning a silently wrong result set.
- Validation must inspect the resolved `SearchParams`, not only raw CLI flags,
  because saved queries and imported URLs may supply either value.

## Considered & rejected

- **Keep `notequals` for role filters.** verified: issue #628 records Bugzilla's
  `anywordssubstr` positive-role rewrite and the resulting `alice` versus
  `alice@example.com` asymmetry; the operator chose complement semantics in
  issue comment 5516940736.
- **Emulate offset after an unbounded request client-side.** judgment: this
  downloads and buffers rows the caller explicitly asked to skip when the
  permitted alternative is a local, actionable rejection.
- **Validate only clap's `--limit` and `--offset` pair.** verified: current
  `query run` and `bug search --from-url` resolve paging values from saved or
  URL-derived `SearchParams`, so raw CLI validation would leave those paths
  inconsistent.
