# Design: prove the comparison harness's server-side saved search filters

Issue: [#710](https://github.com/randomparity/bzr/issues/710)
Decision record: [ADR 0061](../../adr/0061-prove-vendor-extension-behaviour-against-a-shaped-proxy.md)
Build instructions: [the implementation plan](../plans/2026-09-06-prove-server-saved-search-filter.md)
Governing client decision (read, not modified): [ADR 0052](../../adr/0052-detect-vendor-extension-support-before-dispatch.md)

ADR 0061 is the single home for the evidence and the rationale. This spec states the shape
of the change and the properties it must hold; the plan states how to build it. Neither
repeats the ADR's argument.

## Problem

`tests/functional/compare/01-bug-lifecycle.sh`'s `saved-search` row reports a result it
cannot establish. It seeds a named query selecting the two lifecycle bugs, asks
python-bugzilla to run it, and asserts the returned id set equals those two ids — but every
supported image discards `savedsearch` and returns the whole database, and at that point in
a freshly reset run the whole database *is* those two bugs. The row compares a set against
itself, so a server honouring the parameter and one discarding it produce the same PASS.
Since ADR 0052 shipped, nothing exercises the dispatch path that ADR gates either.

## Goal

The row must be able to fail in both directions — if bzr stops sending `savedsearch`, and if
the server stops honouring it — and, separately, the sets it compares must actually be able
to differ on the seeded data. An assertion that bites when broken can still be comparing two
identical sets; that is the defect being fixed, so the two properties are tracked apart.

## Non-goals

- No change to `bzr` itself. ADR 0052's detection and refusal ship separately (#670).
- No change to `docs/adr/README.md`; the ADR 0061 index row is reported as pending.
- No new `bzr` flag, and no weakening of ADR 0052's refusal to make a test pass.

## Shape of the change

**A `saved-search` mode on `tests/functional/redhat-shape-proxy.py`**, selected by
`BZR_FUNC_REDHAT_MODE` and registered in `REWRITE_HOOKS` like every other shape it serves. It
injects a `RedHat` entry into `GET /rest/extensions` so bzr's ADR 0052 gate passes, and
filters `GET /rest/bug` responses carrying `savedsearch` to the ids the named query selects.
The fixture reads `BZR_FUNC_SAVED_SEARCH_{NAME,IDS,SHARER}`. Resolution: no `savedsearch` →
forwarded unchanged (this is what fails the row if bzr stops sending it); a non-matching name
→ empty; a matching name with absent or matching `sharer_id` → filtered to the fixture ids; a
matching name with a different `sharer_id` → empty. A malformed ids list disables the fixture
rather than filtering to nothing, so a harness typo fails loudly instead of producing a
plausible result. The mode is inert unless enabled.

**A discriminating row.** The row keeps its identity and stays an expected gap — the
stock-server difference it records is real. The seeded query changes to the bzr bug only, so
it selects a strict subset of an unfiltered search. Against the shaped proxy, two bzr calls:
an unfiltered control (`bug list --summary "$LIFECYCLE_STEM"`) asserted to **contain** both
lifecycle bugs, and the filtered call (`bug search --saved-search …`) asserted to **equal**
the seeded subset. Against the stock container: bzr refuses per ADR 0052, and
python-bugzilla's result is asserted to contain the pybz bug the seeded query excludes —
which turns the parity record's standing claim into a tested one.

The two proxied calls are **not** one flag apart; they differ by subcommand, search term, and
flag, because the proxy answers a termless `/rest/bug` with `code 1000`. What the pair
establishes is a **strict-superset relation** — the control contains what the filtered call
equals — and that is what removes the vacuity. Preserve that relation, not a notion of
parameter isolation the calls do not have.

Ordering is load-bearing: `expect_gap` converts the row's outcome, so the stock refusal must
be the last bzr probe and every new assertion sits in the precondition chain, where a failure
is an outright FAIL that never reaches `lifecycle_expect_gap 670`. That is the shape the
eight existing `run_gap_ineligible_control` entries already assert.

**Three self-test controls, one per assertion**, in `tests/functional/pybz/container-tests.sh`,
which sources the real phase script against stubs: `LIFECYCLE_SAVED_SEARCH_UNFILTERED` (the
filtered call returns the control set), `LIFECYCLE_SAVED_SEARCH_CONTROL_NARROW` (the control
no longer exceeds the subset, so the comparison would be vacuous), and
`LIFECYCLE_SAVED_SEARCH_PYBZ_FILTERED`. Each must redden through its own assertion rather
than a neighbour that short-circuits ahead of it.

**The parity record**, whose row is restated and kept byte-identical to its fixture copy.

## Properties this design must hold

| Property | Where proven | Container |
|---|---|---|
| Advertises `RedHat`; filters on `savedsearch`; honours/rejects `sharer_id` | `ShapeTests` | no |
| Mode gating, through `make_handler` rather than the transformer alone | `ShapeTests` round trip | no |
| Seeder takes exactly one id and rejects a missing or non-decimal one | `container-tests.sh` seed fixture | no |
| Each of the row's three assertions reddens through itself | three `container-tests.sh` controls | no |
| The control really does exceed the subset on seeded data | `make functional-compare-all` | yes |
| Row green on all three images | `make functional-compare-all` | yes |

`make test` reaches none of this. `make lint` does reach the two shell files, via
`check-shell` (shellcheck, `bash -n`) and `check-functional-test-ids`; nothing lints
`redhat-shape-proxy.py`. `make functional-compare-all` is the gate for the behaviour.

## What this does not prove

The Red-Hat-shaped arm proves bzr's behaviour against a fixture built from the vendor's
documented parameter names. It is not evidence that Red Hat Bugzilla resolves a named query
the same way; no Red Hat source was read and no Red Hat server was contacted. The parity
record says so, so a green row does not imply a fidelity nobody established.

## Threat model

No `src/` change, no shipped artifact, no trust boundary in the product. The change does
parse input it did not produce, so a `$detect-evil` pass was run on the branch rather than
skipped; it returned `approve` with no blocking finding. Three harness-internal boundaries:

- **Backend response → proxy JSON parse.** `_forward` reads the backend body with a bare
  `response.read()` and no size ceiling — `_MAX_REQUEST_BODY` bounds the *inbound client
  request*, not this. That read is pre-existing and unconditional for every route, so the new
  transformers add no new read and no new ceiling is owed; what bounds the crossing is that
  the response producer is the loopback test container the harness itself started. Malformed
  bodies are handled: `_forward` catches `UnicodeDecodeError` and `json.JSONDecodeError`, and
  a `bugs` value that is not a list is forwarded untouched rather than coerced.
- **New parsed route.** `shape_saved_search_extensions` parses `/rest/extensions`, which no
  pre-existing hook touched. Controls: exact route equality, `isinstance` guards on the
  decoded value, the mode gate, and the same decode-error handler above.
- **Proxy environment.** `BZR_FUNC_SAVED_SEARCH_{NAME,IDS,SHARER}` are harness-set, parsed
  strictly (`isascii` + `isdigit` + positive), and a malformed value disables the fixture
  rather than raising out of the handler thread.

No credential handling changes; `prepare_auth_forward` is untouched and the new mode leaves
`bearer_auth_mode` false, so headers forward unaltered. No secret reaches stdout, stderr, or
an evidence marker.
