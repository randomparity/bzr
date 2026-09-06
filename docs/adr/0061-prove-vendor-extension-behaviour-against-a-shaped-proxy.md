# ADR 0061: Prove vendor-extension behaviour against a shaped proxy

## Status

Accepted

## Context

ADR 0052 decided that bzr establishes vendor-extension support before dispatching a
request whose vendor parameter bzr constructs, and refuses otherwise. `savedsearch` and
`sharer_id` on `Bug.search` are the first capability under that rule.

The comparison tier is supposed to hold that behaviour to account, and it cannot. The
`Server saved search` row in `tests/functional/compare/01-bug-lifecycle.sh` seeds a named
query selecting the two bugs the lifecycle just created, asks python-bugzilla for it, and
asserts the returned id set equals those two ids. Every supported image ignores the
parameter and returns the whole database:

```
$ curl -s -o /tmp/ss -w '%{http_code}\n' \
    'http://127.0.0.1:52766/rest/bug?savedsearch=nonexistent-query-xyz'
200
$ python3 -c "import json;print(len(json.load(open('/tmp/ss'))['bugs']))"
182
```

That is 182 bugs on a container that has already run a full suite. The row nevertheless
passes, and **the reason is the clean database, not the client**: at that point in a freshly
reset run the whole database *is* those two bugs, so the filtered and unfiltered sets are the
same set by construction and the oracle compares a set against itself. This inverts the usual
worry — a dirty container is the familiar confound, but here a *clean* one is what makes the
assertion vacuous, and the recent work making `setup-bugzilla.sh reset` reliable keeps it
reliably vacuous. That is why the row has looked healthy for its entire life.

Separately, with ADR 0052's refusal in place, bzr now errors on every image the suite can
run, so nothing exercises the dispatch path that ADR gates.

No supported image can be made to honour the parameter: it is a Red Hat extension, absent
from the source of all three, and Red Hat Bugzilla is not distributable as a test image. The
behaviour is therefore only provable against something the suite synthesizes — and the route
the fixture needs is already served everywhere:

```
$ for p in 52766 52531 52679; do \
    curl -s -o /tmp/ext -w "$p %{http_code} " "http://127.0.0.1:$p/rest/extensions"; \
    cat /tmp/ext; echo; done
52766 200 {"extensions":{}}
52531 200 {"extensions":{}}
52679 200 {"extensions":{}}
```

(bz50 5.0.6, bz52 5.2, bz53 5.3.3+.) So the advertisement can be a rewrite of a real
response rather than a synthesized route.

Issue #710's scope note is wrong on three counts, recorded here because the issue is where
the next reader starts: `saved-search` and its variants occur on **17 lines** (21
occurrences), not "roughly 19" sites; the stale-gaps loop is `670 679 680` (671 and 672 went
when those flags shipped); and the parity document/fixture agreement is **partly** guarded,
by `grep -Fxc "$row"` in `run_parity_report_fixture` (`container-tests.sh:1038`) — only a
document row with no fixture entry is uncaught.

## Decision

**The comparison tier proves vendor-extension behaviour against a shaped proxy that stands
in for the vendor's server, and states in the parity record that the Red-Hat-shaped arm is
a fixture rather than a real server.**

Three parts of that are the decision, not implementation detail:

1. **The fixture is a mode of the existing `tests/functional/redhat-shape-proxy.py`**,
   registered in `REWRITE_HOOKS` like every other production shape it serves. It injects a
   `RedHat` entry into the `GET /rest/extensions` response so bzr's ADR 0052 gate passes,
   and resolves `savedsearch`/`sharer_id` on `GET /rest/bug` by filtering the forwarded
   response to the ids the named query selects.

2. **The seeded query selects a strict subset of what an unfiltered search returns, and the
   row asserts both halves separately**: the unfiltered control is asserted to *contain*
   both lifecycle bugs, and the filtered call is asserted to *equal* the seeded subset.
   Each is load-bearing and each can fail alone. The control assertion carries the weight —
   without it, "the filtered call returned the seeded subset" passes on a database that only
   ever held the seeded subset, which is the defect above. A third assertion that the two
   sets differ would be **entailed by these two and could never fail**, so the row does not
   carry one; an assertion that cannot fail is what this record exists to remove, not to
   add. Containment rather than equality for the control, because the stem-matching set is
   not fixed by construction and a later row creating a stem-bearing bug would break an
   equality assertion for an unrelated reason.

3. **Only the bzr arm is routed through the proxy.** The python-bugzilla arm stays on the
   unproxied container and is asserted to return a bug the seeded query excludes, which
   turns the parity row's standing claim — python-bugzilla returns unfiltered results —
   from an unproven sentence into a tested one. That restated assertion, and the control
   guarding the control set, are **forced** by narrowing the seeded query rather than
   volunteered: once the query selects a strict subset, the old equality assertions stop
   being true and something has to replace them. They are worth having, but they are the
   cost of the fix, not a separate gain it earns.

## Consequences

- The row now proves two things, neither proven before: on a stock server bzr refuses and
  python-bugzilla silently returns unfiltered results, and on a Red-Hat-shaped server bzr's
  saved search actually filters.
