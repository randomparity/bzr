# Semantic functional-test IDs implementation plan

## Goal

Replace every global numeric functional-test label with a stable `<phase>/<semantic-slug>`
reference, enforce that contract statically and at runtime, and prove that the migration preserves
the executed test sequence across every supported Bugzilla version.

## Architecture

`run-tests.sh` remains the phase orchestrator and assigns the current phase immediately before its
canonical `source` line. `lib.sh::test_begin` combines that runner-owned group with a literal slug,
validates the full runtime identity, and prints it separately from the unchanged description. A
dependency-free Bash guard validates runner/file correspondence and every phase call site before
merge; private pre/post functional transcripts prove migration equivalence.

## Tech stack

- GNU Bash 3.2-compatible runtime and guard code
- POSIX `awk`, `sed`, `sort`, `uniq`, and `basename`; existing `rg` guard prerequisite
- GNU Make and GitHub Actions
- Existing Docker/Podman-backed functional harness

## Global constraints

- Full ID: `<phase>/<slug>`.
- Phase regex, transcribed from the spec:
  `[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*`.
- Slug regex: `[a-z0-9]+(-[a-z0-9]+)*`; slugs are literal and contain no shell expansion.
- `test_begin` accepts exactly two arguments: slug and unchanged description.
- Phase files never reference `CURRENT_TEST_GROUP`.
- The runner contains exactly one adjacent canonical assignment/source pair using `_phase`:

  ```bash
  CURRENT_TEST_GROUP="$_phase"
  source "$SCRIPT_DIR/phases/${_phase}.sh"
  ```

- Existing phase order, runtime test order, descriptions, assertions, pass/skip behavior, and
  compiled `bzr` behavior remain unchanged.
- The baseline has 416 source call sites. Six mode-dependent declarations become twelve explicit
  literal-ID branches while retaining one runtime test per mode.
- No dependency or toolchain floor is added. Rust remains at the repository MSRV 1.89.0.
- Host architecture is arm64; no target architectures are declared by effective repository
  instructions; relationship is `no-target-declared`. The change is architecture-insensitive.
- Required guardrails: `make check-functional-test-ids`, `make check-shell`, `make lint`,
  `make test`, and `make functional-test-all`.

## File map

- Create `tools/check-functional-test-ids.sh`: source and runner contract guard.
- Create `tools/check-functional-test-ids-tests.sh`: isolated static fixtures and runtime helper
  tests against the real `tests/functional/lib.sh`.
- Modify `Makefile`: target, lint dependency, `.PHONY`, and installed pre-commit hook.
- Modify `.github/workflows/ci.yml`: individually gated semantic-ID check.
- Modify `tests/functional/lib.sh`: group state, seen-ID state, and two-argument `test_begin`.
- Modify `tests/functional/run-tests.sh`: initialize the group and bind it to each source action.
- Modify all 36 phase files containing `test_begin`: replace 416 numeric labels and unroll six
  mode-dependent ID declarations into literal branches.
- Modify `tests/functional/README.md`: authoring and output contract.

## Task 1: Capture the pre-migration live oracle

### Interfaces

- Produces private files `<workspace>/functional-before.log` and
  `<workspace>/functional-before.normalized` for Task 5.
- Consumes the unchanged harness at the current design-only branch HEAD.
- No repository file changes or commit.

### Steps

1. Resolve the Forge workspace and create both private files there with mode `0600`.
2. Run the all-version suite bare through `tee`, preserving the producer exit code:

   ```bash
   set -o pipefail
   make functional-test-all 2>&1 | tee "$WORKSPACE/functional-before.log"
   ```

   Expected: exit 0 and a green summary for every supported Bugzilla version.
