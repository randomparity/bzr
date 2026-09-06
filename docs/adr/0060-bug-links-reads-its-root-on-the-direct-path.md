# 0060 — `bug links` reads its root on the direct path

- Status: Accepted
- Date: 2026-09-06
- Issue: #719
- Related: [0015](0015-server-errors-are-never-masked.md)

## Context

`bzr bug links <id>` reported a group-restricted bug as `bug not found: <id>`
(exit 2) to a caller who could read the same bug with `bug view` and
`bug history` in the same session, holding the same key.

The two arms of `get_bug_links_nodes` read **different endpoints**, and only one
of them can fault:

- `get_bug_links_nodes_rest` built `self.url("bug")` with each id as a `?id=<n>`
  query parameter — Bugzilla's **search** endpoint. Search omits rows the caller
  cannot see and answers `200` with a shorter list. The method's own doc comment
  stated the consequence: inaccessible and nonexistent ids are indistinguishable
  in the reply.
- The `ApiMode::XmlRpc` arm called `get_bug` per id — the **direct** path —
  which faults, and was already matched on `BzrError::NotFound`.

Measured against a live fixture as one insider actor, with `bzr` out of the loop
(issue #719, owner comment):

| Request | no auth | `X-BUGZILLA-API-KEY` | `Bugzilla_api_key` param |
|---|---|---|---|
| `/rest/bug/<id>` — direct, what `bug view` reads | `401` code 102 | `401` code 102 | `200`, the bug |
| `/rest/bug?id=<id>` — search, what `bug links` read | `200` `{"bugs":[]}` | `200` `{"bugs":[]}` | `200`, the bug |

Row 2 is the defect. An unauthenticated or header-authenticated search for a
restricted bug returns `200` with an empty list, so nothing in the response can
trigger the alternate-auth retry that repairs row 1 — the links path was the one
read in `bzr` with no error path of its own. The caller's root-missing branch
then converted the omission into `BzrError::NotFound`.

ADR 0015 already settled the governing rule for `get_bug`: `NotFound` is
reserved for **the direct path returning an empty result with no error
payload** — the one case where "no such bug" is what the server actually said.
That rule was never carried to the links root read.

## Decision

**`bug links` reads its root id on Bugzilla's direct endpoint. Related ids keep
the batched search read.**

`get_bug_links_root_node` issues `rest/bug/<id>` with the same isolated
`LINKS_INCLUDE_FIELDS` set and surfaces whatever the server says. The root's
absence can now be an authorization error rather than a factual one, and the
401 it draws is visible to the alternate-auth retry exactly as `bug view`'s is.

Two properties are deliberate:

- **Only the root moves.** An omission is fatal for the root and skippable for a
  related bug, which is the distinction the batched read could not express and
  the caller always had to make. Related ids stay batched in
  `LINKS_ID_CHUNK`-sized search requests, and an id the caller cannot see is
  still silently omitted from the graph.
- **The root read is not `get_bug`.** `BugLinksNode::from_bug` can only express
  the three core relations, so routing the root through `get_bug` would have
  dropped `duplicates`, `regressed_by`, and `regressions` — the BMO relations —
  from the node that seeds traversal. The root read deserializes
  `BugLinksResponse` directly and keeps all six.

XML-RPC already read the faultable path; its behaviour is unchanged and is the
reference oracle the REST arm is asserted against.

## Consequences

- **Two user-visible exit-code transitions on the REST/Hybrid root read.** Both
  follow from the direct path faulting where search did not, and both make
  `bug links` agree with `bug view` on the same id:

  1. **A restricted bug the caller cannot see**: exit `2` (`not-found`,
     `bug not found: <id>`) → exit `4` (`api`) with the server's code and
     message. This is the reported defect.
  2. **A genuinely absent bug**: exit `2` → exit `4` (`api`, code 101,
     `Bug #<id> does not exist.`). Not the reported defect, but an unavoidable
     consequence of the first: a stock Bugzilla answers `rest/bug/<absent>` with
     an error payload rather than an empty `200`. `bug view` on an absent id
     already exits 4 with code 101, so this removes a divergence rather than
     creating one.

  ADR 0015 accepted this direction for `bug view` in the same words: scripts
  branching on exit 2 to mean "restricted or absent" must branch on 2-or-4.
  Exit 2 remains reachable for `bug links` only where the direct read returns an
  empty result with no error payload — the residual case ADR 0015 reserves.

- **A member whose key travels in a header now succeeds where the command
  failed.** That configuration drew `200`-with-no-rows from search, which no
  retry could see; it now draws a `401` the alternate-auth retry repairs.

- **One extra request per invocation is not added.** The root was already a
  request of its own — a one-id search — and is now a one-id direct read.

- **The BMO relations are load-bearing in the root read.** A future refactor
  routing the root through `get_bug` would silently truncate the root's
  adjacency to three relations; `get_bug_links_root_node_reads_direct_path_with_isolated_fields`
  pins all six.

- **`bug adjacency` is unaffected.** It does not share this code path. After
  this change `get_bug_links_nodes` has exactly one caller — the related-id
  batch in `commands/bug/links.rs` — and `get_bug_links_root_node` has exactly
  one, the root read in the same file. Neither has a caller anywhere else.

- **The 100500 search fallback travels with the root read.** The direct
  endpoint is the one some extensions hook and crash on, which is why
  `get_bug_rest` retries through search at all. `get_bug_links_root_node_rest`
  carries the same retry, and the same ADR 0015 rule that an empty retry
  re-surfaces the original error rather than becoming `NotFound`. Without it
  this change would have taken `bug links` from exit 0 to exit 4 on exactly the
  deployments ADR 0015 exists to keep working — a third exit-code transition,
  and an unintended one. It does **not** carry Hybrid's residual-100500
  XML-RPC step, which the links path never had.

- **CHANGELOG entry required**, as for ADR 0015. Release notes are generated
  from commit subjects and bodies are discarded, so the transition has to be
  named in a subject line to reach a reader of the notes; a subject describing
  only the mechanism would leave the contract change invisible.

## Alternatives considered

- **Re-read the root on the direct path only when the search omits it.** The
  issue's second suggestion. Rejected: it keeps a read that is wrong by
  construction and layers a repair on top, costs a second round trip in exactly
  the degraded case, and still leaves the root's normal read unable to fault.
  Reading the root directly is one request either way.
- **Route the root through `get_bug`.** Rejected: `BugLinksNode::from_bug`
  drops the three BMO relations, so the root would seed traversal with an
  incomplete edge set on Red Hat and Mozilla deployments — a silent graph
  regression in exchange for reusing a function whose field handling does not
  fit.
- **Wait for #713.** #713's auth-probe fix would incidentally close the reported
  symptom, since the query-parameter column already returns the bug. Rejected:
  the links arm would still be the only read in `bzr` with no error path of its
  own, and would fail the same way against any deployment or account where a bug
  is visible to some callers and not others.
- **Keep exit 2 and improve the message.** Rejected for the reason ADR 0015
  gives: it keeps `bzr` deciding disclosure, discards the server's `code` and
  `message`, and leaves the operator unable to tell a permissions outcome from a
  deleted id — the exact complaint in #719.
