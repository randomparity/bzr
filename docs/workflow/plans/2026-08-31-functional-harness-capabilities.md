# Implementation plan — functional-harness capabilities (issue #617)

**Goal.** Land four of the five harness capabilities epic #616's conformance entries depend on,
without correcting a single fixture and without touching a `src/` production path.

**Scope cap.** An operator decision after design moved acceptance criterion 3 — the per-endpoint
rewrite-hook registry in `tests/functional/redhat-shape-proxy.py` — to its own entry,
[#634](https://github.com/randomparity/bzr/issues/634). Tasks 2 and 3 below are struck. That file
and every workflow file are untouched by this change.

**Architecture.** Everything lives in `tests/functional/` plus one project file. The shell harness
(`lib.sh`, phase scripts) gains a fixture user and three helpers; `Makefile` gains one target; and
`CONTRIBUTING.md` gains a procedure. Four Rust test files gain comment-only markers.

**Design record.** `docs/workflow/specs/2026-08-31-functional-harness-capabilities-design.md`.

**Tech stack.** bash 5 (the harness), GNU Make, Rust 2021 (comments only).

## Global constraints

Every task's requirements implicitly include this section.

- **No fixture value changes and no `src/` production-path changes.** Rust edits in this plan are
  comment lines only. Changing an asserted value would make this change red on arrival; epic #616
  requirement R4 assigns that correction to the dependent entry.
- **Do not edit `tests/functional/redhat-shape-proxy.py`, any workflow file, `src/cli/product.rs`,
  or `tests/functional/phases/03-products.sh`.** The proxy and its gate belong to #634; the other
  two landed under #618, whose merge is already in this branch.
- **Functional test IDs.** `test_begin "<slug>" "<description>"` on one line, two literal
  arguments, slug matching `^[a-z0-9]+(-[a-z0-9]+)*$`, unique within its phase.
  `make check-functional-test-ids` enforces it.
- **Shell style.** `make check-shell` runs `shellcheck -s bash` and `bash -n` over
  `tests/functional/lib.sh` and every `tests/functional/phases/*.sh`. It does **not** run `shfmt`
  over `tests/functional/`; match the surrounding 4-space indentation by hand.
- **Guardrails.** `make lint`, `make test`, `make functional-test-all`. Never bare `cargo test`.
- **Commits.** Conventional Commits, imperative, subject ≤72 chars, one logical change each,
  ending with the trailer `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  Infra scopes (`test`, `docs`, `ci`, `build`) are excluded from the generated changelog by
  design, which is correct for this change — it alters no compiled `bzr` behavior.

## File map

| File | Created / modified | Answerable for |
|---|---|---|
| `Makefile` | modified | the `functional-test-bz50` target |
| `tests/functional/lib.sh` | modified | the non-member fixture global and its three helpers |
| `tests/functional/phases/07-groups.sh` | modified | provisioning the fixture; `TODO(#625)` markers |
| `tests/functional/phases/02-server-auth.sh` | modified | the `TODO(#626)` marker |
| `CONTRIBUTING.md` | modified | the controlled-fault procedure |
| `src/commands/bug/clone_tests.rs` | modified | `TODO(#621)` markers (comments only) |
| `src/client/resources/group_tests.rs` | modified | `TODO(#625)` markers (comments only) |
| `src/client/resources/server_tests.rs` | modified | `TODO(#626)` markers (comments only) |
| `src/xmlrpc/resources/mappers_tests.rs` | modified | `TODO(#622)` marker (comments only) |

## Task 1 — the `functional-test-bz50` target

**Modifies:** `Makefile`. **Tests:** `make -n functional-test-bz50`.

**Interfaces.** Consumes nothing. Later tasks rely on nothing from it; Task 5's CONTRIBUTING text
names the target by name.

### Steps

1. In the `.PHONY` list, change the line
   `        functional-test-bz52 functional-test-bz53 functional-test-all functional-stop-all \`
   to
   `        functional-test-bz50 functional-test-bz52 functional-test-bz53 functional-test-all functional-stop-all \`

2. Immediately before the `functional-test-bz52:` rule, insert:

```make
# `make functional-test` is the unpinned form of this target: it runs whatever
# tests/functional/container-env.sh defaults BZ_VERSION to, which is bz50 today.
# The two agree only while that default is bz50; moving it makes them diverge
# silently, since both still succeed.
functional-test-bz50: ## Run functional tests against Bugzilla 5.0
	BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh start
	BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh
```

   Recipe lines are **tab**-indented, matching the sibling rules.

3. Verify: `make -n functional-test-bz50`. Expect exactly two echoed lines,
   `BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh start` and
   `BZR_BZ_VERSION=bz50 tests/functional/run-tests.sh`, and exit status 0.

4. Verify the rule and its help text landed:
   `rg -n 'functional-test-bz50: ## Run functional tests against Bugzilla 5.0' Makefile`.
   Expect one hit.

   Do **not** verify through `make help`. Its filter at `Makefile:223` is
   `grep -E '^[a-zA-Z_-]+:.*##'`, whose character class has no digit range, so no target with a
   digit in its name is listed — `functional-test-bz52` and `functional-test-bz53` are already
   invisible there today (`make help | grep -c functional-test-bz52` prints `0`). Widening that
   class would change `make help` output for targets this change does not own, so it stays out
   of scope.

   **Knowingly unowned.** No issue is filed for the help-filter gap and none is planned: it is
   pre-existing (two sibling targets are already invisible), this change adds a third rather than
   creating the condition, and the discovery path contributors actually use is the
   `CONTRIBUTING.md` procedure, which names all three arms. Recorded here so it is a decision
   rather than an omission.

5. Commit: `feat(test): add a functional-test-bz50 make target`.

### Acceptance criteria

- `make -n functional-test-bz50` prints the two commands above and exits 0.
- `functional-test-bz50` is in `.PHONY`.
- `run-all-versions.sh` is unchanged — it remains the only list of matrix versions.

## Task 2 — DROPPED (deferred to #634)

The `check-proxy-self-test` guard and its `.github/workflows/ci.yml` step existed only to gate the
Task 3 registry's self-tests. Both move to #634 with the work they gate. `Makefile`'s `.PHONY` list
and `lint` rule are untouched, and this change edits no workflow file.

## Task 3 — DROPPED (deferred to #634)

The per-endpoint rewrite-hook registry, dispatcher, documentation, and self-tests in
`tests/functional/redhat-shape-proxy.py` are **not** part of this change. An operator scope cap
moved acceptance criterion 3 to its own entry, [#634](https://github.com/randomparity/bzr/issues/634),
which carries the criterion verbatim plus the two constraints this design established: the
`metadata-sort-keys shaped route={field|product} count=<n>` marker stays byte-identical because
`tests/functional/phases/03-products.sh` counts it, and the explicit `sys.stderr.flush()` after each
marker write is load-bearing because that phase reads the log file mid-run.

`tests/functional/redhat-shape-proxy.py` is untouched by this change, and `Makefile` gains no
`check-proxy-self-test` target.

## Task 4 — the enabled non-member user fixture

**Modifies:** `tests/functional/lib.sh`, `tests/functional/phases/07-groups.sh`.
**Tests:** the new `07-groups/fixture-enabled-non-member-user` functional test.

**Interfaces.**

- Consumes: `run_bzr`, `test_fail`, `assert_json`, `$BZR_EXIT`, `$BZR_STDERR` — all defined in
  `tests/functional/lib.sh` (`run_bzr` at `:168`, `test_fail` at `:94`, `assert_json` at `:212`).
- Provides, for issue #625 and any later phase:
  - `NONMEMBER_EMAIL` — string global, `functest-nonmember@test.bzr`.
  - `ensure_enabled_nonmember_user` — no arguments; returns 0 on success, 1 with a diagnostic on
    stderr otherwise. Overwrites the `BZR_*` capture globals.
  - `assert_user_login_enabled <login>` — returns 0 when the server reports `can_login` true for
    that exact login; otherwise calls `test_fail` and returns 1. Overwrites the `BZR_*` globals.
  - `assert_user_group_membership <login> <group> <in|out>` — returns 0 when `<login>`'s own
    `groups` array does (`in`) or does not (`out`) contain `<group>`; otherwise calls `test_fail`
    and returns 1. Overwrites the `BZR_*` globals. This is the helper #625 consumes to assert the
    group filter, so its name and three-argument shape are the interface, not an internal detail.

**Tests, restated:** two new functional tests —
`07-groups/fixture-enabled-non-member-user` and `07-groups/fixture-non-member-is-not-in-the-group`.

### Steps

1. In `tests/functional/lib.sh`, immediately after the `run_bugzilla_sql_file` function (it ends
   with the `"$runtime" exec -i "$container" mysql -u root bugs <"$sql_file"` line and its closing
   brace) and before the `# ── TLS fixture (issue #406) ──` banner, insert:

```bash
# ── Group non-member fixture (issue #617) ────────────────────────────
# A second, *enabled* user who is not a member of the functional test group.
# Asserting that `group list-users --group <g>` honors its filter needs one: an
# added member appears in the listing whether or not the filter is applied, and
# the absence half proves nothing against a user the server would have hidden
# anyway. The login deliberately shares no substring with `testuser`, so the
# existing `user search testuser` and `assert_stdout_not_contains
# "testuser@test.bzr"` assertions cannot match it.
NONMEMBER_EMAIL="functest-nonmember@test.bzr"

# ensure_enabled_nonmember_user — create $NONMEMBER_EMAIL if absent and make
# sure it can log in. Idempotent in both halves: an "already exists" create is
# success, and the enable runs unconditionally so a prior run that disabled the
# user is repaired. The password is explicit because omitting it makes the
# server generate one and mail it, and this harness configures no mail path.
# Overwrites the BZR_* capture globals. Returns non-zero with a diagnostic on
# stderr when either half fails.
#
# This establishes *exists* and *enabled*, not *not a member* — that half holds
# only because nothing adds this login to a group, and containers are reused
# between runs (setup-bugzilla.sh reuses one per checkout+version), so group
# membership survives. A phase that adds $NONMEMBER_EMAIL to a group must
# remove it, or it breaks the next run's non-membership assertion.
ensure_enabled_nonmember_user() {
    run_bzr user create --email "$NONMEMBER_EMAIL" \
        --full-name "Enabled Non-Member" --password "TestPass1!"
    if [[ $BZR_EXIT -ne 0 ]] && ! grep -q "already" "$BZR_STDERR" 2>/dev/null; then
        echo "ensure_enabled_nonmember_user: create failed (exit $BZR_EXIT):" \
            "$(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 1
    fi
    run_bzr user update "$NONMEMBER_EMAIL" --disable-login false --login-denied-text ""
    if [[ $BZR_EXIT -ne 0 ]]; then
        echo "ensure_enabled_nonmember_user: enable failed (exit $BZR_EXIT):" \
            "$(tail -1 "$BZR_STDERR" 2>/dev/null)" >&2
        return 1
    fi
    return 0
}

# assert_user_login_enabled <login> — fail the current test unless the server
# reports can_login true for exactly <login>. This is the half a dependent
# assertion rests on: a fixture that silently degraded to disabled would make an
# absence assertion pass for the same wrong reason the current one does.
# Overwrites the BZR_* capture globals, so call it before capturing output you
# still need.
assert_user_login_enabled() {
    local login="$1"
    run_bzr user search "$login" --details
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "user search '$login' --details exited $BZR_EXIT"
        return 1
    fi
    assert_json "[.[] | select(.name == \"$login\")][0].can_login" "true"
}

# assert_user_group_membership <login> <group> <in|out> — assert that <login>'s
# own membership set does or does not contain <group>, read from the `groups`
# field `user search --details` already returns (USER_FIELDS_DETAILED,
# src/client/mod.rs:22). This reads the *user* resource, not
# `group list-users`, so it is independent of the group filter #625 owns.
#
# An empty `groups` array would make an `out` assertion pass for the wrong
# reason, so callers pair it with an `in` assertion on a user known to be a
# member: that positive control is what proves the harness can see membership
# at all. Overwrites the BZR_* capture globals.
assert_user_group_membership() {
    local login="$1"
    local group="$2"
    local want="$3"
    local expected=0
    [[ "$want" == "in" ]] && expected=1
    run_bzr user search "$login" --details
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "user search '$login' --details exited $BZR_EXIT"
        return 1
    fi
    assert_json \
        "[[.[] | select(.name == \"$login\")][0].groups[]? | select(.name == \"$group\")] | length" \
        "$expected"
}
```

2. In `tests/functional/phases/07-groups.sh`, immediately after the
   `user-re-enable-for-group-tests` test block (it ends with
   `if assert_success; then test_pass; fi` followed by a blank line) and before
   `test_begin "group-add-user"`, insert:

```bash
# The enabled non-member fixture (issue #617). It exists so #625 can assert that
# `group list-users --group functest-grp` excludes a user the server would
# otherwise return. That assertion is red until #625 lands the group-filter fix,
# so this phase provisions and validates the fixture and stops there.
# Invariant: nothing may leave $NONMEMBER_EMAIL in a group. Containers are
# reused between runs, so a membership added here survives into the next one.
test_begin "fixture-enabled-non-member-user" "fixture enabled non-member user"
if ! ensure_enabled_nonmember_user; then
    test_fail "could not provision the enabled non-member fixture user"
elif assert_user_login_enabled "$NONMEMBER_EMAIL"; then
    test_pass
fi
```

2a. Assert the fixture's defining property, with a positive control. Insert this **immediately
    after the existing `group-add-user` test block** (so `testuser@test.bzr` is a known member).

    Steps 2a and 3 both add text between `group-add-user` and `group-list-users`, so fix the
    order once: `group-add-user` → this new test → step 3's `TODO(#625)` comment →
    `test_begin "group-list-users"`. Anchoring both on "before `group-list-users`" would let the
    marker land above this new test instead of above the assertions it indicts.

```bash
# The fixture's non-membership is what #625's assertion will rest on, so assert
# it rather than trusting that nothing added the user to a group — containers
# are reused across runs, so a stray membership persists indefinitely. The
# testuser half is the positive control: it proves the harness can see
# membership at all, so the nonmember half cannot pass on an empty `groups`.
# Both read the user resource, not `group list-users`, so neither depends on
# the group filter #625 owns.
test_begin "fixture-non-member-is-not-in-the-group" "fixture non-member is not in the group"
if assert_user_group_membership "testuser@test.bzr" functest-grp in &&
    assert_user_group_membership "$NONMEMBER_EMAIL" functest-grp out; then
    test_pass
fi
```

    **Verify this against a live container before relying on it.** Bugzilla returns `groups` on
    `User.get` only to a caller permitted to see them; the harness runs as `admin@test.bzr`, which
    holds `editusers`, so it should be populated.

    If the positive control fails — `testuser` shows an empty `groups` — **diagnose before
    deleting anything**, because two causes produce the identical symptom and only one is a
    harness limitation. Run `run_bzr user search admin@test.bzr --details` and inspect `.groups`:
    a non-empty array on any user proves the field *is* visible to this credential, which means
    `group add-user` never created the membership. That is a product defect, not a harness gap —
    stop and report it. Note that this assertion is the first evidence in the repository that
    `group add-user` does anything: `07-groups.sh:87` asserts only that `testuser` appears in a
    listing, which by this change's own `TODO(#625)` reasoning holds whether or not the group
    filter is honored.

    Only when `.groups` is empty for every user does the field genuinely fail to reach this
    credential. Then delete this test, restore the comment-only invariant, and record in this plan
    and in the spec which arm withheld it and what would prove non-membership instead. Do **not**
    keep the `out` half alone: without the control it passes on an empty array, which is the
    pass-for-the-wrong-reason failure this whole epic exists to remove.

3. In the same file, immediately before `test_begin "group-list-users"` — and **below** the test
   step 2a inserted — insert:

```bash
# TODO(#625): these list-users assertions pass whether or not the group filter is
# honored. An added member appears in an unfiltered listing too, and the absence
# assertion below is only reached after the user is disabled, which hides it from
# user search regardless. #625 owns the `groups=` fix and the replacement
# assertion, which uses the enabled $NONMEMBER_EMAIL fixture above.
```

4. In the same file, extend the existing comment above the re-disable line (currently `:106-107`,
   `# Re-disable testuser so it's excluded from list-users results ...`) by appending one line to
   that comment block:

```bash
# TODO(#625): this re-disable is what makes the absence assertion below pass; it
# is not evidence that `group remove-user` worked.
```

5. Verify the test ID guard: `make check-functional-test-ids`. Expect exit 0. It is not silent —
   it prints three success banners ending `functional test semantic IDs are valid`.

6. Verify shell lint: `make check-shell`. Expect exit 0. It echoes its six recipe lines (none is
   `@`-prefixed); exit status is the signal, not silence.

7. Verify the phase against a live container:
   `unset BZR_BIN && cargo build --release && make functional-test-bz50`. Expect
   `TEST  [07-groups/fixture-enabled-non-member-user] fixture enabled non-member user ... PASS`
   and a final `FAILED: 0`.

8. Controlled fault: in step 1's helper, replace the **whole flag pair**
   `--disable-login false --login-denied-text ""` with
   `--disable-login true --login-denied-text "fault disabled"` — the idiom `06-users.sh:46`
   already uses to disable a user. Re-run `make functional-test-bz50` and expect
   `07-groups/fixture-enabled-non-member-user` to FAIL with
   `jq '[.[] | select(.name == "functest-nonmember@test.bzr")][0].can_login' = 'false', expected 'true'`.
   Restore the pair, confirm with `git diff tests/functional/lib.sh`, re-run, expect PASS. Record
   both observations for the pull-request body.

   Replace the pair, not just the boolean. `resolve_login_denied_text`
   (`src/commands/user/update.rs:95-105`) maps `(Some(true), Some(text))` to that text, and an
   empty `text` is byte-identical to what `(Some(false), _)` sends — so
   `--disable-login true --login-denied-text ""` **re-enables** the user and the fault is inert,
   costing a full container arm to observe PASS where a FAIL was promised — a fault that does not
   fault.

9. Commit: `test(functional): add an enabled non-member group fixture user`.

### Acceptance criteria

- `07-groups/fixture-enabled-non-member-user` passes on bz50, bz52, and bz53.
- `07-groups/fixture-non-member-is-not-in-the-group` passes on all three arms, **with its
  positive control passing** — or the test is removed and step 2a's fallback is recorded.
- No existing assertion in `07-groups.sh` changed — only insertions.
- `make check-functional-test-ids` and `make check-shell` are clean.

## Task 5 — the controlled-fault procedure in CONTRIBUTING

**Modifies:** `CONTRIBUTING.md`. **Tests:** none executable; reviewed against the commands it
names, each of which must exist.

**Interfaces.** Consumes the target name Task 1 adds (`make functional-test-bz50`). Write this
task after it so every command it names exists. It must **not** name `make check-proxy-self-test`:
that target was Task 2's, which is dropped, so naming it would put a phantom command into
`CONTRIBUTING.md`. The proxy's own `--self-test` entry point is on `main` already and is named
directly.

### Steps

1. **Do not touch `## Development setup`.** An earlier draft added `python3` to it, to provision a
   `make lint` prerequisite that Task 2 no longer creates. `make lint` gains no new requirement,
   so the setup paragraph stays as it is.

2. In `CONTRIBUTING.md`, insert the new subsection at the **end** of `## Verification`:
   immediately after the `Documentation-only changes should also confirm...` paragraph
   (`CONTRIBUTING.md:69`) and immediately before the `## Pull requests` heading (`:72`).

   Not earlier. Placing an `### Controlled-fault verification` heading before that paragraph
   re-parents it under the new subsection, so general link-and-command-existence guidance would
   read as a step of demonstrating a test goes red.

   Insert everything between the four-backtick fences below (the inner three-backtick `bash`
   block is part of the insert):

````markdown
### Controlled-fault verification

A test that passes both before and after a fix has proved nothing about the fix. When a change
corrects a defect, demonstrate the test goes red against the pre-fix code and green after, and
record both observations in the pull-request body.

1. Write or strengthen the test first.
2. Remove the fix from the working tree — `git stash push` the source paths, or invert the one
   line under test. Do not weaken the test.
3. Run the narrowest command that covers it:
   - a unit test: `make test-one T=<name-substring>`;
   - a production-shape proxy rewrite: `python3 tests/functional/redhat-shape-proxy.py --self-test`;
   - a single functional arm: `make functional-test-bz50`, `make functional-test-bz52`,
     `make functional-test-bz53`, or `make functional-test` for the unpinned default.
4. Observe the failure. Record the exact command and the failing assertion.
5. Restore the fix, confirm the tree really is restored (`git stash list`, `git status`), re-run
   the same command, and observe green.
6. Put both observations in the pull-request body.

**A functional arm needs a fresh binary and a fresh container.** Two things can make the run
report on something other than your fault:

- **A stale binary.** `tests/functional/phases/00-build.sh:16` uses `$BZR_BIN` verbatim when it is
  set and executable, so an exported `BZR_BIN` runs the whole arm against a binary that never
  received your fault. A *failed* build is not the hazard: `run-tests.sh` runs under
  `set -euo pipefail`, so a non-zero `cargo build` aborts the run rather than falling through to
  a stale artifact.
- **A stale container.** `tests/functional/setup-bugzilla.sh` reuses an already-running container
  for this checkout and version, so users, groups, and bugs from earlier runs persist. Residue
  can satisfy the assertion under test in the faulted state, or fail it in the restored state.

So run the functional arm as one gated chain, before and after removing the fault:

```bash
unset BZR_BIN
BZR_BZ_VERSION=bz50 tests/functional/setup-bugzilla.sh reset \
  && cargo build --release \
  && make functional-test-bz50
```

Chain the commands rather than pasting them as separate lines, so a failed reset or build stops
before the arm runs instead of testing the previous state.
````

3. Verify every command the section names exists:
   `make -n test-one T=x`, `make -n functional-test-bz50`, `make -n functional-test-bz52`,
   `make -n functional-test-bz53`, `make -n functional-test`. Each must exit 0.
   (`make -n test-one` without `T=` errors by design; pass `T=x`.) Also verify the proxy entry
   point the section names actually runs:
   `python3 tests/functional/redhat-shape-proxy.py --self-test`, expect exit 0 ending `OK`.

4. Verify the relative link target still resolves: the section adds no new links, so
   `rg -n '\]\(' CONTRIBUTING.md` should show the same link set as before the edit.

5. Commit: `docs: record the controlled-fault verification procedure`.

### Acceptance criteria

- Every command named in the section exists and `make -n` accepts it.
- The section sits under `## Verification`, per accepted ADR 0021.

## Task 6 — the `TODO(#N)` fixture inventory

**Modifies:** `src/commands/bug/clone_tests.rs`, `src/client/resources/group_tests.rs`,
`src/client/resources/server_tests.rs`, `src/xmlrpc/resources/mappers_tests.rs`,
`tests/functional/phases/02-server-auth.sh`. **Tests:** `make test`, `make lint`.

**Interfaces.** Consumes nothing, provides nothing. Comment lines only: **no asserted value
changes.**

### Steps

1. `src/commands/bug/clone_tests.rs` — above **both** occurrences of
   `                "rep_platform": "x86_64",` (at `:91` and `:329`), insert:

```rust
                // TODO(#621): the server emits this key as `platform`; this
                // fixture mirrors the client's own misconception, which is why
                // the test passes. #621 owns the rename on read and write.
```

   Do not touch `:62` (`rep_platform: None`, a Rust field name) or `:383`
   (`parsed["changes"]["rep_platform"]`, the update payload).

2. `src/client/resources/group_tests.rs` — above each of the four
   `        .and(query_param("group", ...))` lines (`:16`, `:52`, `:87`, `:106`), insert:

```rust
        // TODO(#625): Bugzilla ignores an unrecognized `group` param; #625 switches to `groups`.
```

   That line is 97 characters including its indentation, within the 100-character limit.

3. `src/client/resources/server_tests.rs` — above both
   `            "parameters": {"maxattachmentsize": 1000}` lines (`:93`, `:149`), insert:

```rust
            // TODO(#626): every stock server stringifies /parameters values; #626 owns the fix.
```

4. `src/xmlrpc/resources/mappers_tests.rs` — above
   `    m.insert("dt".into(), Value::DateTime("2024-01-01T00:00:00".into()));` (`:60`), insert:

```rust
    // TODO(#622): XMLRPC.pm strips the dashes (20240101T00:00:00); #622 owns the fix.
```

5. `tests/functional/phases/02-server-auth.sh` — immediately before
   `test_begin "server-capabilities"` (`:15`), insert:

```bash
# TODO(#626): this credentialed assertion never checks max_attachment_size, so a
# permanently null value has always passed here. The credentialless `null`
# assertion further down is correct under accepted ADR 0005 and stays; #626 owns
# adding the non-null credentialed case.
```

6. Verify formatting is untouched by the comments: `cargo fmt --check`. Expect no output, exit 0.
   If rustfmt reflows a comment, accept its output and re-run.

7. Verify no behavior changed: `make test`. Expect the same pass count as before the edit and
   exit 0.

8. Verify the shell edit: `make check-shell` and `make check-functional-test-ids`. Expect exit 0
   from each; both print success banners rather than staying silent.

9. Verify no asserted value moved. Filter on **comment syntax**, not on the marker token — the
   markers are multi-line and only their first line carries `TODO(#`, so a token filter reports
   every continuation line as a violation on correct work:

   ```bash
   git diff -U0 src/ tests/functional/phases/02-server-auth.sh \
     | rg '^[+-]' | rg -v '^[+-][+-]' | rg -v '^[+-]\s*(//|#)'
   ```

   must print nothing — every changed line is a comment. Then confirm the markers are actually
   present: `rg -c 'TODO\(#' src/commands/bug/clone_tests.rs src/client/resources/group_tests.rs
   src/client/resources/server_tests.rs src/xmlrpc/resources/mappers_tests.rs
   tests/functional/phases/02-server-auth.sh` should report 2, 4, 2, 1, and 1.

10. Commit: `test: mark fixtures that encode the conformance defects they hide`.

### Acceptance criteria

- This task lands ten markers: two `#621` (`clone_tests.rs`), four `#625` (`group_tests.rs`),
  two `#626` (`server_tests.rs`), one `#622` (`mappers_tests.rs`), and one `#626`
  (`02-server-auth.sh`). Task 4 lands the two `#625` markers in `07-groups.sh`, bringing the
  change to twelve and covering all six sites issue #617 names.
- `git diff` on `src/` shows only added comment lines.
- `make test` and `make lint` are green.

## Final verification

Run in this order, each bare (no pipe), after all six tasks:

1. `make lint` — expect exit 0. Its prerequisite list is unchanged by this task set.
1a. `python3 tests/functional/redhat-shape-proxy.py --self-test` — expect exit 0, ending `OK`.
   Not a guardrail this change owns; run it once to confirm the command `CONTRIBUTING.md` now
   names really works, since the proxy file itself is untouched here.
2. `make test` — expect exit 0.
3. `unset BZR_BIN && cargo build --release` — expect exit 0.
4. `make functional-test-bz50` — expect the phase-0 banner to read
   `║  bzr functional tests (bz50)` and the suite to reach `FAILED: 0`. **Read the banner**: it
   is the only observable that distinguishes this target from a recipe mis-copied to
   `BZR_BZ_VERSION=bz52`, which would start the 5.2 container, run the identical phase list, and
   also exit 0. An unknown token needs no check — `setup-bugzilla.sh:23-35` rejects it before any
   container work. This is Task 1's only proof: `run-all-versions.sh:20-40` invokes
   `setup-bugzilla.sh` and `run-tests.sh` directly and never shells out to a Make target, so
   step 5 does not traverse the new recipe.
5. `make functional-test-all` — expect `bz50: PASSED`, `bz52: PASSED`, `bz53: PASSED` and exit 0.
6. `make functional-stop-all` — clean up the three containers.

Record the controlled-fault observation (Task 4 step 8) in the pull-request body. The change adds
no file beyond the file map above — with Tasks 2 and 3 dropped, no workflow file and no proxy file
is touched, so there is nothing to disclose as surface beyond what issue #617 suggests. State
plainly in the body that acceptance criterion 3 is deferred to #634 and not met here.