3. Normalize completed tests to one description/outcome record and retain per-version summaries:

   ```bash
   awk '
     function trim_cr(value) { sub(/\r$/, "", value); return value }
     function emit_test(value) { print "TEST " value; test_count++ }
     /^[[:space:]]*TEST[[:space:]]/ {
       if (pending != "") exit 2
       line = trim_cr($0)
       sub(/^[[:space:]]*TEST[[:space:]]+/, "", line)
       sub(/^([0-9]+[a-z0-9-]*\. |\[[^]]+\] )/, "", line)
       if (line ~ /\.\.\. (PASS|FAIL|SKIP)(  \(.*\))?$/) {
         emit_test(line)
         next
       }
       marker = index(line, " ... ")
       if (marker == 0) exit 2
       pending = substr(line, 1, marker + 4)
       next
     }
     pending != "" {
       outcome = trim_cr($0)
       sub(/^[[:space:]]*/, "", outcome)
       if (outcome ~ /^(PASS|FAIL|SKIP)(  \(.*\))?$/) {
         emit_test(pending outcome)
         pending = ""
         next
       }
     }
     /^── Phase 17: Cleanup \(/ || /PASSED:/ {
       print trim_cr($0)
     }
     /^[[:space:]]*TOTAL:/ {
       line = trim_cr($0)
       total = line
       sub(/^[[:space:]]*TOTAL:[[:space:]]*/, "", total)
       if (total !~ /^[0-9]+$/ || test_count != total + 0) exit 3
       print line
       test_count = 0
       summaries++
     }
     END { if (pending != "" || test_count != 0 || summaries == 0) exit 2 }
   ' "$WORKSPACE/functional-before.log" >"$WORKSPACE/functional-before.normalized"
   chmod 600 "$WORKSPACE/functional-before.normalized"
   ```

   Expected: one ordered normalized record containing descriptions/outcomes and summaries for all
   versions. Live timestamps, warning timestamps, and observed fixture counts printed between a
   test start and its outcome are excluded. A second test start before the pending outcome is an
   error, and each version's normalized test count must equal its `TOTAL`. Record the normalized
   file's SHA-256 in the Forge ledger.

### Acceptance criteria

- Both transcript files are regular private files outside the repository.
- The all-version command exited 0; no baseline is created from a red run.
- The normalized file is non-empty and includes each version cleanup heading.
- A constructed transcript with consecutive split-form test starts is rejected.

## Task 2: Add the static semantic-ID guard with tests

### Interfaces

- `tools/check-functional-test-ids.sh <repository-root>` returns 0 only when runner, phase files,
  and call sites satisfy ADR 0029.
- `tools/check-functional-test-ids-tests.sh` initially invokes the guard against isolated fixtures;
  Task 3 extends it with tests against the real runtime helper.
- Later tasks rely on `make check-functional-test-ids` as their focused test.

### Steps

1. Create `tools/check-functional-test-ids-tests.sh` first. Its fixture helper creates a temporary
   repository containing `tests/functional/run-tests.sh` and `phases/*.sh`; each case invokes the
   checker and asserts the expected status and diagnostic. Add cases for:

   - valid column-zero and indented literal calls;
   - valid explicit `rest` and `xmlrpc` literal branches;
   - invalid and duplicate phase basenames;
   - runner/file set mismatch;
   - swapped or alternate source variable;
   - an intervening command between assignment and source;
   - direct and derived `CURRENT_TEST_GROUP` access in a phase;
   - legacy numeric, malformed, variable-bearing, duplicate, missing-description, and extra-arg
     calls, at both column zero and indentation;
   - noncanonical occurrences such as `false || test_begin ...` and
     `if false; then test_begin ...; fi`, proving shell syntax cannot bypass the inventory.

   The canonical valid runner fixture is:

   ```bash
   for _phase in \
     01-config 08-bugs; do
       CURRENT_TEST_GROUP="$_phase"
       source "$SCRIPT_DIR/phases/${_phase}.sh"
   done
   ```

2. Run `bash tools/check-functional-test-ids-tests.sh`.
   Expected: nonzero with `check-functional-test-ids.sh: No such file or directory`, proving red.
