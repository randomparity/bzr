# Functional Test Coverage Expansion — Design

**Date:** 2026-06-20
**Status:** Proposed
**Scope:** `tests/functional/` — container-based end-to-end tests against real Bugzilla 5.0/5.2/5.3

## Context

Substantial CLI surface has landed since the `v0.4.4` tag (current `Cargo.toml` is
`0.4.5-dev`), but the functional suite has not kept pace. `git log v0.4.4..HEAD --
tests/functional/` shows **exactly one** commit since the tag — `2f52f79`, the
attachment `--x/--no-x` flag rename — which only updated existing tests.

Every other new command and flag is invisible to the container suite. Unit tests
(wiremock-mocked) were added for these features, but functional tests are the only
layer that proves request shapes actually work against a live Bugzilla across all
three supported versions. The transport-sensitive additions (server-side paging,
`--count` via `limit=0`, optimistic-concurrency collision detection, the new
convenience verbs) are precisely the things mocks cannot validate.

A `grep` of `tests/functional/run-tests.sh` returns **zero** matches for: `bug
resolve|close|reopen|dup`, `completion`, `schema`, `component list|view`,
`attachment view`, `classification list`, `--from-json`, `--sort|--order|--offset|
--paginate|--count`, `--dry-run`, `--expect-unchanged-since`, `bug view --web`,
`--retry|--timeout`, `--output ndjson`, `config remove-server|rename-server`,
`comment add --body-file`, and ad-hoc `--server-url`.

**Goal:** Exhaustive functional coverage of every CLI command, subcommand, and field
— restructured from the current 1,541-line monolith into maintainable per-resource
files.

## Current State

- **Runner:** `tests/functional/run-tests.sh` (1,541 lines, ~108 tests in 16 phases),
  a single sequential script. Phases share state through global variables
  (`PRODUCT_ID`, `BUG1..BUG4`, `COMMENT_ID`, `ATTACH_ID`, …) initialized at the top
  (`run-tests.sh:25-37`) and populated as earlier phases create fixtures.
- **Harness:** `tests/functional/lib.sh` (259 lines) provides the test lifecycle
  (`test_begin/pass/fail/skip`), `run_bzr` / `run_bzr_raw` (the latter omits
  `--json`), version gating (`require_version 520 "reason"`), and the `assert_*`
  family (exit-code, jq-path, stdout/file substring).
- **Containers:** `setup-bugzilla.sh` + `versions/bz5{0,2,3}/` build Fedora+MariaDB+
  Bugzilla images on ports 8089/8090/8091. Seeded with admin `admin@test.bzr`, a
  fixed API key, `insidergroup=admin`, `mail_delivery_method=None`, and (bz52+) a
  `fix-needed` keyword. `run-all-versions.sh` runs all three sequentially.
- **Make targets:** `functional-test`, `functional-test-bz5{2,3}`,
  `functional-test-all`, `functional-test-keyring` (`Makefile:161-190`).

## Coverage Gap Analysis

Legend: ✅ covered · ⚠️ partial · ❌ none.

### Bug
| Surface | Status | Notes |
|---|---|---|
| `bug create` core fields | ✅ | product/component/summary/description, priority/severity/op-sys/platform, blocks/depends-on/dupe-of, description-file/stdin precedence |
| `bug create` metadata fields | ❌ | `--alias --url --whiteboard --target-milestone --deadline --cc --keywords --groups --flag` |
| `bug create --from-json` | ❌ | object form, array (multi-bug) form, stdin `-`, unknown-key rejection (exit 7), CLI-override-JSON precedence |
| `bug list` filters | ⚠️ | status/product/whiteboard/resolution covered; `--target-milestone --version --op-sys --platform --qa-contact --url --assignee --creator --priority --severity --alias --summary` untested; negation `!` only partly exercised |
| `bug list/search/my` paging+sort | ❌ | `--sort --order --offset --paginate --count` |
| `bug search --from-url --save-as` | ❌ | quicksearch covered; URL-derived search untested |
| `bug view --web` | ❌ | headless URL-print path |
| `bug view --fields/--exclude-fields` | ⚠️ | `--fields` covered; `--exclude-fields` untested |
| `bug update` collision guard | ❌ | `--expect-unchanged-since` (exit 14 MidAirCollision) |
| `bug update` list mutations | ⚠️ | blocks/depends-on/keywords/cc add-remove covered; `--groups-add/remove --see-also-add/remove` untested |
| `bug update` time-tracking | ❌ | `--estimated-time --remaining-time --work-time` |
| `bug update --alias` | ❌ | single-bug alias set |
| `bug resolve/close/reopen/dup` verbs | ❌ | all four new convenience verbs, incl. `--as`, comment flags, batch partial-failure (exit 11) |

