# Dependency Analysis Skill Implementation Plan

Goal: ship an embedded, installable `bzr-dependency-analysis` skill that collects bounded
Bugzilla dependency evidence, analyzes it deterministically, renders safe artifacts, and proves the
installed workflow against real functional Bugzilla servers.

Architecture: three Python 3 standard-library helpers form an explicit pipeline:
`collect.py` invokes only allowlisted read-only `bzr` commands and writes a versioned collection;
`analyze.py` transforms that inventory into deterministic graph findings; `render.py` consumes only
the analysis schema. The skill instructions choose inputs and explain limitations. Unit fixtures
cover graph semantics; the existing Rust embedding and shell functional harness prove packaging and
live CLI behavior.

Tech stack: Python 3 standard library, POSIX shell functional phases, existing Rust embedded-skill
loader, `jq`, Docker-compatible Bugzilla harness, asciinema/agg demo tooling.

## Global Constraints

- Branch `feat/dependency-analysis-570`; base branch `main`.
- Host is arm64 macOS. Declared targets remain x86_64/aarch64 Linux, powerpc64le, s390x,
  aarch64 macOS, and x86_64/aarch64 Windows; host and target set differ.
- Python helpers use only the standard library and run without a shell when invoking `bzr`.
- Default bounds are depth 5 and 200 nodes; both are positive and every lookup consumes a node slot.
- Identity is `(server alias, numeric bug ID)`; unresolved aliases use `(server, requested)`.
- Resource-scoped Bugzilla codes 100/101/102 become sanitized unknown nodes. Every other API,
  authentication, TLS, connection, and transport failure is run-fatal with a valid partial file.
- Numeric roots and scope responses use ascending bug-ID order. Mixed provenance uses the exact
  total ordering and deduplication contract in the specification.
- Direction controls discovery and canonical-edge creation. Raw observations are staged before
  endpoint filtering; a second pass attaches reciprocal evidence without fetch-order dependence.
- Version 1 reports only longest dependency chain by edge count and never calls it a schedule or
  weighted critical path.
- No Bugzilla mutation, no Codex/model runtime or confinement harness, and no new Rust graph policy.
- User-facing output safely escapes Markdown and Mermaid contexts.
- Functional coverage is mandatory. Final gates: `make lint`, `make test`, and
  `make functional-test-all`.

## Task 1: Deterministic collection helper and fixtures

Files:

- Create `content/skills/bzr-dependency-analysis/scripts/collect.py`.
- Create `content/skills/bzr-dependency-analysis/tests/test_collect.py`.
- Create collection inputs and byte-exact oracles under
  `content/skills/bzr-dependency-analysis/tests/fixtures/`.

Interfaces:

- CLI: `collect.py --policy PATH --output PATH [--runner PATH] [--analysis-timestamp TIMESTAMP]`.
- Policy JSON supplies ordered scopes, server aliases, bounds, direction, restrictions, resolved
  mode/statuses, stale days, and `bzr` executable. Replay time comes only from the optional CLI
  timestamp in exact `YYYY-MM-DDTHH:MM:SSZ` form.
- Output: exact `bzr-dependency-collection/v1` JSON specified by the design.
- Later tasks consume only the emitted JSON; tests inject a runner executable that records argv and
  returns fixture envelopes.

Steps:

1. Write failing unittest cases DA-04, DA-06, DA-13 through DA-15c, DA-17, DA-18, and DA-21. Include
   root permutations at caps, alias/numeric collapse, code 100/101/102 continuations, fatal API and
   transport envelopes, mixed-scope provenance permutations, cycles, restriction overflow, and
   direction isolation with reciprocal evidence arriving before and after selected observations.
2. Run `python3 -m unittest content/skills/bzr-dependency-analysis/tests/test_collect.py`; expect
   import/file failures and record the red result.
3. Implement strict policy validation, reject malformed `--analysis-timestamp` before invoking the
   runner, canonical scope enumeration with `--sort bug_id --order asc`,
   alias-first cap reservation, BFS fetch-once admission, sanitized envelope parsing, atomic writes,
   and deterministic two-pass observation normalization. Reject unknown keys and malformed schemas.
4. Re-run the focused unittest; expect all collection cases to pass and every command log to contain
   only `bug view`, `bug list`, `bug search`, or `query run` with documented flags.
5. Introduce a controlled fault that maps API 102 to fatal, confirm DA-15b fails, restore it, and
   re-run green.
6. Commit `feat(skills): collect bounded dependency evidence`.

Acceptance: byte-identical output under all stated permutations; no lookup beyond `max_nodes`; each
admitted identity fetched at most once; codes 100/101/102 are distinct nonfatal unknowns; other
failures preserve valid partial output without raw server text.