3. Create `tools/check-functional-test-ids.sh` with `set -euo pipefail` and `LC_ALL=C`. Implement:

   ```bash
   phase_re='^[0-9]{2}[a-z]?-[a-z0-9]+(-[a-z0-9]+)*$'
   call_re='^[[:space:]]*test_begin[[:space:]]+"([a-z0-9]+(-[a-z0-9]+)*)"[[:space:]]+"[^"]*"[[:space:]]*$'
   ```

   - enumerate phase basenames from `tests/functional/phases/*.sh` and validate `phase_re`;
   - extract the `_phase` list only from the runner's `for _phase in \` through `; do`, reject
     duplicates, and compare its C-sorted value set byte-for-byte with disk;
   - use one `awk` pass to require exactly one assignment, one source line, and one adjacent pair
     matching the canonical lines in Global constraints;
   - run `rg -n 'CURRENT_TEST_GROUP'` over phase files and distinguish match (contract error), exit
     1 (no match), and every other exit (tool failure);
   - inventory every textual `test_begin` occurrence in each phase file, distinguish no match from
     tool failure, and require every occurrence to be consumed by the canonical complete-line
     validator; reject occurrences embedded in other shell syntax instead of silently omitting
     them;
   - validate each consumed complete line against `call_re`, then maintain a newline-delimited
     `<phase>/<slug>` set to reject duplicate literal IDs without associative arrays;
   - accumulate actionable `file:line` errors and return 1 if any occurred.

4. Re-run `bash tools/check-functional-test-ids-tests.sh`.
   Expected: fixture tests for the new guard pass; the real-tree positive check is not enabled yet
   because migration has not happened.
5. Run the fixture command again, then commit only the inactive checker and its tests. Wiring waits
   until Task 4 because activating the guard against legacy one-argument call sites would make this
   intermediate commit red:

   ```bash
   git add tools/check-functional-test-ids.sh tools/check-functional-test-ids-tests.sh
   git commit -m "test(functional): enforce semantic test IDs"
   ```

### Acceptance criteria

- New tests were observed red before the checker existed and green afterward.
- Every rejection reports its phase file or runner concern and the required form.
- Scripts run under host Bash 3.2 and add no dependency.
- The fixture suite is green while the checker remains deliberately unwired until its callers are
  migrated.

## Task 3: Compose and validate runtime IDs

### Interfaces

- `test_begin <literal-slug> <description>` prints `[<group>/<slug>] <description> ...`.
- `CURRENT_TEST_GROUP` is initialized by `lib.sh` and assigned by `run-tests.sh`.
- `SEEN_TEST_IDS` is private newline-delimited state owned by `lib.sh`.

### Steps

1. Extend `tools/check-functional-test-ids-tests.sh` with a `--runtime-only` mode and subprocess
   cases that source the real `tests/functional/lib.sh`. Assert:

   - `CURRENT_TEST_GROUP=08-bugs; test_begin create-first-bug 'bug create (bug one)'` prints the
     exact reference and description;
   - a second distinct slug succeeds;
   - missing group, one or three arguments, invalid group examples `08--bugs`, `08_Bugs`, and
     `bugs-08`, malformed slug examples, and a repeated full ID return nonzero with the offending
     value in stderr;
   - for each rejected missing/invalid group, arity, and malformed-slug call, seed both
     `CURRENT_TEST` and `SEEN_TEST_IDS`, assert both remain byte-for-byte unchanged, then retry the
     same slug with valid inputs and prove it succeeds;
   - for duplicate-ID rejection, assert both state values remain exactly at their already-valid
     pre-call values, then prove a distinct valid ID can still be added.

2. Run the runtime-only test mode.
   Expected: nonzero because the existing helper accepts one argument and prints the legacy text.
