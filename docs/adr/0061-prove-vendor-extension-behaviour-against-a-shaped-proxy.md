# ADR 0061: Prove vendor-extension behaviour against a shaped proxy

## Status

Accepted

## Context

ADR 0052 decided that bzr establishes vendor-extension support before dispatching a
request whose vendor parameter bzr constructs, and refuses otherwise. `savedsearch` and
`sharer_id` on `Bug.search` are the first capability under that rule.

The comparison tier is supposed to hold that behaviour to account, and it cannot. The
`Server saved search` row in `tests/functional/compare/01-bug-lifecycle.sh` seeds a
server-side named query selecting the two bugs the lifecycle just created, asks
python-bugzilla for it, and asserts the returned id set equals those two ids. Every
supported image ignores the parameter and returns the whole database, so the assertion
passes only because "the whole database" and "the seeded set" are the same two bugs at
that point in the run. Against a container that has already run a full suite, the same
call returns 182 bugs:

```
$ curl -s -o /tmp/ss -w '%{http_code}\n' \
    'http://127.0.0.1:52766/rest/bug?savedsearch=nonexistent-query-xyz'
200
$ python3 -c "import json;print(len(json.load(open('/tmp/ss'))['bugs']))"
182
```

So the row's oracle compares a filtered set against itself. A server that honours the
parameter and a server that discards it produce the same PASS, and no arrangement of the
existing assertion can separate them — the row has never been able to show what it claims
to show. Separately, with ADR 0052's refusal in place, bzr now errors on every image the
suite can run, so nothing anywhere exercises the dispatch path the ADR gates.

No supported image can be made to honour the parameter: it is a Red Hat extension, absent
from the source of all three, and Red Hat Bugzilla is not distributable as a test image.
The behaviour is therefore only provable against something the suite synthesizes.

## Decision

**The comparison tier proves vendor-extension behaviour against a shaped proxy that
stands in for the vendor's server, and states in the parity record that the Red-Hat-shaped
arm is a fixture rather than a real server.**

Three parts of that are the decision, not implementation detail:

1. **The fixture is a mode of the existing `tests/functional/redhat-shape-proxy.py`**,
   registered in `REWRITE_HOOKS` like every other production shape it serves. The proxy
   advertises `RedHat` at `GET /rest/extensions` — verified to return `200
   {"extensions":{}}` on all three images, so a response rewrite suffices and no
   synthesized route is needed — and resolves `savedsearch`/`sharer_id` on `GET /rest/bug`
   by filtering the forwarded response to the ids the named query selects.

2. **The seeded query selects a strict subset of what an unfiltered search returns, and
   the row asserts the filtered set differs from an unfiltered control captured in the
   same run.** Equality against a fixed expected set is what failed here; a set that
   happens to equal its own control is a non-discriminating oracle whether or not every
   assertion in it bites. The two properties — that an assertion fails when broken, and
   that the sets it compares can differ — are asserted separately, because passing the
   first says nothing about the second.

3. **Only the bzr arm is routed through the proxy.** The python-bugzilla arm stays on the
   unproxied container and is asserted to return a bug the seeded query excludes, which
   turns the parity row's standing claim — python-bugzilla returns unfiltered results —
   from an unproven sentence into a tested one.

## Consequences

- The row now proves two distinct things: on a stock server bzr refuses and
  python-bugzilla silently returns unfiltered results, and on a Red-Hat-shaped server
  bzr's saved search actually filters. Neither was proven before.
- **The Red-Hat-shaped arm proves bzr's behaviour, not Red Hat's.** The proxy is a fixture
  built from the vendor's documented parameter names; it is not evidence that Red Hat
  Bugzilla resolves a named query the same way. This is the same class of limit ADR 0052
  already accepted for detection — advertisement is a proxy for a patched `Bug.search`,
  not proof of one — and the parity record says so rather than letting a green row imply
  a fidelity nobody established.
- A regression in which bzr stops sending `savedsearch` turns the row red, because the
  proxy filters only when the parameter is present. A regression in which the proxy stops
  filtering also turns it red, because the filtered set then equals the control. Those are
  the two failure directions the old row could not see.
- The two arms now talk to two different servers within one row. That is deliberate — the
  gap being recorded is a stock-server gap and has to be measured on a stock server — but
  it means the row is no longer a single like-for-like comparison, and its assertions name
  which server each one addresses.
- The fixture's name-to-ids mapping and the seeded `namedqueries` row encode the same set
  in two places. They are set from the same shell values in the same block to keep the
  drift window to that block.
- Nothing in `make lint` or `make test` reaches any of this; only
  `make functional-compare-all` does. The harness self-test in
  `tests/functional/pybz/container-tests.sh` is what makes the row's controls checkable
  without a container, and it gains controls for both new failure directions.

## Considered & rejected

- **Leave the row as an expected gap and document that it is unprovable.** judgment:
  the row already reports a result, and a result that cannot fail is worse than no row —
  it spends a reader's trust without earning it. The suite would keep reporting PASS on an
  oracle comparing a set against itself.
- **Obtain a real Red Hat Bugzilla image for the suite.** verified: `savedsearch` and
  `sharer_id` appear nowhere under `Bugzilla/` in the three functional images (5.0.6, 5.2,
  5.3.3), per ADR 0052's source review; Red Hat Bugzilla is not published as a
  distributable image. judgment: not available at any price the suite can pay.
- **Have the proxy rewrite the request — replace `savedsearch=<name>` with `id=<ids>`
  before forwarding — so the real server does the filtering.** judgment: more faithful,
  but it needs request-URL rewriting in `_forward`, which the hook registry does not
  carry; the registry is response-only by construction and its docstring says so. The
  fidelity gained is over a fixture either way, so it buys nothing the response filter
  does not, at the cost of new plumbing on the forwarding path.
- **Synthesize `GET /rest/extensions` in the handler instead of rewriting the response.**
  verified: all three images return `200 {"extensions":{}}` for that route (command and
  output in Context), so there is a real response to shape. judgment: a synthesized route
  would bypass the backend for a request the backend answers correctly, and would not
  notice if that ever stopped being true.
- **Route the python-bugzilla arm through the proxy too, so both arms meet a
  Red-Hat-shaped server.** verified: `saved_search` is a `LEGACY_OPERATIONS` member in
  `tests/functional/compare/python-bugzilla-adapter.py`, so it refuses a `transport`
  override and its client is built against the hardcoded `SERVER_URL`; routing it would
  add a per-operation URL override to the adapter. judgment: it would also delete the
  row's only measurement of the documented stock-server difference, which is the half of
  the row that records why bzr and python-bugzilla deliberately diverge.
- **Assert the python-bugzilla arm returns exactly the unfiltered control set.**
  verified: the unproxied call returns the whole database — 182 bugs on a container that
  has run a full suite — not the lifecycle stem's two bugs. judgment: an equality
  assertion against a set the suite does not control is a flake waiting for the first
  fixture that seeds another bug; asserting it returned an id the seeded query excludes
  says the same thing and cannot drift.
- **Gate the row on a new `bzr` flag or client change so it can be proven on stock.**
  judgment: out of scope by the issue's own boundary, and it would mean weakening ADR
  0052's refusal to make a test pass.