## Task 2: Graph analyzer and deterministic PM findings

Files:

- Create `content/skills/bzr-dependency-analysis/scripts/analyze.py`.
- Create `content/skills/bzr-dependency-analysis/tests/test_analyze.py`.
- Add branch, diamond, cycle, missing, inaccessible, resolved, empty-partial, cross-server, and stale
  collection/oracle fixtures under the fixture directory.

Interfaces:

- CLI: `analyze.py --input PATH --output PATH [--allow-partial]`.
- Input: collection v1 only. Output: exact `bzr-dependency-analysis/v1` JSON.
- Component identity is `cNNNN`; downstream renderers depend on exact fields and stable ordering.

Steps:

1. Write failing cases DA-01, DA-03, DA-05 through DA-11, DA-16, DA-19, and DA-20. Assert SCCs,
   condensation layers, edge-count longest chain, roots/leaves, fan-out bottlenecks, unassigned and
   stale blockers, unknown boundaries, policy assumptions, root/provenance copying, and empty graph.
2. Run `python3 -m unittest content/skills/bzr-dependency-analysis/tests/test_analyze.py`; expect red.
3. Implement strict schema validation, canonical edges, Tarjan SCCs, deterministic component IDs,
   topological layers, lexicographically tied longest path, total staleness table, PM findings, and
   atomic canonical JSON.
4. Run focused tests; expect all pass. Mutate the SCC visited-set rule, prove the cycle fixture fails,
   restore it, and re-run green.
5. Commit `feat(skills): analyze dependency graph evidence`.

Acceptance: all nodes belong to one component; endpoint/component references close; cycles never
loop; partial input requires opt-in; no duration or delivery-date claim appears.

## Task 3: Safe renderers and skill contract

Files:

- Create `content/skills/bzr-dependency-analysis/scripts/render.py`.
- Create `content/skills/bzr-dependency-analysis/tests/test_render.py`.
- Create `content/skills/bzr-dependency-analysis/tests/skill-contract.sh`.
- Create `content/skills/bzr-dependency-analysis/SKILL.md`.
- Add hostile rendering fixtures and expected Markdown/Mermaid artifacts.

Interfaces:

- CLI: `render.py --input PATH --format markdown|mermaid --output PATH`.
- `SKILL.md` documents the exact three-stage commands, defaults, allowed reads, structural wording,
  unknown-node behavior, and refusal of mutation requests.

Steps:

1. Write failing DA-02, DA-07, and DA-12 tests for HTML-like payloads, Markdown
   links/images/fences, and Mermaid directives and quoting.
2. Run the Python renderer test and `bash .../skill-contract.sh`; expect red.
3. Implement strict analysis input parsing and deterministic format-specific escaping. Disable
   Markdown raw HTML, images, and autolinks; quote Mermaid labels and directives; use atomic output
   writes.
4. Write the skill workflow using only commands validated by current `--help` output. State bounds
   before retrieval and describe fixture-only cycle proof and unsupported weighted analysis.
5. Run focused tests, inspect complete Markdown fences and Mermaid tokens, and expect green.
6. Commit `feat(skills): render safe dependency reports`.

Acceptance: no hostile fixture becomes active syntax; all output is deterministic; contract scan
finds no phantom flags or mutation commands.

## Task 4: Embed, install, and exercise the complete installed pipeline

Files:

- Modify `src/skills/embedded_tests.rs`.
- Modify `tests/functional/phases/08e-bugs-restricted-access.sh` and
  `tests/functional/phases/18c-skills-install.sh`.
- Create `tests/functional/phases/18d-dependency-analysis.sh`.
- Modify `tests/functional/run-tests.sh`.

Interfaces:

- The build script automatically embeds every file below `content/skills`; Rust tests and
  `SKILLS_EXPECTED` enumerate `bzr-dependency-analysis` lexically.
- Phase 18d consumes `SKILLS_PROJECT` and earlier `RESTRICTED_BUG`, which `run-tests.sh` initializes
  and phase 08e intentionally preserves through 18d. It uses only the freshly installed
  `.agents/skills/bzr-dependency-analysis` paths, and invokes the real release `bzr` binary.

Steps:

1. Add failing Rust and shell assertions for all payload paths and installed fixture pipeline.
2. Run `make test-one T=embeds_all_current_skills_in_lexical_order`; expect failure until the name
   list is updated.
3. Initialize `RESTRICTED_BUG` in `run-tests.sh`, remove it from phase 08e's early cleanup, and unset
   it at the end of 18d. Update embedding/install lists and extend 18c to run an installed fixture cycle through analyze
   and render.