3. In `tests/functional/lib.sh`, initialize:

   ```bash
   CURRENT_TEST_GROUP=""
   SEEN_TEST_IDS=$'\n'
   ```

   Replace `test_begin` with a Bash 3.2-compatible implementation that checks arity before reading
   positional arguments, validates the exact phase and slug regexes, composes
   `test_id="$CURRENT_TEST_GROUP/$slug"`, detects `$'\n'$test_id$'\n'` in `SEEN_TEST_IDS`, appends
   the new ID, sets `CURRENT_TEST="$description"`, and prints:

   ```bash
   printf "  ${CYAN}TEST${RESET}  [%s] %s ... " "$test_id" "$CURRENT_TEST"
   ```

   All validation failures use `printf ... >&2` and return 2 before changing state.
4. In `tests/functional/run-tests.sh`, add `CURRENT_TEST_GROUP=""` to shared initialization and put
   the canonical assignment directly before the existing source line.
5. Run the runtime-only tests again.
   Expected: all runtime cases pass. Do not commit yet: the two-argument interface and its legacy
   phase callers are one atomic change, completed in Task 4.

### Acceptance criteria

- Tests prove every reachable validation branch and exact output shape.
- A rejected call does not enter the seen set or change `CURRENT_TEST`.
- No associative arrays or post-Bash-3.2 features are used.

## Task 4: Migrate every phase call site

### Interfaces

- Consumes the two-argument `test_begin` interface from Task 3.
- Produces only literal slug arguments; descriptions remain unchanged after removing their numeric
  prefix.
- The static guard from Task 2 is the migration completeness check.

### Steps

1. Mechanically rewrite non-variable descriptions in all phase files. For each line matching
   `^([[:space:]]*)test_begin "[0-9]+[a-z0-9-]*\. ([^"]*)"$`, retain indentation and description,
   generate a candidate slug by lowercasing, removing `#<digits>`, changing flag prefixes and every
   remaining non-alphanumeric run to one hyphen, and trimming edge hyphens. Emit
   `test_begin "<slug>" "<unchanged-description>"`. Skip the six lines containing a mode variable.
2. Review every generated slug in the diff. Shorten fixture-specific wording where it does not
   distinguish behavior, remove issue numbers and transient values, and keep command/behavior terms
   that make the reference searchable. Do not regenerate IDs from descriptions at runtime.
3. Replace the two mutually exclusive TLS fixture declarations with distinct literal slugs:
   `https-fixture-tools-unavailable` and `https-fixture-start-failed`.
4. Give the three dependency-analysis proxy-unavailable declarations distinct fallback slugs:
   `production-policy-proxy-start-failed`, `scoped-proof-skipped-proxy-unavailable`, and
   `api-failure-classification-skipped-proxy-unavailable`.
5. Replace each of the five `_DA_ADJ_MODE` declarations and the one `_RESTRICTED_MODE` declaration
   with a `case` immediately before the shared test body. Each `rest` and `xmlrpc` arm calls
   `test_begin` with an explicit mode-prefixed literal slug and the original interpolated
   description; an unexpected mode prints an error and returns nonzero. This produces twelve source
   declarations but preserves one runtime declaration per loop iteration.
6. Update `tests/functional/README.md` in the same atomic migration with the `<phase>/<slug>` output
   example, exact slug rules, literal-only and uniqueness requirements, description/ID
   independence, and insertion guidance.
7. Wire the target in `Makefile`:

   ```make
   check-functional-test-ids:
	   bash tools/check-functional-test-ids-tests.sh
	   bash tools/check-functional-test-ids.sh .
   ```

   Add it to `.PHONY`, `lint`, and the installed pre-commit body. Extend `check-shell` with
   `shellcheck -s bash tests/functional/lib.sh tests/functional/run-tests.sh
   tests/functional/phases/*.sh` and
   `bash -n tests/functional/lib.sh tests/functional/run-tests.sh
   tests/functional/phases/*.sh`; retain the established shfmt scope because the functional
   harness uses its own four-space style. Fix the three pre-existing ShellCheck findings exposed
   by this newly activated coverage without changing behavior. Add
   `make check-functional-test-ids` as its own step in CI's `test-layout` job, which already
   installs `rg`. Then run `make check-functional-test-ids`. Expected: exit 0, no legacy labels,
   no slug expansions, no duplicate full IDs, 37 matching phase groups, and exact runner/source
   binding.