### Comment
| Surface | Status | Notes |
|---|---|---|
| `comment add --body/--private` | ✅ | |
| `comment add --body-file` / stdin `-` | ❌ | file body, stdin precedence, body/body-file conflict (exit 2) |
| `comment list --since` | ✅ | across rest/hybrid/xmlrpc |
| `comment tag --add/--remove`, `search-tags` | ✅ | |

### Attachment
| Surface | Status | Notes |
|---|---|---|
| `attachment upload` core + `--patch/--private` | ✅ | new `--x/--no-x` grammar |
| `attachment upload --comment/--comment-private/--flag` | ❌ | folded comment + flag-on-attachment |
| `attachment view <id>` | ❌ | metadata-only (no byte download) |
| `attachment list/download` | ✅ | incl. `--bug`, `--out`, mixed positional |
| `attachment update` | ⚠️ | summary/obsolete covered; `--file-name --content-type --flag` and override pairs untested |

### Product / Component / Classification
| Surface | Status | Notes |
|---|---|---|
| `product create/list/view/update` | ✅ | incl. `--type` variants; `--version --is-open` untested on create, `--default-milestone` untested on update |
| `component create/update` | ⚠️ | create covered; update version-gated; `--name --default-assignee` on update untested |
| `component list/view` | ❌ | both new subcommands |
| `classification list` | ❌ | new; only `classification view` covered |