4. Build 18d live fixtures: branch, diamond, resolved blocker, hostile summary, a nonexistent root,
   and the earlier restricted bug through the credentialless server. Give the disposable graph a
   stable unique whiteboard marker that the recorder can later discover without mutation. Run installed collect,
   analyze, and render; assert separate `not_found`/`inaccessible` unknowns, continuation, bounds,
   server identities, resolved behavior, and inert text.
5. Source 18d after 18c in `run-tests.sh`; run the default functional suite and expect all new cases
   green before proceeding.
6. Commit `test(functional): prove installed dependency analysis`.

Acceptance: no stage resolves from the checkout; real missing and inaccessible envelopes are parsed
by installed collector code; deterministic cycle proof runs from installed fixtures.

## Task 5: Publish the user documentation and demonstration workflow

Files:

- Create `docs/bzr-dependency-analysis.md`.
- Modify `tools/record-demo.sh`.
- Create generated `docs/assets/bzr-dependency-analysis-demo.cast` and
  `docs/assets/bzr-dependency-analysis-demo.gif` through the recorder.

Interfaces:

- `tools/record-demo.sh dependency-analysis` records the named cast/GIF while its no-argument path
  retains the existing README demo behavior.
- Documentation embeds the GIF and links the cast; `skill-contract.sh` checks both references.

Steps:

1. Add failing shell assertions for the named recorder mode and documentation asset references.
2. Implement `dependency-analysis` orchestration and `--drive-dependency-analysis` PTY modes. The
   orchestrator creates a temporary project, runs `bzr skills install --agent codex --project`,
   resolves collector/analyzer/renderer beneath that installation, and discovers the graph already
   provisioned by phase 18d using its stable marker and read-only commands. It writes the policy with
   the discovered IDs and passes only those paths and IDs to the drive mode. If the marker is absent,
   stop with an instruction to run the functional setup; never create bugs or dependency links.
   Keep the current no-argument and `--drive` branches byte-compatible.
3. Record directly to `docs/assets/bzr-dependency-analysis-demo.cast`, using the installed helpers
   in the drive commands, then render the named GIF. Record the live
   branch/diamond/resolved/hostile-summary flow against the functional server;
   inspect the cast for secrets and render the GIF.
4. Write the documentation with exact installed commands, limitations, and structural terminology.
5. Run `bash content/skills/bzr-dependency-analysis/tests/skill-contract.sh`; expect green.
6. Commit `docs(skills): demonstrate dependency analysis`.

Acceptance: both assets exist, are referenced by published documentation, contain no credential or
private host data, the displayed workflow is read-only, and the default recorder remains unchanged.

## Task 6: File the measured retrieval-primitive follow-up

Files: no repository files; create one GitHub issue through the authorized bounty workflow.

Interfaces: the follow-up is a narrow CLI contract for typed, bounded multi-bug adjacency retrieval;
it links issue #570 and carries campaign occurrence
`cdd7727c70ce-5cc933a5-0ee2-4c20-bf70-0c97cb2eff7d source=#570 sweep=#570`.

Steps:

1. Search open and closed issues for equivalent typed batched Bug.get/adjacency retrieval.
2. If absent, file one PR-sized issue documenting measured deficiencies: `bug links` omits roots and
   revisited edges; multi-ID permissive failures are prose-only; aliases can duplicate numeric IDs.
3. Require versioned structured per-ID failures, bounded deterministic request semantics, and no
   Rust graph-analysis policy. Link it from #570 and verify labels/state/relationship.

Acceptance: exactly one deduplicated follow-up exists and its complete campaign occurrence tuple is
available for the final report.

## Task 7: Final verification and handoff preparation

Files: all files above; no new surface.

Interfaces: all pipeline contracts are frozen by byte-exact fixtures and installed-copy tests.

Steps:

1. Run every Python unittest and the shell contract check; expect zero failures.
2. Run `make lint`; expect exit 0 and no warnings.
3. Run `make test`; expect exit 0.
4. Run `make functional-test-all`; expect all bz50, bz52, and bz53 phases green.
5. Re-read `git diff main...HEAD` for scope, generated assets, secrets, and naming; remove only
   redundant implementation within the authorized surface.
6. Commit any behavior-preserving cleanup as `refactor(skills): simplify dependency analysis`.

Acceptance: every issue checklist item maps to a passing deterministic or live test; all required
guardrails are green; no untracked or modified file remains.

Rollback: deleting the new embedded skill, its additive test phase, documentation/assets, and list
entries restores prior behavior. No config, persisted Bugzilla data model, or Rust public API changes.