8. Inspect the complete runtime, phase, and README diff and confirm descriptions, assertion bodies,
   and execution order are unchanged. Run `make check-functional-test-ids`, `make check-shell`,
   `make lint`, and `make test`. Do not commit yet: Task 5 first proves the complete working tree
   against the live pre-migration transcript.

   After Task 5's comparison passes, commit the atomic interface migration:

   ```bash
   git add tests/functional/lib.sh tests/functional/run-tests.sh tests/functional/phases/*.sh \
     tests/functional/README.md tools/check-functional-test-ids-tests.sh Makefile \
     .github/workflows/ci.yml
   git commit -m "test(functional): adopt semantic test references"
   ```

### Acceptance criteria

- All 416 baseline declarations are migrated; the only source-count increase is the six explicit
  mode branches.
- Every ID is literal, unique within its phase, semantic, and independent of source order.
- Functional descriptions, test bodies, and execution order are unchanged.
- The runtime interface, callers, guard wiring, shell coverage, and authoring contract land in one
  revertible commit after the live proof is green.

## Task 5: Prove and commit the migration

### Interfaces

- Consumes Task 1's private normalized baseline and the completed harness.
- Produces the final guardrail evidence and no committed transcript.

### Steps

1. Run focused/local gates:

   ```bash
   make check-functional-test-ids
   make check-shell
   make lint
   make test
   ```

   Expected: each exits 0 with no warning.
2. Run the post-migration all-version suite with the same host and capture method as Task 1:

   ```bash
   set -o pipefail
   make functional-test-all 2>&1 | tee "$WORKSPACE/functional-after.log"
   ```

   Expected: exit 0 and green summaries for all versions.
3. Apply Task 1's exact `awk` normalizer to `functional-after.log`, write a private mode-0600
   `functional-after.normalized`, then run:

   ```bash
   cmp "$WORKSPACE/functional-before.normalized" "$WORKSPACE/functional-after.normalized"
   ```

   Expected: exit 0. A mismatch is investigated; the baseline is never updated to accept it.
4. Commit the atomic migration using Task 4's exact `git add` and commit command only after the
   normalized comparison is green.
5. Re-read `git diff main...HEAD` for description, assertion, and ordering drift. Run the full local
   gates once more if review changed executable files.

### Acceptance criteria

- All focused, lint, unit/integration, and live all-version checks pass.
- Normalized pre/post transcripts compare byte-for-byte.
- The README gives enough information to add a test without consulting the implementation.
- Private transcripts remain outside Git and are retained only in the Forge workspace/ledger for
  review evidence.

## Rollback

The inactive guard and the atomic runtime/caller/README migration are isolated commits. Before
publication, reverting the atomic migration first restores the old runtime, callers, authoring
contract, Make/CI wiring, and shell coverage; reverting the inactive guard second removes its
scripts. Retain both private transcripts until rollback diagnosis is complete. No persisted data or
external schema is involved. Functional containers may be stopped with `make functional-stop-all`;
private transcripts are disposed through the Quest/Forge artifact lifecycle after review
publication.

## Durable workflow context

- Issue: #602; scope token: `q602-28a4a246`
- Branch: `feat/semantic-functional-test-ids-602`
- Base branch: `main`
- Design: `docs/workflow/specs/2026-08-31-semantic-functional-test-ids-design.md`
- ADR: `docs/adr/0029-semantic-functional-test-ids.md`
- Guardrails: `make check-functional-test-ids`; `make check-shell`; `make lint`; `make test`;
  `make functional-test-all`
- Open findings: none
- Review deferrals/suppressions: none
