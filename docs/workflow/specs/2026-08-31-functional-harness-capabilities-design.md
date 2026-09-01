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
  non-zero with a diagnostic on stderr when either half fails.
- `assert_user_login_enabled <login>` — runs `user search <login> --details` and fails the
  current test unless the server reports `can_login` true for that exact login. It overwrites
  the `BZR_*` capture globals, which its doc comment states, because every `assert_*` helper in
  this file reads those globals.

The enabled half is the load-bearing half and needs its own assertion: a fixture that silently
degraded to disabled would make a dependent's absence assertion pass for the same wrong reason
the current one does.

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
stops it. The 5.0 arm therefore cannot be re-run alone after a red controlled-fault run, which
is when an implementer needs it most.

Add `functional-test-bz50` in the same two-line shape as its siblings, ordered before them, and
list it in `.PHONY`. Nothing else changes: `run-all-versions.sh` keeps driving the matrix, so
the target adds a way in rather than a second source of truth for the version list.

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
  Returning a zero count means the hook declined; the dispatcher then writes no marker.

`apply_response_hooks(path, body) -> (bytes, list[(name, route, count)])` walks
`RESPONSE_HOOKS` in order, applies every matching hook, and returns the rewritten body with one
entry per hook that changed something. `_forward` calls it once for any 2xx response, maps a
`UnicodeDecodeError` or `json.JSONDecodeError` to the existing 502, and writes one
`"{name} shaped route={route} count={count}"` line to stderr per applied hook.

The three existing rewrites become the three initial registry entries. Their transforms keep
their names and gain the uniform signature:

| name | matches | route | rewriter |
|---|---|---|---|
| `bug-multivalue` | path starts with `/rest/bug` | `bug` | `shape_bug_response` |
| `product-ids` | path starts with `/rest/product_accessible`, `/rest/product_selectable`, or `/rest/product_enterable` | `product-ids` | `shape_product_ids_response` |
| `metadata-sort-keys` | `is_metadata_sort_key_route(path)` | `field` or `product` | `shape_metadata_sort_keys_response` |

### Behavior this must preserve

`tests/functional/phases/03-products.sh` counts lines matching
`metadata-sort-keys shaped route=field count=<n>` and the `product` variant in the proxy log.
That file belongs to a concurrent run and is not edited here, so the sort-key hook's `name` and
`route` values are fixed by that contract and the emitted line stays byte-identical.

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
decline-by-zero-count rule, and the obligation to add self-tests. `tests/functional/README.md`
gains a short "Adding a production-shape rewrite" section pointing at it.

### Self-tests

`--self-test` gains cases for the registry itself, alongside the existing per-transform cases
updated to the new signature:

1. A path no hook claims returns the body unchanged and an empty applied list.
2. A `/rest/bug` body reports `("bug-multivalue", "bug", <count>)` with the count equal to the
   number of rewritten fields.
3. A `/rest/field/bug` body reports `("metadata-sort-keys", "field", 3)`, and a `/rest/product`
   body reports the `product` route — the contract `03-products.sh` asserts end to end.
4. Every registered hook's rewriter returns a `(bytes, int)` pair on a payload it declines. This
   is the contract a new hook author gets wrong, so it is asserted over the registry rather than
   over a fixed list of names.
5. A hook that matches but changes nothing produces no applied entry.

## Deliverable 4 — the controlled-fault procedure in CONTRIBUTING

Epic requirement R2 makes every conformance entry demonstrate its test red against pre-fix code
and green after. Per accepted ADR 0021 contributor guidance lives in `CONTRIBUTING.md`, so a
new `### Controlled-fault verification` subsection under `## Verification` records one
procedure the following pull requests cite:

1. Write or strengthen the test first.
2. Remove the fix from the working tree — `git stash push` the source paths, or invert the one
   line under test. Do not weaken the test.
3. Run the narrowest command that covers it: `make test-one T=<substring>` for a unit test,
   `python3 tests/functional/redhat-shape-proxy.py --self-test` for a proxy rewrite, or
   `make functional-test-bz50` / `-bz52` / `-bz53` for a single functional arm.
4. Observe the failure and record the command and the failing assertion.
5. Restore the fix, re-run the same command, observe green.
6. Put both observations in the pull-request body. A test that passes in both states does not
   close its finding.

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

- `python3 tests/functional/redhat-shape-proxy.py --self-test` — all proxy cases pass.
- `make lint` — includes `check-shell` (shellcheck and `bash -n` over `lib.sh` and every phase)
  and `check-functional-test-ids`, which constrains the new `test_begin` identifier to
  `^[a-z0-9]+(-[a-z0-9]+)*$`.
- `make test` — green; the markers are comments, so no Rust behavior changes.
- `make functional-test-all` — green on bz50, bz52, and bz53.
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
