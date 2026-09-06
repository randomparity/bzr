# Design: prove the comparison harness's server-side saved search filters

Issue: [#710](https://github.com/randomparity/bzr/issues/710)
Decision record: [ADR 0061](../../adr/0061-prove-vendor-extension-behaviour-against-a-shaped-proxy.md)
Governing client decision (read, not modified): [ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md)

## Problem

`tests/functional/compare/01-bug-lifecycle.sh` has a `saved-search` row that reports a
result it cannot establish.

The row seeds a server-side named query selecting the two bugs the lifecycle just created
(`bug_id=<bzr>,<pybz>&bug_id_type=anyexact`), asks python-bugzilla to run it, and asserts
the returned id set equals `[<bzr>,<pybz>]`. Every supported image discards `savedsearch`
and returns the whole database. The assertion passes because, at that point in a freshly
reset compare run, the whole database *is* those two bugs.

Verified — the same call against a container that has already run a full suite:

```
$ curl -s -o /tmp/ss -w '%{http_code}\n' \
    'http://127.0.0.1:52766/rest/bug?savedsearch=nonexistent-query-xyz'
200
$ python3 -c "import json;print(len(json.load(open('/tmp/ss'))['bugs']))"
182
```

So the row compares a filtered set against itself. Two consequences:

1. A server honouring the parameter and a server discarding it produce the same PASS.
2. Since ADR 0052 shipped, bzr refuses on every image the suite can run, so nothing
   exercises the dispatch path that ADR gates.

## Goal

Make the row able to fail for the right reason, in both directions:

- if bzr stops sending `savedsearch`, the row goes red;
- if the server stops honouring it, the row goes red;
- and the sets the row compares must actually be able to differ on the seeded data.

The third is a separate property from the first two. An assertion that bites when broken
can still be comparing two identical sets, which is the defect being fixed here.

## Non-goals

- No change to `bzr` itself. ADR 0052's detection and refusal ship separately (#670).
- No change to `docs/adr/README.md`; the index row for ADR 0061 is reported as pending.
- No new `bzr` flag, and no weakening of ADR 0052's refusal to make a test pass.

## Approach

Three changes, described below and planned in
[the implementation plan](../plans/2026-09-06-prove-server-saved-search-filter.md).

### 1. A `saved-search` mode on the shaped proxy

`tests/functional/redhat-shape-proxy.py` gains one mode, selected the way every other mode
is (`BZR_FUNC_REDHAT_MODE=saved-search`) and registered in `REWRITE_HOOKS` as one
`(matcher, transformer)` pair. It shapes two routes:

- `GET /rest/extensions` — inject a `RedHat` entry into the forwarded response, so bzr's
  ADR 0052 capability gate passes. Verified: all three images return `200
  {"extensions":{}}` for this route, so a response rewrite is enough. The injected shape
  is `{"RedHat": {"version": "1.0"}}`, which matches `ExtensionInfo { version:
  Option<String> }` in `src/types/server_info.rs`.
- `GET /rest/bug` carrying `savedsearch` — filter the forwarded response's `bugs` array to
  the ids the named query selects.

The fixture's mapping comes from the environment the harness sets when it starts the
proxy:

| Variable | Meaning |
|---|---|
| `BZR_FUNC_SAVED_SEARCH_NAME` | the named query this fixture resolves |
| `BZR_FUNC_SAVED_SEARCH_IDS` | comma-separated bug ids that query selects |
| `BZR_FUNC_SAVED_SEARCH_SHARER` | the owning user id `sharer_id` must match |

Resolution rules, which are the whole of the fixture's behaviour:

- `savedsearch` absent → no filtering, response forwarded unchanged. This is what makes
  the row fail if bzr stops sending the parameter.
- `savedsearch` present but not equal to `BZR_FUNC_SAVED_SEARCH_NAME` → empty `bugs`
  array (the server resolved no such query).
- `savedsearch` matches, `sharer_id` absent or equal to `BZR_FUNC_SAVED_SEARCH_SHARER` →
  `bugs` filtered to `BZR_FUNC_SAVED_SEARCH_IDS`.
- `savedsearch` matches, `sharer_id` present and different → empty `bugs` array (the
  query is not shared with that user).

The mode is inert unless enabled, like every other mode, so no existing row's traffic
changes.

### 2. A discriminating assertion in the lifecycle row

The row keeps its identity (`compare/01-bug-lifecycle/saved-search`) and stays an expected
gap, because the gap it records — the stock-server difference between bzr and
python-bugzilla — is real and still there. What changes is that all three of its
assertions become capable of failing.

The seeded named query changes from "both lifecycle bugs" to **the bzr bug only**, so the
query selects a strict subset of what an unfiltered search returns.

The row then establishes, in order:

1. **A controlled pair against the Red-Hat-shaped server.** With the proxy running in
   `saved-search` mode, two bzr calls go through it, differing in exactly one thing — the
   `--saved-search` flag:
   - the **unfiltered control**, `bzr bug list --summary "$LIFECYCLE_STEM"`, returns both
     lifecycle bugs (the stem is a substring of both `"$LIFECYCLE_STEM [bzr]"` and
     `"$LIFECYCLE_STEM [pybz]"`). A term is required because the proxy answers a termless
     `/rest/bug` with Bugzilla's own `code 1000` error, mirroring the server;
   - the **filtered** call, `bzr bug search --saved-search "$LIFECYCLE_SAVED_SEARCH"`,
     returns exactly the seeded subset, **and that set differs from the control**.

   Same server, same path, one flag apart, so the inequality isolates the parameter. It is
   the assertion issue #710 asks for: a server that ignores the parameter returns the
   control for both calls and fails here.
2. **Stock server, both clients** — bzr refuses with
   `"type":"unsupported_server_capability"` and exit 15 (unchanged, ADR 0052); and
   python-bugzilla's result **contains the pybz bug id, which the seeded query excludes**.
   That is the row's proof that python-bugzilla ignores the parameter — the claim the
   parity record has been making without evidence.

Ordering is load-bearing. `expect_gap` converts the row's recorded outcome, so the stock
refusal must be the last bzr probe in the row: every assertion above sits in the row's
precondition chain, where a failure produces an outright FAIL that never reaches
`lifecycle_expect_gap 670`. That is the same shape the eight existing
`run_gap_ineligible_control` entries already assert for this row.

Assertion 2's python-bugzilla check uses containment rather than equality against the
control, because the unproxied call returns the whole database rather than the stem's two
bugs — 182 on the container probed above. Containment says the same thing and cannot drift
when a later fixture seeds another bug.

Only the bzr arm is proxied; ADR 0061 records why.

Two small helpers are added beside `lifecycle_ids_are`, which is exact-equality only:
`lifecycle_ids_differ <a> <b>` (the two id sets are not equal) and
`lifecycle_ids_contain <file> <json-id-array>` (every listed id is present).

### 3. The harness self-test keeps step, and gains two controls

`tests/functional/pybz/container-tests.sh` sources the real phase script against stubbed
clients, so it is where "the test bites" is proven deterministically without a container.
It gains stubs for the proxy lifecycle and for the proxied bzr call, and two new failure
controls, one per new failure direction:

- `LIFECYCLE_SAVED_SEARCH_UNFILTERED` — the proxied bzr call returns the control set
  instead of the seeded subset. The row must go red. This is the deliberately broken
  filter of the issue's third acceptance criterion.
- `LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED` — python-bugzilla's result omits the pybz bug,
  i.e. it appears to honour the parameter. The row must go red.

The existing eight `run_gap_ineligible_control` entries for `saved-search`, the
`LIFECYCLE_STALE_GAPS` scenario, the slug list, and the PASS/FAIL/GAP counts are
re-established against the new row rather than assumed to survive it.

### 4. The parity record

`docs/dev/python-bugzilla-parity.md`'s `Server saved search` row is restated to say what
the row now proves, and its literal copy in the parity-report fixture
(`container-tests.sh`, `run_parity_report_fixture`) is updated to the identical string.
Nothing enforces that agreement, so the plan makes it one task with both edits in it.

## What this does not prove

The Red-Hat-shaped arm proves bzr's behaviour against a fixture built from the vendor's
documented parameter names. It is not evidence that Red Hat Bugzilla resolves a named
query the same way; no Red Hat source was read and no Red Hat server was contacted. This
is the same limit ADR 0052 already accepted for detection, and the parity record states it
so a green row does not imply a fidelity nobody established.

## Testing

| Property | Where it is proven | Needs a container |
|---|---|---|
| Proxy advertises `RedHat` | `ShapeTests` in `redhat-shape-proxy.py` | no |
| Proxy filters on `savedsearch` | `ShapeTests` | no |
| Proxy honours/rejects `sharer_id` | `ShapeTests` | no |
| Proxy leaves traffic alone when the mode is off | `ShapeTests` | no |
| The row goes red on an unfiltered proxied result | `container-tests.sh` control | no |
| The row goes red if python-bugzilla appears to filter | `container-tests.sh` control | no |
| Filtered and control sets actually differ on seeded data | `make functional-compare-all` | yes |
| End-to-end row green on all three images | `make functional-compare-all` | yes |

`make lint` and `make test` reach none of this; `make functional-compare-all` is the gate.

## Threat model

Not security-relevant under `$quest` step 6's triggers: no `src/` change, no shipped
artifact, no trust boundary in the product. Two boundaries exist inside the test harness
and are noted because the proxy parses attacker-shaped input in principle:

- **Proxy ← backend response.** Already bounded: `_forward` caps request bodies at 1 MiB
  and the existing hooks already `json.loads` backend responses. The new transformer adds
  no new parse of a new source; it reads the same already-parsed body.
- **Proxy ← its own environment.** `BZR_FUNC_SAVED_SEARCH_IDS` is set by the harness, not
  by a remote party. It is parsed strictly (decimal ids only) and a malformed value
  disables the fixture rather than being coerced, so a typo fails the row loudly instead
  of silently filtering to nothing.

No credential handling changes; the existing `prepare_auth_forward` path is untouched.
