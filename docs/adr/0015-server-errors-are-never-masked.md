# 0015 — A server error is never masked by an empty result

- Status: Accepted
- Date: 2026-08-04
- Issue: #504
- Related: [0014](0014-structured-error-detail-keys.md)

## Context

`bzr bug view <id>` intermittently reported `error: bug not found: <id>` (exit 2)
for a bug in an access-restricted product that the caller could see. Repeating
the command eventually succeeded.

`BzrError::NotFound` for a bug is reachable from exactly one condition: the
server returned HTTP 200, the body parsed as JSON, and the `bugs` array was
**empty** (`client/resources/bug.rs`). Every genuine 4xx/5xx and every Bugzilla
fault takes a different path with a different message and exit code. So the
message meant bzr had been told why and discarded it. Two places discard it:

1. **`check_bugzilla_200_error` downgrades an error payload to a `warn!` whenever
   the response contains a known data key.** The predicate was
   `map.contains_key(k)` — presence, not content. A body such as
   `{"error":true,"code":102,"message":"You are not authorized to access bug
   #216593","bugs":[]}` satisfied it, so the error was logged and swallowed,
   leaving an empty list that became `NotFound`.

   The leniency itself is deliberate and load-bearing: some deployments (the IBM
   LTC Bugzilla that motivated it) return an extension's error *alongside* real
   data, and failing there would break working invocations. The defect is that
   the predicate could not tell "data alongside an error" from "an error and
   nothing else".

2. **The 100500 search-endpoint retry dropped the original error.** `get_bug_rest`
   retries `/rest/bug?id=<id>` when the direct lookup returns Bugzilla's internal
   error 100500, because some extensions only crash on the direct path. But
   Bugzilla's search path filters bugs the caller cannot see into an empty 200
   result instead of faulting, so when the retry legitimately returned nothing the
   original 100500 was lost and `NotFound` was reported in its place.

Both triggers are server-load dependent, which is what made the failure
intermittent and the report hard to act on: the user is told the bug does not
exist, which is both false and not actionable.

## Decision

**A server error is surfaced whenever it is the only thing the server told us.
bzr does not re-implement Bugzilla's disclosure policy.**

Concretely:

1. **`has_data_fields` requires a data key to be non-empty, not merely present.**
   A key counts as data when its value is a non-empty array, a non-empty object,
   or a non-null scalar. `[]`, `{}`, and `null` do not count. When no data key
   carries content, an `error: true` payload is fatal (`BzrError::Api`) exactly as
   it is when the key is absent entirely.

   This keeps the IBM LTC accommodation intact — an error beside a populated
   `bugs` array still yields the data and a warning — while closing the case where
   the "data" was an empty placeholder. The comment envelope
   `{"bugs":{"42":{"comments":[]}}}` (bug acknowledged, no comments) stays a
   legitimate empty result: the top-level `bugs` map is non-empty.

2. **The 100500 search fallback preserves the original error.** When the search
   retry returns no rows, `get_bug_via_search` surfaces the original
   `Api{code:100500}` annotated with the fact that the fallback found nothing,
   rather than `NotFound`. `NotFound` is reserved for the direct path returning an
   empty result with no error payload — the one case where "no such bug" is what
   the server actually said.

**Consequence accepted deliberately: bzr becomes an existence oracle to the
extent the server already is.** A caller who receives `102 You are not authorized`
learns the bug exists. That disclosure is Bugzilla's to make — it chooses per
deployment whether to answer 102 (exists, restricted) or 101 (indistinguishable
from absent), and a server that wants ambiguity already returns the ambiguous
answer. bzr second-guessing that choice is what produced #504: it converted a
server's clear answer into a false one, and gave the operator nothing to act on.
Relaying faithfully cannot leak more than the server chose to emit.

## Consequences

- **A user-visible exit-code change.** A restricted bug that previously exited 2
  (`not-found`) now exits 4 (`api`) with the server's code and message. Scripts
  branching on exit 2 to mean "restricted or absent" must branch on 2-or-4.
  CHANGELOG entry required; no schema change (`BzrError::Api` already carries
  `api_code` under ADR-0014).
- **Cross-cutting.** `check_bugzilla_200_error` is on every resource read path —
  bugs, comments, attachments, products, groups, users, fields, classifications,
  `ids`. Tightening it can turn a previously-silent error into a failure for any
  of them. That is the intended correction, but it is the change's main regression
  surface, and it is why the non-empty rule is scoped to content rather than
  replaced with "any error is fatal".
- **The functional suite could not have caught this.** `08c-bugs-create-fields.sh`
  test 146c asserted only `assert_failure` (exit `!= 0`), which passes for exit 2,
  4, 5, and 9 alike. It is tightened to assert the exit code and the message, and
  the harness gains a second credentialed identity so the
  authenticated-group-member direction — the reporter's actual scenario — is
  testable at all.

## Alternatives considered

- **Preserve the ambiguity, improve the hint.** Keep exit 2 but explain that the
  bug may exist and be restricted. Rejected: it keeps bzr in the business of
  deciding disclosure, still discards the server's `code`/`message`, and leaves
  the operator guessing which of the two it was — the exact failure in #504.
- **Faithful relay only when authenticated; ambiguous when anonymous.** Rejected
  as a distinction bzr cannot draw correctly: "authenticated" is a client-side
  belief about a credential the server may have rejected, ignored, or scoped
  differently, so the branch would misfire precisely in the degraded cases that
  matter.
- **Treat any `error: true` as fatal regardless of data.** The literal reading of
  "never mask". Rejected: it reverses the IBM LTC accommodation (`response.rs`,
  and the test at `response_tests.rs`) and would break deployments where an
  extension warns on every successful response. An error accompanied by the data
  the caller asked for is informational, not masked — the data *is* the answer.
- **Retry the direct lookup instead of falling back to search on 100500.**
  Rejected as orthogonal: it addresses the crash, not the error-loss, and the
  existing search fallback is there because some extensions crash deterministically
  on the direct path where a retry would never succeed.