### User / Group / Server / Config
| Surface | Status | Notes |
|---|---|---|
| `user create/search/update` | ✅ | `--login` on create and `--real-name/--email` on update untested |
| `group create/view/update/add-user/remove-user/list-users` | ✅ | |
| `server info`, `whoami` | ✅ | `whoami show` removed (#323) — confirm bare form, assert old form errors |
| `config set-server/show/set-default/keyring` | ✅ | |
| `config remove-server/rename-server` | ❌ | both new; incl. keychain cleanup, default-pointer rewrite |
| `--server-url/--server-api-key-env` (ad-hoc) | ❌ | config-less invocation |
| `--server-email` (ad-hoc fallback) | ⚠️ | only reached when API-key whoami can't return identity; see Open Risks — likely unit-only in this harness |
| `--config <path>` alternate file | ❌ | |

### New top-level commands
| Surface | Status | Notes |
|---|---|---|
| `completion {bash,zsh,fish,powershell,elvish}` | ❌ | non-empty script, shell-appropriate marker |
| `schema [name]` | ❌ | list mode + per-name JSON Schema (valid draft 2020-12) |

### Global flags / output
| Surface | Status | Notes |
|---|---|---|
| `--output table/json` + `--quiet` | ✅ | |
| `--output ndjson` | ❌ | one compact value per line, empty-list → no lines, truncation note to stderr |
| `--dry-run` | ❌ | preview on each mutation verb; exit 7 on non-mutation |
| `--retry <n>` / `--timeout <secs>` | ❌ | happy-path smoke (hard to force transient failures in-container) |
| `-y/--yes` batch confirmation | ⚠️ | large-batch (>10) confirmation path |

## Target Architecture

Restructure the monolith into per-resource phase files sourced by a thin orchestrator,
preserving the existing sequential/shared-fixture model.

```
tests/functional/
  lib.sh                 # unchanged core harness (+ new assertions, below)
  run-tests.sh           # orchestrator: setup, source phases in order, summary
  phases/
    00-build.sh
    01-config.sh         # set-server/show/set-default + remove-server/rename-server
    02-server-auth.sh    # server info, whoami, --server-url ad-hoc, --config
    03-products.sh
    04-components.sh     # create/update/list/view
    05-fields-classifications.sh   # + classification list
    06-users.sh
    07-groups.sh
    08-bugs-create.sh    # all fields + --from-json
    09-bugs-read.sh      # list/search/my/view: filters, paging, sort, --web, ndjson
    10-bugs-update.sh    # update fields, list mutations, time-tracking, collision
    11-bugs-verbs.sh     # resolve/close/reopen/dup (NEW)
    12-bugs-clone.sh
    13-templates.sh
    14-queries.sh
    15-comments.sh       # + --body-file, stdin
    16-attachments.sh    # + view, upload --comment/--flag, update --file-name/--flag
    17-global-options.sh # ndjson, dry-run, retry/timeout, yes
    18-completion-schema.sh  # NEW top-level commands
    99-sequences.sh      # cross-command lifecycle + cross-transport parity
```

**These are ordered segments, not independent modules.** The split is a readability
and ownership win, **not** an isolation win — be honest about that. The suite remains a
single sequential run against one mutating container, and files share state through
globals (`BUG1` created in `08`, updated in `10`, cloned in `12`). A `phases/*.sh` file
**cannot** be run on its own: under the runner's `set -euo pipefail` (`run-tests.sh:4`),
`16-attachments.sh` dereferencing `BUG1` aborts on `set -u` if `08` did not run first.
The split's value is "find the attachment tests fast," not "run them alone."

Given that, the model is:

- Phase files are **sourced** (not subshelled) by `run-tests.sh` in a fixed order, so
  shared globals stay visible. The orchestrator owns the shared-variable declarations
  (`run-tests.sh:25-37`) and the `cleanup`/`trap` (`run-tests.sh:43-49`).
- **Enforced ordering, not documented ordering.** The orchestrator sources each file
  through a wrapper that first asserts the globals that file declares as prerequisites
  are non-empty (`: "${BUG1:?08-bugs-create must run first}"`). A broken source order
  fails loudly at the boundary instead of mid-test with a confusing `set -u` error.
- Each file begins with a header naming the globals it **reads** and **creates**, so the
  dependency chain is auditable and the wrapper's prerequisite list stays in sync.
- New standalone test groups that do **not** need an upstream bug (filter/paging/count
  tests, see the Test-Design Contract) create their own marked fixtures via `make_bug`
  rather than reusing `BUG1`, so they neither depend on nor pollute the shared corpus.
- `run-all-versions.sh`, `setup-bugzilla.sh`, and the `Makefile` targets are unchanged
  (they invoke `run-tests.sh`).

This is a mechanical move of existing passing tests into the matching `phases/*.sh`
file, verified by the per-test diff in Phasing step 1 (not a totals comparison) before
any new tests are added.

## Test-Design Contract (read before writing assertions)

The suite runs **sequentially against one mutating container**, and the bug corpus
grows by an unknown, version-dependent amount as phases run. Two failure modes follow,
and every new test must defend against them:

1. **A test that only checks "the command ran" proves nothing.** A filter that is
   silently broken and returns *all* bugs passes a naive "≥1 result" assertion. Every
   filter test must use **discriminating fixtures**: seed at least one bug that matches
   and one that does not, then assert both the presence of the match **and the absence**
   of the non-match (and the converse for the `!`-negation form). Crucially, the
   presence half must identify the test's **own** fixture, not just "some matching
   row" — the shared corpus already contains many bugs that match a given filter, so
   capture the id returned by `make_bug` and assert that **exact** `.id` is in the
   result array (e.g. `assert_json_exists '.[] | select(.id==<id>)'`), never merely that
   the array is non-empty or that the field value appears somewhere. Likewise the absence
   half asserts the non-match fixture's id is **absent**. This needs a new
   `assert_json_not_contains <jq-expr> <substring>` helper.

   For **sort/order** tests the same identify-by-id rule applies, plus the fixtures must
   carry controlled, distinct values in the sort key (e.g. three bugs with priorities
   P1/P3/P5): assert the returned ids of *those fixtures* appear in the expected relative
   order, rather than asserting global ordering over the whole mutating corpus.

2. **Absolute counts against the shared corpus are flaky.** Never assert an exact total
   (`--count`, `--offset`, `--paginate` lengths) against the whole database — any added
   or reordered test shifts it. Scope every count/paging test to a **freshly created,
   uniquely-marked fixture set**: create exactly N bugs stamped with a per-test
   whiteboard marker, filter on that marker, and assert exactly N / the expected page
   boundaries. The marker isolates the assertion from the growing corpus and from other
   version runs.

Marker discipline — **per-run-unique, not deterministic.** `setup-bugzilla.sh start`
(and therefore `make functional-test`) **reuses an already-running container without
resetting the DB** (`setup-bugzilla.sh:83-92`), so fixtures from a previous run persist.
A stable marker like `pagetest-09-1` would then collide across runs (run 2 creates a
second bug with the same marker → exact count off by one). Derive the marker from a
per-run token — reuse the harness's existing `mktemp` run id, or `$RANDOM` — so re-runs
on a persisted container never collide. (The repo's no-`Math.random`/`Date.now` rule is
about Workflow JS scripts, not bash test fixtures; shell tests may use `$RANDOM`.) Exact
counts are reproducible across re-runs **because** each run's marker is fresh; they do
**not** require a DB reset, though `setup-bugzilla.sh reset` remains the way to reclaim a
clean DB if accumulated fixtures slow a container down.

The forced **mid-air-collision** test (`bug update --expect-unchanged-since`) has a
timing hazard: `last_change_time` is second-granular, so a `view → mutate → guarded
update` that completes within one second may not advance the timestamp and the collision
(exit 14) won't fire. Make the precondition deterministic: capture `last_change_time`,
perform a mutation, then poll `bug view` until it reports a **strictly greater**
`last_change_time` before issuing the guarded update; only then assert exit 14.

Negative-path assertions must check the **reason**, not just the code. Exit 2 is
overloaded — `src/error.rs:97,182` map `NotFound = 2`, the same code clap returns for
usage/conflict errors, so a bare `assert_exit_code 2` also passes on a flag typo or a
not-found. Every conflict case pairs `assert_exit_code 2` with `assert_stderr_contains`
on the specific clap conflict message, so the test proves the conflict path fired.

## Harness Enhancements (lib.sh)

Add the assertions the new tests need:

- `assert_ndjson_line_count <n>` — count non-empty lines in `$BZR_STDOUT` (ndjson).
- `assert_json_valid` — `jq -e . >/dev/null` to confirm parseable JSON (schema tests).
- `assert_stderr_contains <substr>` — mirror of `assert_stdout_contains` for
  truncation/dry-run notices, conflict messages, and routed errors.
- `assert_json_not_contains <jq-expr> <substring>` — assert a value is **absent**, for
  the discriminating-fixture exclusion checks above.
- `assert_count <n>` — assert `{"count": n}` JSON shape from `--count` (used only on
  marker-isolated fixtures, never the shared corpus).
- `make_bug [--marker <tag>] …` — create a bug, echo its id; optional marker stamps a
  whiteboard tag (caller passes a per-run-unique value) for isolated filter/paging/count
  tests.
- `wait_for_changed <bug_id> <prev_last_change_time>` — poll `bug view` until
  `last_change_time` strictly advances; backs the deterministic collision test.

New fixture needs in the container entrypoints (`versions/bz5*/entrypoint.sh`):

- A second product + milestones/versions so `--target-milestone`/`--version` filters
  have non-trivial data to match.
- A flag type definition (e.g. `review`) enabled on bugs and attachments so `--flag`
  paths exercise real flag handling rather than erroring. **Gate** flag tests with
  `require_version` if a version lacks the definition.

## Exhaustive Coverage To Add (by phase)

Each item below is one or more `test_begin` cases. "Negation" means the `!`-prefixed
filter form. Mutations assert the round-trip via a follow-up `bug view`/`list`. Filter,
paging, and conflict cases follow the **Test-Design Contract** above (discriminating
fixtures, marker-isolated counts, stderr-checked conflicts) — those rules are not
restated per bullet.

- **01-config:** `remove-server` (happy, refuse-current-default, keychain cleanup),
  `rename-server` (preserves creds, rewrites `default_server`), error on duplicate/new
  name (config error, exit 3), removal of nonexistent (exit 3).
- **02-server-auth:** `--server-url + --server-api-key-env` round-trip with no config
  file; `--config <alt>` reads an alternate file; `--server-url` + `--server` conflict
  (exit 2 + stderr conflict message); confirm `whoami show` now errors (exit 2 +
  unexpected-arg message). (`--server-email` is **not** scheduled here — see Open Risks;
  it is not reachable while API-key whoami succeeds, so it stays unit-tested.)
- **03-products:** create `--version` and `--is-open false` (assert both via `product
  view`); update `--default-milestone` and `--is-open` round-trips. (Closes the
  `--version/--is-open/--default-milestone` gaps flagged in the analysis.)
- **04-components:** `component list --product`, `component view <prod> <name>`,
  update `--name`/`--default-assignee` (version-gate as `component update` already is).
- **05-fields-classifications:** `classification list` (and the disabled-note path).
- **06-users:** create `--login`, update `--real-name`/`--email`.
- **08-bugs-create:** every metadata field (`--alias --url --whiteboard
  --target-milestone --deadline --cc --keywords --groups --flag`), each asserted in the
  resulting `bug view`; `--from-json` object, array, stdin `-`, unknown-key (exit 7),
  CLI-overrides-JSON; `--template` + `--from-json` mutual exclusion (exit 2 + stderr).
- **09-bugs-read:** each `bug list` filter and its negation (`--target-milestone
  --version --op-sys --platform --qa-contact --url --assignee --creator --priority
  --severity --alias --summary`) using discriminating match/non-match fixtures,
  asserting the match fixture's own id is present and the non-match id is absent;
  `--sort`/`--order` (asc/desc, on fixtures with controlled distinct sort-key values,
  asserting those ids' relative order); `--offset` and `--paginate` (mutually exclusive → exit 2 +
  stderr; page boundaries asserted on a marker-isolated N-bug set); `--count` (exact
  count only against the marked set, plus conflict-with-offset/paginate → exit 2 +
  stderr); `bug search --from-url`, `--save-as` (requires `--from-url`); `bug view
  --web` headless URL print (assert the printed `show_bug.cgi?id=` URL, no browser);
  `bug view --exclude-fields`; `bug my --created/--cc/--all` with paging/sort;
  `--output ndjson` on a list.
- **10-bugs-update:** `--alias` (single bug), `--groups-add/remove`,
  `--see-also-add/remove`, `--estimated-time/--remaining-time/--work-time`,
  `--expect-unchanged-since` happy path **and** forced collision (exit 14) using the
  deterministic `wait_for_changed` precondition from the Test-Design Contract; batch
  collision aborts before any write (verify no leg committed via follow-up `bug view`).
- **11-bugs-verbs (NEW):** `bug resolve` (default FIXED + `--as DUPLICATE` requires
  dupe semantics; use WONTFIX/INVALID), `bug close --as`, `bug reopen`, `bug dup
  <id> <target>`; each with `--comment`/`--comment-file`/`--comment-private`; batch
  forms with one invalid id → exit 11; assert state via `bug view`.
- **15-comments:** `comment add --body-file`, stdin `-`, body/body-file conflict
  (exit 2).
- **16-attachments:** `attachment view <id>` metadata; upload `--comment` +
  `--comment-private` + `--flag`; update `--file-name`, `--content-type`, `--flag`,
  and each override pair (`--obsolete/--no-obsolete`, etc.).
- **17-global-options:** `--output ndjson` shape (incl. empty list → zero lines,
  truncation note on stderr); `--dry-run` on each mutation verb (no write — confirm via
  follow-up read) and exit 7 on a non-mutation; `--retry 2`/`--timeout 5` happy-path
  smoke; `-y` large-batch (>10 bugs) confirmation bypass.
- **18-completion-schema (NEW):** `completion bash|zsh|fish|powershell|elvish` each
  emits a non-empty, shell-appropriate script; `schema` (list) and `schema <name>`
  emit valid JSON (`assert_json_valid`).

## Phasing (implementation order)

1. **Refactor, no behavior change.** Split `run-tests.sh` into `phases/*.sh`; the
   orchestrator sources them in the existing order. **Manual pre-merge gate** (these
   tests are not in CI — they need containers and run via `make`): before the split,
   capture the ordered per-test list and result by piping a run through
   `grep -E 'TEST|PASS|FAIL|SKIP'` to a baseline file; after the split, `diff` the two.
   Equal pass/skip/fail **totals are necessary but not sufficient** — a reordered or
   silently-mutated test can preserve totals — so the gate is a zero-line diff of the
   ordered name+result list, run on all three versions. Commit.
2. **Harness + fixtures.** Add the new `assert_*` helpers and container fixtures
   (second product, milestones, flag type). Commit.
3. **New coverage, resource by resource.** Land each `phases/*.sh` expansion as its own
   small commit (bisectable), running the affected version(s) after each. Highest-value
   first: bug verbs → paging/count → from-json → collision guard → completion/schema →
   config remove/rename → ndjson/dry-run → remaining field gaps.
4. **CHANGELOG + docs.** Note expanded functional coverage; cross-check `docs/bzr-cli.md`
   for any flag the new tests reveal as undocumented.

## Verification

- After step 1: the ordered per-test name+result diff (Phasing step 1) is empty on all
  three versions — proves the split is inert. A matching-totals-but-nonempty diff is a
  regression, not a pass.
- After each expansion commit: run the touched version(s); every new `test_begin` ends
  `PASS` or an intentional `SKIP` (version-gated) — never `FAIL`.
- Filter tests prove filtering, not execution: each asserts presence of a matching
  fixture **and absence** of a non-matching one (Test-Design Contract).
- Count/paging tests assert exact values only against per-run-unique marker-isolated
  fixtures, never the shared corpus. Because each run stamps a fresh marker, re-running
  the suite against the **reused** container (the default `make functional-test` path,
  which does not reset the DB) still yields passing exact-count assertions.
- Cross-version: features gated behind Bugzilla capability (flags, `component update`,
  user-create email-as-login) must `SKIP` cleanly on older versions, not error.
- Negative paths assert the **exact** documented exit code (verified in `src/error.rs`:
  7 input validation, 11 batch partial failure, 14 mid-air collision, 3 config; clap
  conflicts 2) via `assert_exit_code` **and** an `assert_stderr_contains` on the reason,
  because exit 2 is shared by clap usage errors and `NotFound`.
- Confirm no test leaks fixtures across versions (each run uses a fresh container and a
  temp `XDG_CONFIG_HOME`).

## Open Risks

- **Flag fixtures** require server-side flag-type setup; if a version can't be
  configured, gate those tests rather than ship a degraded assertion.
- **`--retry`/`--timeout`** can't reliably force transient failures inside a healthy
  container — keep these as happy-path smoke tests; real retry logic stays in unit
  tests (wiremock can inject 429/5xx).
- **`--server-email`** is only reached when API-key whoami can't resolve identity. All
  three containers authenticate by API key, so this fallback is **not functionally
  exercisable** in the current harness — it stays unit-tested, not counted as functional
  coverage. Revisit only if a container is configured to disable API-key whoami.
- **`bug view --web`** must be exercised in headless mode so it prints the URL instead
  of spawning a browser; assert on the printed `show_bug.cgi?id=` URL.
- **Runtime budget.** "Exhaustive every field × negation × 3 versions" multiplies a
  suite that already runs ~108 tests against containers with 90–240 s startup. A suite
  too slow to run stops being run, which defeats the goal. Track the new test count and
  wall-clock per version as coverage lands; if a full run exceeds a practical budget
  (target: keep a single-version run under a few minutes of test time, excluding image
  build/startup), tier into a fast `make functional-test` smoke subset and a full
  `make functional-test-all` rather than dropping coverage. Reuse marker-isolated
  fixtures across related assertions to avoid one-bug-per-assertion blowup.
