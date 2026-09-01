# Functional-harness capabilities for the REST conformance epic

Issue: [#617](https://github.com/randomparity/bzr/issues/617). Epic: [#616](https://github.com/randomparity/bzr/issues/616).

## Goal

Deliver four of the five functional-harness capabilities that the conformance entries of epic
#616 depend on, in a change that lands green on its own and corrects no fixture.

**Scope cap.** An operator decision after design deferred acceptance criterion 3 — the
production-shape proxy's rewrite-hook registry — to [#634](https://github.com/randomparity/bzr/issues/634).
Criterion 3 is **not met by this change**; see Deliverable 3 below.

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
- The production-shape proxy's rewrite-hook registry. Deferred to #634;
  `tests/functional/redhat-shape-proxy.py` and every workflow file are untouched.
- `src/cli/product.rs` and `tests/functional/phases/03-products.sh`, owned by #618, whose merge is
  already in this branch.
- Restructuring the functional harness beyond the extension points named below (epic #616
  non-goals).

## No new ADR

The delivered work — a Make target, a fixture user and its helpers, a `CONTRIBUTING` section, and
comment-only markers — carries no decision with viable alternatives at the level ADRs record here,
introduces no payload contract, schema, or public behavior, and contradicts no accepted record.
The deferred proxy work extends accepted
[ADR 0028](../../adr/0028-signed-metadata-sort-keys.md) rather than superseding it; that reasoning
travels to #634.

## Deliverable 1 — enabled non-member user fixture

### Problem

`tests/functional/phases/07-groups.sh:87` asserts that an added member appears in
`group list-users --group functest-grp`. That holds even if the `group=` filter is ignored
entirely, because the unfiltered listing contains every enabled user. The absence assertion at
`:110-112` is reached only after `:108` re-disables `testuser`, and Bugzilla's default user
search hides disabled users — so `group remove-user` is effectively unverified. Asserting the
filter correctly requires a second user who is **enabled** and **not a member**.

### Design

`tests/functional/lib.sh` gains one fixture global and three helpers, beside the existing TLS and
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

## Deliverable 3 — DEFERRED to #634, not delivered here

Acceptance criterion 3 — documented per-endpoint rewrite hooks with self-tests in
`tests/functional/redhat-shape-proxy.py` — is **not met by this change**. An operator scope cap
after design moved it to its own entry,
[#634](https://github.com/randomparity/bzr/issues/634), which carries the criterion verbatim.

Two constraints this design established travel with it, because they are not obvious from the
proxy source and a reimplementer would otherwise break them:

- **The marker line is a contract.** `tests/functional/phases/03-products.sh` counts lines matching
  `metadata-sort-keys shaped route=field count=[1-9][0-9]*` and the `route=product` variant, and
  further requires `server capabilities` to raise the `route=field` count. Any refactor must keep
  that string byte-identical, and the pattern matches only a **non-zero** count.
- **The explicit `sys.stderr.flush()` after each marker write is load-bearing.** `lib.sh` redirects
  the proxy's output to a log file that `03-products.sh` reads *while the proxy is still running*.
  On CPython before 3.9 stderr to a non-tty is block-buffered, so dropping the flush leaves the
  markers in the buffer and the mid-run counters read zero. On 3.9+ it is redundant, which is what
  makes it easy to drop and invisible on a modern box.

The proxy has three consumers that any such refactor must keep green: `03-products.sh`,
`18d-dependency-analysis.sh`, and `18e-release-readiness.sh` — the last pins the bug transform's
output values, not just its marker.

`tests/functional/redhat-shape-proxy.py` and every workflow file are untouched by this change.

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

- `make lint` — unchanged in its prerequisite list, and green. It covers this change through
  `check-shell` (shellcheck and `bash -n` over `lib.sh` and every phase) and
  `check-functional-test-ids`, which constrains the new `test_begin` identifiers to
  `^[a-z0-9]+(-[a-z0-9]+)*$`.
- `make test` — green; the markers are comments, so no Rust behavior changes.
- `make functional-test-all` — green on bz50, bz52, and bz53.
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
- Controlled fault, applying the procedure this change documents: replace
  `ensure_enabled_nonmember_user`'s `--disable-login false --login-denied-text ""` with
  `--disable-login true --login-denied-text "fault disabled"`, observe the new phase test red,
  restore, observe green. Replacing the whole flag pair is required:
  `resolve_login_denied_text` (`src/commands/user/update.rs:95-105`) maps
  `(Some(true), Some(""))` to the same empty string as `(Some(false), _)`, so flipping only the
  boolean re-enables the user and the fault is inert.

## Failure modes considered

- **The fixture user leaks into an unrelated assertion.** Mitigated by the login prefix, and
  checked by running the full matrix rather than one arm.
- **`make functional-test-bz50` drifting from the matrix.** The target starts the container and
  runs the same `run-tests.sh`; `run-all-versions.sh` keeps owning the version list.