- **The Red-Hat-shaped arm proves bzr's behaviour, not Red Hat's.** The proxy is a fixture
  built from the vendor's documented parameter names, not evidence that Red Hat Bugzilla
  resolves a named query the same way — the same class of limit ADR 0052 already accepted for
  detection. The parity record says so, rather than letting a green row imply a fidelity
  nobody established.
- Both failure directions the old row could not see now turn it red, though the first by a
  non-obvious route: if bzr stops sending `savedsearch` the request becomes
  `/rest/bug?limit=50&order=bug_id`, which `is_termless_bug_search` classifies as termless
  (`limit` and `order` are both ignored), so the proxy answers `code 1000` and the probe
  fails there — had it been forwarded, the unfiltered result would fail the equality anyway.
  If the proxy stops filtering, the equality fails directly.
- **The fixture filters a page the backend already limited.** `bzr bug search` sends
  `limit=50` (`DEFAULT_SEARCH_LIMIT`, `src/commands/bug/search.rs:22`) and upstream discards
  `savedsearch`, so the proxy filters the backend's own first page. Sound only while the
  comparison database stays under a page — which `setup-bugzilla.sh reset` guarantees.
- The two arms talk to two different servers within one row. Deliberate — the gap recorded is
  a stock-server gap and must be measured on one — but the row is no longer a single
  like-for-like comparison, and its assertions name which server each addresses.
- **Routing the bzr arm through the proxy also subjects it to the proxy's *ungated* hooks**,
  not just the saved-search mode. `REWRITE_HOOKS[0]` matches `/rest/bug` unconditionally and
  rewrites `component`/`version` into arrays with a `-redhat-secondary` member. That is
  benign for this row — `src/types/bug.rs:50-51` already types both as
  `Option<Vec<String>>`, and the row asserts only on `id` — but it is a real coupling: a
  future assertion on either field would see the shaped value on the proxied calls and the
  plain value on the stock ones.
- The fixture's name-to-ids mapping and the seeded `namedqueries` row encode the same set
  twice, set from the same shell values in the same block to bound the drift window.
- **The mode is read once, the fixture on every request.** `make_handler` reads
  `BZR_FUNC_REDHAT_MODE` at construction; `_saved_search_fixture()` reads its variables per
  request. A test driving the proxy must hold both in scope for the whole round trip —
  restoring the environment after starting the server disables the filter silently.
- Only `make functional-compare-all` reaches any of this. The harness self-test is what makes
  the row's controls checkable without a container, and it gains one control per assertion.

## Considered & rejected

- **Leave the row as an expected gap and document that it is unprovable.** judgment: the row
  already reports a result, and a result that cannot fail is worse than no row — it spends a
  reader's trust without earning it.
- **Obtain a real Red Hat Bugzilla image for the suite.** verified: `savedsearch` and
  `sharer_id` appear nowhere under `Bugzilla/` in the three functional images (5.0.6, 5.2,
  5.3.3), per ADR 0052's source review; Red Hat Bugzilla is not published as a distributable
  image. judgment: not available at any price the suite can pay.
- **Have the proxy rewrite the request — replace `savedsearch=<name>` with `id=<ids>` before
  forwarding — so the real server does the filtering.** judgment: more faithful, but it needs
  request-URL rewriting in `_forward`, which the hook registry does not carry; the registry is
  response-only by construction and its docstring says so. It would apply the id restriction
  before the backend pages, which the response filter cannot — that is the one thing it
  genuinely buys. It matters only on a database larger than one page, and the comparison tier
  resets its container, so it is not worth new plumbing on the forwarding path.
- **Synthesize `GET /rest/extensions` in the handler instead of rewriting the response.**
  verified: all three images answer that route `200 {"extensions":{}}` (command and output in
  Context). judgment: a synthesized route would bypass the backend for a request the backend
  answers correctly, and would not notice if that ever stopped being true.
- **Route the python-bugzilla arm through the proxy too, so both arms meet a Red-Hat-shaped
  server.** verified: the adapter runs inside a sidecar container started with
  `--network container:${bugzilla_container}` (`tests/functional/lib.sh:406`) and talks to the
  module constant `SERVER_URL = "http://127.0.0.1"` (`python-bugzilla-adapter.py:17`, used at
  `:824`); the shaped proxy binds a *host* loopback port (`lib.sh:1440`), which a container
  sharing Bugzilla's network namespace cannot reach. Routing the arm would mean re-networking
  the sidecar, not just adding a URL override. judgment: it would also delete the row's only
  measurement of the documented stock-server difference.
- **Assert the python-bugzilla arm returns exactly the unfiltered control set.** verified: the
  unproxied call returns the whole database — 182 bugs above — not the stem's two bugs.
  judgment: an equality assertion against a set the suite does not control is a flake waiting
  for the first fixture that seeds another bug; asserting it returned an id the seeded query
  excludes says the same thing and cannot drift.
- **Gate the row on a new `bzr` flag or client change so it can be proven on stock.**
  judgment: out of scope by the issue's own boundary, and it would mean weakening ADR 0052's
  refusal to make a test pass.
