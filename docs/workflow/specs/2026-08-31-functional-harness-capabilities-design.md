# Functional-harness capabilities for the REST conformance epic

Issue: [#617](https://github.com/randomparity/bzr/issues/617). Epic: [#616](https://github.com/randomparity/bzr/issues/616).

## Goal

Deliver the five functional-harness capabilities that the conformance entries of epic #616
depend on, in a change that lands green on its own and corrects no fixture.

## Why this is separable

Epic requirement R4 assigns fixture correction to the pull request that supplies the matching
code fix, because a corrected fixture is red until that fix exists. This entry therefore
delivers only capability plus an inventory: the harness gains what the dependents need, and
every fixture that mirrors the implementation's own mistake gains a `TODO(#N)` naming the entry
that owns its correction. Changing an asserted value here would make this entry red on arrival,
which is exactly the failure R4 exists to prevent.

## Non-goals

- Correcting any inventoried fixture value. Owned by #621, #622, #625, #626.
- Any change to a `src/` production code path.
- `src/cli/product.rs` and `tests/functional/phases/03-products.sh`, owned by the concurrent
  run on #618.
- Restructuring the functional harness beyond the extension points named below (epic #616
  non-goals).

## No new ADR

The proxy work extends accepted [ADR 0028](../../adr/0028-signed-metadata-sort-keys.md), which
already decided that leniency findings are proved by rewriting a successful response into its
production shape behind the functional proxy, with proxy self-tests. This change generalizes
the mechanism that record chose; it neither supersedes nor contradicts it, and it introduces no
new payload contract, schema, or public behavior. The remaining deliverables — a Make target, a
fixture user, a `CONTRIBUTING` section, and comment-only markers — carry no decision with viable
alternatives at the level ADRs record here.

## Deliverable 1 — enabled non-member user fixture

### Problem

`tests/functional/phases/07-groups.sh:87` asserts that an added member appears in
`group list-users --group functest-grp`. That holds even if the `group=` filter is ignored
entirely, because the unfiltered listing contains every enabled user. The absence assertion at
`:110-112` is reached only after `:108` re-disables `testuser`, and Bugzilla's default user
search hides disabled users — so `group remove-user` is effectively unverified. Asserting the
filter correctly requires a second user who is **enabled** and **not a member**.

### Design

`tests/functional/lib.sh` gains one fixture global and two helpers, beside the existing TLS and
production-shape fixture sections:

- `NONMEMBER_EMAIL` — the fixture login, `functest-nonmember@test.bzr`. The prefix deliberately
  shares no substring with `testuser`, so the existing `user search testuser` and
  `assert_stdout_not_contains "testuser@test.bzr"` assertions cannot match it.
- `ensure_enabled_nonmember_user` — idempotently creates that user and then guarantees it is
  login-enabled. Idempotent in both halves: an "already exists" create is success, and the
  enable runs unconditionally so a prior run that disabled the user is repaired. Returns
  non-zero with a diagnostic on stderr when either half fails. It creates with an **explicit
  `--password`**, matching the existing fixture at `06-users.sh:12` and `:51-52`: `--password`
  is optional, and omitting it makes the server generate one and mail it
  (`src/cli/user.rs:42-43`), which would put an outbound-mail path this harness does not
  configure into a fixture's critical path.
- `assert_user_login_enabled <login>` — runs `user search <login> --details` and fails the
  current test unless the server reports `can_login` true for that exact login. It overwrites
  the `BZR_*` capture globals, which its doc comment states, because every `assert_*` helper in
  this file reads those globals.

The enabled half is the load-bearing half and needs its own assertion: a fixture that silently
degraded to disabled would make a dependent's absence assertion pass for the same wrong reason
the current one does.

**One assumption, stated rather than assumed:** that a freshly created, enabled user reports
`can_login` true on bz50, bz52, and bz53. Nothing in the harness demonstrates it — the only
`can_login` assertion today is `06-users.sh:66-69`, and it asserts `false` after an explicit
disable. The first `make functional-test-all` run is what verifies it. If an arm comes back red
on `fixture-enabled-non-member-user`, read that as this assumption failing on that server
version, not as the helper being wrong, and fix the fixture rather than weakening the assertion —
an assertion that tolerates a disabled user is worth nothing to the dependent that consumes it.

**The non-member half is asserted, not assumed.** `ensure_enabled_nonmember_user` establishes
*exists* and *enabled*; nothing in it establishes *not a member*, which would otherwise hold only
because nothing adds this login to a group. That is weaker than it looks, because
`setup-bugzilla.sh start` reuses an existing container for the checkout-and-version pair, so
group membership survives across runs and across branches in the same worktree. An aborted #625
iteration that added the user to `functest-grp` would leave the container in a state where
#625's own assertion fails against a fixture it does not own — a defect passing for the wrong
reason inside the fixture built to eliminate exactly that.

So a third helper, `assert_user_group_membership <login> <group> <in|out>`, asserts it directly
from the `groups` array `user search --details` already returns (`USER_FIELDS_DETAILED`,
`src/client/mod.rs:22`). This reads the **user** resource, not `group list-users`, so it is
independent of the filter #625 owns and is green today.

One trap it must not fall into: an empty `groups` array — which is what a server would return if
the calling credential could not see membership — makes an `out` assertion pass for the wrong
reason. So the phase test pairs it with an `in` assertion on `testuser@test.bzr`, a known member
by that point in the phase. That positive control is what proves the harness can see membership
at all. Whether `groups` is populated for this credential on every arm is verified by the first
`make functional-test-all` run, not assumed: if the control fails on an arm, the paired test is
removed and the residual returns to a recorded invariant — **a phase that adds
`$NONMEMBER_EMAIL` to a group must remove it** — with that arm named. The `out` half is never
kept without the control.

`tests/functional/phases/07-groups.sh` provisions the fixture after `group add-user` and
asserts it is enabled. It does **not** assert the non-member is absent from the group listing —
that assertion is red until #625 lands the filter fix, and it is #625's to write.

### Interaction with later phases

The fixture adds one enabled user to the instance. The assertions that enumerate users are
`user search testuser` (06-users), which cannot match the new login, and the group-listing
projections in 07-groups, which assert key counts and element existence rather than
cardinality or ordering. No phase asserts a user count.

## Deliverable 2 — `make functional-test-bz50`

`Makefile:201-207` defines `functional-test-bz52` and `functional-test-bz53`;
`tests/functional/run-all-versions.sh:8` runs `bz50` as well, and `functional-stop-all` already
stops it.

The 5.0 arm is not unreachable today: `make functional-test` depends on `functional-start`,
which invokes `setup-bugzilla.sh` with no version override, and `container-env.sh:7` reads
`BZ_VERSION="${BZR_BZ_VERSION:-bz50}"` — so that target already runs the 5.0 arm alone. What is
missing is that the version under test is **implicit** there while 5.2 and 5.3 are named, which
makes the three arms asymmetric exactly where a contributor is citing one arm by name in a
controlled-fault observation.

Add `functional-test-bz50` in the same two-line shape as its siblings, ordered before them, and
list it in `.PHONY`. Nothing else changes: `run-all-versions.sh` keeps driving the matrix, so
the target adds a name rather than a second source of truth for the version list.

The consequence to record is that `make functional-test` and `make functional-test-bz50` now
denote the same arm, and they stop agreeing the moment `container-env.sh`'s default moves off
`bz50` — both would still succeed, silently testing different versions. A comment on the new
target names `functional-test` as its unpinned form and points at the default that couples
them, so the next person to move that default sees the pair.

## Deliverable 3 — per-endpoint rewrite hooks in the production-shape proxy

### Problem

`tests/functional/redhat-shape-proxy.py` applies three rewrites through three hand-written
`if` blocks inside `_forward`, each with its own path test, its own error handling, and — for
one of the three — its own stderr marker. Epic requirement R7 makes this proxy the proof
mechanism for every leniency finding, so entries #620, #626, #627, and #629 will each add
another rewrite. Four more copies of that block is the wrong shape.

### Design

A registry of hooks, one uniform contract, one dispatcher.

```python
ResponseHook = collections.namedtuple("ResponseHook", "name matches route rewrite")
```

- `name` — the stable marker token written to stderr, e.g. `metadata-sort-keys`.
- `matches(path) -> bool` — whether this hook claims the request path (raw path with query, as
  the handler sees it).
- `route(path) -> str` — the sub-route label for the stderr marker, so one hook covering two
  endpoints reports which one fired.
- `rewrite(path, body) -> (bytes, int)` — the transform and the number of values it changed.
  A zero count means the hook changed nothing, and the dispatcher then writes no marker for it.

**The count governs the marker, never the payload.** `apply_response_hooks` always adopts each
matching hook's returned body, zero count included. This is the behavior-preserving reading:
`shape_bug_response` and `shape_product_ids_response` today re-serialize every matching 2xx
response with `separators=(",", ":")` whether or not they changed a value — a POST bug-create
response `{"id": N}` on `/rest/bug` is the reachable case, since `_forward` serves POST and the
bug matcher is a bare `/rest/bug` prefix — so discarding a zero-count body would change what
those responses look like on the wire. The count is a reporting signal only.

`apply_response_hooks(path, body) -> (bytes, list[(name, route, count)])` walks
`RESPONSE_HOOKS` in order, applies every matching hook, and returns the rewritten body with one
entry per hook that changed something. `_forward` calls it once for any 2xx response, maps a
`UnicodeDecodeError` or `json.JSONDecodeError` to the existing 502, and writes one
`"{name} shaped route={route} count={count}"` line to stderr per applied hook.

Each hook defines what its count counts: `bug-multivalue` counts every field whose scalar value
was replaced by a list, the empty string included (`""` becomes `[]`, which the existing
`test_shapes_scalar_empty_and_multi_values` case already exercises); `product-ids` counts the
non-string elements rewritten in `ids`; and `metadata-sort-keys` keeps its existing count of
rewritten `sort_key` values.

The three existing rewrites become the three initial registry entries. Their transforms keep
their names and gain the uniform signature:

| name | matches | route | rewriter |
|---|---|---|---|
| `bug-multivalue` | path starts with `/rest/bug` | `bug` | `shape_bug_response` |
| `product-ids` | path starts with `/rest/product_accessible`, `/rest/product_selectable`, or `/rest/product_enterable` | `product-ids` | `shape_product_ids_response` |
| `metadata-sort-keys` | `is_metadata_sort_key_route(path)` | `field` or `product` | `shape_metadata_sort_keys_response` |

### Behavior this must preserve

The proxy has three consumers, all of which must stay green:

- `tests/functional/phases/03-products.sh:71-83` counts lines matching
  `metadata-sort-keys shaped route=field count=[1-9][0-9]*` and the `product` variant in the
  proxy log. That file belongs to a concurrent run and is not edited here, so the sort-key
  hook's `name` and `route` values are fixed by that contract and the emitted line stays
  byte-identical. Two details the pattern encodes: it matches only a **non-zero** count, which
  agrees with the dispatcher writing no marker at zero; and `:81-84` further requires
  `server capabilities` to emit *additional* `route=field` markers, so the field matcher's
  coverage of the endpoints that command touches is part of the preserved contract, not just
  the line's spelling. `:49-53` is the third assertion in that phase, requiring
  `product list --type accessible` to return a non-empty array through the proxy — the
  `product-ids` hook's end-to-end consumer.
- **The explicit `sys.stderr.flush()` after each marker write.** `lib.sh:622-623` redirects the
  proxy's stderr to `$REDHAT_SHAPE_LOG`, a *file*, and `03-products.sh` reads that file with awk
  while the proxy is still running (it is not stopped until `:87`). On CPython before 3.9 stderr
  to a non-tty is block-buffered, so without the flush the markers sit in the buffer and the
  mid-run counters read zero, failing the phase. On 3.9+ stderr is line-buffered and the flush
  is redundant — which is exactly what makes it easy to drop in a refactor and invisible on a
  modern box. It is part of the contract, not decoration.
- `tests/functional/phases/18e-release-readiness.sh:136-166` pins `shape_bug_response`'s output
  byte-for-byte across `bug list --paginate`, `bug search --from-url`, and `bug adjacency`,
  asserting `.component == [$component, ($component + "-redhat-secondary")]` and the `version`
  equivalent. The bug transform's values, not just its marker, are a contract.
- `tests/functional/phases/18d-dependency-analysis.sh:734` routes an installed collector and a
  termless-preflight exit-4 assertion through the same proxy, so the non-2xx paths
  (`is_termless_bug_search`, the 400 body) must keep bypassing the hook registry as they do now.

Matchers keep the existing raw-path prefix tests rather than switching to parsed paths, so no
request changes hook membership. The route predicate the sort-key rewriter applies internally
is extracted to `is_metadata_sort_key_route` and used both as the hook's matcher and as the
rewriter's own guard, so the function stays independently correct and the condition is written
once.

The two rewrites that log nothing today start emitting their own markers. That is additive: the
awk counters in `03-products.sh` match on their own prefixes, and the proxy log is otherwise
read only by humans. It is also the half of the ADR 0028 pattern that lets a dependent prove
its rewrite actually fired rather than assuming it did.

### Documentation

A comment block above `RESPONSE_HOOKS` states the four fields, the rewriter contract, the
count-governs-the-marker rule, the obligation to add self-tests, and the gate that enforces
that obligation. `tests/functional/README.md` gains a short "Adding a production-shape rewrite"
section pointing at it.

### The self-tests need a gate

Today nothing runs `--self-test`: `make lint` does not, `run-tests.sh` does not, and no CI
workflow does. That is fatal to the role this design gives them — they are the stated mitigation
for a hook refactor silently dropping a rewrite, and the comment block imposes a self-test
obligation on the four later entries R7 routes through this proxy. An obligation checked by
nobody is a convention, and it will not survive four pull requests.

So `Makefile` gains `check-proxy-self-test`, running
`python3 tests/functional/redhat-shape-proxy.py --self-test`, added to the `lint` prerequisite
list and to `.PHONY`. It guards on `python3` the way `check-shell` guards on `shellcheck` — an
actionable error rather than a silent skip.

**Naming the whole prerequisite, not just the interpreter.** Four of the suite's fourteen cases
route through `_start_server` (`redhat-shape-proxy.py:380-387`) to start a real
`ThreadingHTTPServer` on `127.0.0.1`, issue live
requests with two-second timeouts, and join threads; they are essentially all of its 2.0s
runtime. So `make lint` gains loopback TCP bind-and-connect and a few seconds of timing
headroom, not merely `python3` — every existing `lint` prerequisite is offline filesystem work.
This is documented rather than avoided, so a `make lint` failure in a restricted sandbox is
diagnosable instead of mysterious; the suite was measured passing in an agent sandbox on this
host (14 tests, 2.010s, `OK`). One residual is accepted and named:
`test_unavailable_backend_returns_502` picks its unavailable port by binding port 0 and closing
the socket, so a process that grabs that port in the window fails the case. The window is
sub-millisecond and the case predates this change; promoting the suite to a gate raises the
exposure, and the honest response is to record it here rather than to rewrite a working test.
Since `python3` becomes a `make lint` requirement, `tests/functional/README.md`'s prerequisite
list (which today calls it TLS-phase-only) and `CONTRIBUTING.md`'s development-setup section are
corrected in the same change — a change that adds a prerequisite updates provisioning with it.

**`make lint` alone would not be a gate.** No workflow runs it: `rg -n 'make lint|make check-'`
over `.github/workflows/` returns only `ci.yml:46-48` (`check-test-layout`,
`check-functional-test-ids`, `check-no-spawn`) and `ci.yml:264` (`check-shell`), and the
pre-commit hook the Makefile writes at `:65` runs fmt, clippy, `check-test-layout`, and
`check-functional-test-ids` — not `lint`. This repository's convention is that a guard reaches
CI by being named as its own workflow step; `check-build-script` and
`check-release-security-notes` sit in `lint`'s prerequisite list and are consequently run by
nothing automated.

So the gate is delivered in both places, following that convention: the `lint` prerequisite for
contributors and agents, and one `- run: make check-proxy-self-test` step in `ci.yml`'s existing
`test-layout` job, beside the three `make check-*` steps already there. `python3` is preinstalled
on `ubuntu-latest`, so the job needs no new setup step. **This adds `.github/workflows/ci.yml` to
the change surface** beyond the file list the issue suggests — a one-line step in an existing
job, no new action, no permission change, no dependency — because without it the obligation this
section imposes on four later entries is enforced by nothing they will run.

### Self-tests

`--self-test` gains cases for the registry itself, alongside the existing per-transform cases
updated to the new signature:

1. A path no hook claims returns the body unchanged and an empty applied list.
2. A `/rest/bug` body reports `("bug-multivalue", "bug", <count>)` with the count equal to the
   number of rewritten fields.
3. A `/rest/field/bug` body reports `("metadata-sort-keys", "field", 3)`, and a `/rest/product`
   body reports the `product` route — the contract `03-products.sh` asserts end to end.
4. A `/rest/product_accessible` body with numeric `ids` reports
   `("product-ids", "product-ids", <count>)`, and a `/rest/product` body does not. Without this
   the `product-ids` matcher — a three-prefix `startswith` tuple — is dispatched by no case at
   all: case 5 below calls every rewriter directly and bypasses matching entirely, so a dropped
   prefix or a tuple that stops being a tuple would leave the whole suite green. The only
   residual detector is nothing at all. `03-products.sh:49-53` looks like one but is not: it
   asserts only `length > 0` on `product list --type accessible` through the proxy, and a proxy
   that stopped rewriting would serve the backend's native numeric `ids` — the *easier* shape,
   which the client parses fine. Only the string shape the rewrite forces was ever the hard
   case. So case 4 is the whole detector, not a belt beside a brace.
5. Every registered hook's rewriter returns a `(bytes, int)` pair on a payload it declines. This
   is the contract a new hook author gets wrong, so it is asserted over the registry rather than
   over a fixed list of names.
6. A hook that matches but changes nothing produces no applied entry.

## Deliverable 4 — the controlled-fault procedure in CONTRIBUTING

Epic requirement R2 makes every conformance entry demonstrate its test red against pre-fix code
and green after. Per accepted ADR 0021 contributor guidance lives in `CONTRIBUTING.md`, so a
new `### Controlled-fault verification` subsection under `## Verification` records one
procedure the following pull requests cite:

1. Write or strengthen the test first.
2. Remove the fix from the working tree — `git stash push` the source paths, or invert the one
   line under test. Do not weaken the test.
3. Run the narrowest command that covers it: `make test-one T=<substring>` for a unit test,
   `python3 tests/functional/redhat-shape-proxy.py --self-test` for a proxy rewrite, or a single
   functional arm — `make functional-test-bz50` / `-bz52` / `-bz53`, or `make functional-test`
   for the unpinned default.
4. Observe the failure and record the command and the failing assertion.
5. Restore the fix, confirm the tree is actually restored (`git stash list`, `git status`),
   re-run the same command, observe green.
6. Put both observations in the pull-request body. A test that passes in both states does not
   close its finding.

**The functional arm needs an explicit rebuild before each of steps 3 and 5**, and the procedure
says so with the reason attached: `phases/00-build.sh:16-17` uses `$BZR_BIN` verbatim whenever it
is set and executable, skipping `cargo` entirely. `BZR_BIN` is a documented override and CI sets
it, so a contributor with it exported in their shell never rebuilds and the fault never reaches
the binary under test. The procedure therefore prefixes the functional arm with `unset BZR_BIN`
and an explicit `cargo build --release` before the container run.

A **failed** build is not the hazard, and the procedure must not claim it is. `00-build.sh:20`
runs `cargo build --release 2>&1 | tail -3`, which looks like the exit-status-hiding pipeline the
repository's guardrail rules forbid — but `run-tests.sh:14` is `set -euo pipefail` and phase files
are *sourced* into that shell (`:91`), so `pipefail` gives the pipeline cargo's status and
`errexit` aborts the runner before any phase executes. Verified with a runner mirroring those
options and a cargo stub exiting 101: the run dies with status 101 and nothing after the source
executes. Documenting the opposite would put a false statement about the repository's own
guardrails into `CONTRIBUTING.md`, which ADR 0021 makes authoritative for exactly that.

**The container is the other half of what a controlled fault must control.** The same reuse
Deliverable 1 records above applies here: `setup-bugzilla.sh:119-127` returns early whenever the
named container is already running, so the ordinary second run of an arm lands on a warm, dirty
instance. That breaks attribution in both directions — a mutation left by the pre-fault run can
already satisfy the assertion under test, so the faulted arm passes; and residue from an aborted
run, such as the `$NONMEMBER_EMAIL` group membership named above, can make the restored arm fail.
Either way the red/green pair the pull-request body reports is not attributable to the fault. So
the procedure resets the container before each of steps 3 and 5:
`BZR_BZ_VERSION=<arm> tests/functional/setup-bugzilla.sh reset`, which is stop-then-start
(`:186-190`) and yields a fresh instance from the image. Stale binary and stale instance are the
two ways the same observation goes wrong, and the procedure closes both.

## Deliverable 5 — inventory of compromised fixtures

Comment-only `TODO(#N)` markers, no asserted value changed, each naming the entry that owns the
correction:

| Site | Marker | What is wrong |
|---|---|---|
| `src/commands/bug/clone_tests.rs:91`, `:329` | `TODO(#621)` | mocks `rep_platform` as a response key; the server sends `platform` |
| `src/client/resources/group_tests.rs:16,52,87,106` | `TODO(#625)` | codifies the `group=` query parameter, which the API does not recognize |
| `src/client/resources/server_tests.rs:93,149` | `TODO(#626)` | numeric `maxattachmentsize`; stock servers send a string |
| `src/xmlrpc/resources/mappers_tests.rs:60` | `TODO(#622)` | dashed datetime the XML-RPC serializer never emits |
| `tests/functional/phases/07-groups.sh` list-users assertions | `TODO(#625)` | passes whether or not the group filter is applied |
| `tests/functional/phases/02-server-auth.sh` `server capabilities` | `TODO(#626)` | never asserts `max_attachment_size` non-null on the credentialed path |

The issue's line citations were checked against `HEAD` before writing this table; the
`clone_tests.rs` fixture appears twice, and `02-server-auth.sh`'s only
`max_attachment_size` assertion is the credentialless `null` case at `:74`, which is correct
under ADR 0005 and stays. The untested gap is the credentialed non-null case, so the marker
sits on the credentialed `server capabilities` test.

## Testing and acceptance

- `make lint` — now includes `check-proxy-self-test`, so all proxy cases pass as part of the
  guardrail contributors and agents are told to run. It also includes `check-shell` (shellcheck
  and `bash -n` over `lib.sh` and every phase) and `check-functional-test-ids`, which constrains
  the new `test_begin` identifier to `^[a-z0-9]+(-[a-z0-9]+)*$`.
- `make check-proxy-self-test` in CI, as its own step in `ci.yml`'s `test-layout` job — the half
  that makes the self-test obligation binding on a pull request rather than on memory.
- `make test` — green; the markers are comments, so no Rust behavior changes.
- `make functional-test-all` — green on bz50, bz52, and bz53. That covers all three proxy
  consumers: `03-products.sh` (sort-key markers), `18e-release-readiness.sh` (bug-transform
  values), and `18d-dependency-analysis.sh` (the non-2xx preflight path).
- `make functional-test-bz50` — run once, on its own, **and read the phase-0 banner**
  (`00-build.sh:11` prints `bzr functional tests (<version>)`) to confirm it says `bz50`. This is
  Deliverable 2's only proof: `run-all-versions.sh:20-40` calls `setup-bugzilla.sh` and
  `run-tests.sh` directly with `BZR_BZ_VERSION` exported and never invokes a Make target, so
  `functional-test-all` does not traverse the new recipe. The banner is the part that adds
  coverage. An *unknown* token needs no proof — `setup-bugzilla.sh:23-35` rejects anything but
  `bz50`/`bz52`/`bz53` on the target's first line, before any container work. A token
  mis-**copied** from the sibling recipe is the reachable defect: `BZR_BZ_VERSION=bz52` starts
  the 5.2 container, runs the identical phase list, and exits 0, because that is exactly what
  `make functional-test-bz52` asserts today. Green alone therefore does not distinguish the two;
  the banner does.
- Both `TODO` observations: these are the first `TODO` comments anywhere under `src/`
  (`rg -n 'TODO' src/` is empty at HEAD), and `sonar-project.properties` sets
  `sonar.sources=src`, so SonarQube analyses them on every non-fork pull request
  (`ci.yml:217-247`). The expected effect is a reported new-issue class on the dashboard, not a
  failing check: `sonar.qualitygate.wait` is not configured and the scan step passes no
  gate-wait input. No exclusion or marker-syntax change is warranted — the markers are exactly
  the traceability epic #616 asked for.
- Controlled fault, applying the procedure this change documents: point
  `ensure_enabled_nonmember_user` at `--disable-login true`, observe the new phase test red,
  restore, observe green; break one hook matcher in `RESPONSE_HOOKS`, observe the registry
  self-tests red, restore, observe green.

## Failure modes considered

- **The fixture user leaks into an unrelated assertion.** Mitigated by the login prefix, and
  checked by running the full matrix rather than one arm.
- **The hook refactor silently drops a rewrite.** The registry self-tests assert route and count
  per hook, and `03-products.sh` still counts the sort-key markers end to end.
- **A 2xx response that is not JSON.** Unchanged: a hook that claims the path raises, and
  `_forward` maps it to 502. A hook that does not claim it is never called, so a non-JSON body
  on an unrelated route still passes through, as today.
- **`make functional-test-bz50` drifting from the matrix.** The target starts the container and
  runs the same `run-tests.sh`; `run-all-versions.sh` keeps owning the version list.
