# Release-readiness skill implementation plan

## Goal and architecture

Add an embedded `bzr-release-readiness` skill that guides an agent through a
read-only, policy-eliciting Bugzilla review and produces a PM-usable readiness
artifact. Keep decisions in `SKILL.md`, move detailed cases and the report shape
to references, and use a small skill-local shell contract test plus the real
functional Bugzilla harness for proof. No runtime analysis program is added.

Tech stack: Markdown skill/reference files, POSIX shell contract checks, Rust
embedding tests, Bash functional phases, and the existing demo recorder.

## Global constraints

- Add no CLI command, dependency, persistent Bugzilla state, fixed agent
  runtime/evaluation harness, or universal readiness policy.
- Complete collections use `--limit 100 --paginate --sort bug_id --order asc`;
  unknown/restricted data remains explicit.
- Bugzilla text remains inert in artifacts and every review remains read-only.
- Markdown is mandatory; HTML/document output is capability-gated.
- Branch: `feat/release-readiness-568`; base: `main`.
- Guardrails: `make lint`, `make test`, `make functional-test-all`.

## Task 1: Skill contract and deterministic fixtures

Files: create `content/skills/bzr-release-readiness/SKILL.md`,
`content/skills/bzr-release-readiness/reference/eval-cases.md`,
`content/skills/bzr-release-readiness/reference/report-template.md`,
`content/skills/bzr-release-readiness/tests/fixtures/release-bugs.json`,
`content/skills/bzr-release-readiness/tests/fixtures/release-report.expected.md`,
and `content/skills/bzr-release-readiness/tests/run.sh`; update
`agent-skills/tests/run.sh`.

Interfaces:

- `SKILL.md` consumes only `bzr bug list`, `bug search`, `query show`, `query
  run`, `bug view`, `bug history`, `bug links`, `field list`, `server
  capabilities`, and `schema` read surfaces.
- The report template defines the PM artifact sections and fact/assumption/
  assessment labels; eval cases name the section and rule each fixture protects.
- `tests/run.sh` consumes the committed fixture and installed `BZR_BIN`; it
  validates all documented command shapes through `--help`, rejects mutation
  verbs, checks selected-field projections and bounded paging templates, and
  compares the hostile-data Markdown fixture byte-for-byte.

Steps:

1. Create a temporary controlled copy containing a nonexistent command shape in
   the skill-local test, run `sh content/skills/bzr-release-readiness/tests/run.sh`,
   and require that controlled negative case to fail while the test exits 0.
2. Add the minimal test driver and hook it into `agent-skills/tests/run.sh`; run
   `sh content/skills/bzr-release-readiness/tests/run.sh` and expect exit 0.
3. Write `SKILL.md` with scope selection, policy elicitation, exact command
   templates, selected-field matrix, rolling-snapshot/unknown rules, custom-field
   type/operator validation, total readiness precedence, read-only allowlist,
   and capability-gated artifact safety.
4. Add the detailed RR cases and report template, including prompt input and the
   final PM-facing report fixture; rerun the skill-local test and
   `agent-skills/tests/run.sh`, expecting exit 0.

Acceptance: every issue criterion changes an agent decision; every documented
command shape is executable; fixtures demonstrate the report contract without
claiming that CI executes an agent.

## Task 2: Embedded installer inventory

Files: update `src/skills/embedded_tests.rs` and
`tests/functional/phases/18c-skills-install.sh`.

Interfaces: `build.rs` discovers canonical skill directories; both tests consume
the lexically sorted skill names and the functional phase verifies the complete
installed payload.

Steps:

1. Add `bzr-release-readiness` to both expected inventories and list its six
   payload files in the functional installer fixture.
2. Run `make test-one T=embeds_all_current_skills_in_lexical_order`; expect one
   passing focused test.
3. Run `make test-one T=skills_install`; expect the installer integration tests
   to pass.

Acceptance: both supported agent layouts receive the complete skill payload.

## Task 3: Real Bugzilla workflow and prompt-to-artifact demo

Files: create `tests/functional/phases/18e-release-readiness.sh` and
`tools/run-release-readiness-demo.sh`; update `tests/functional/run-tests.sh`,
`tools/record-demo.sh`, and create `docs/bzr-release-readiness.md` plus the
generated `docs/assets/bzr-release-readiness-demo.cast` and
`docs/assets/bzr-release-readiness-demo.gif`.

Interfaces:

- The functional phase uses existing `run_bzr`, fixture mutation helpers, and
  the active Bugzilla container to seed and read release-shaped data.
- `tools/run-release-readiness-demo.sh` consumes a server profile and fixture
  marker, runs the same read-only commands as the skill, and writes one
  deterministic Markdown report for recording; it is demo plumbing, not a
  required skill runtime.
- `tools/record-demo.sh release-readiness` records an agent-style request followed
  by the final report, while hiding setup commands and private paths.

Steps:

1. Add a functional phase that seeds deadline, assignee, milestone, priority,
   whiteboard, history, dependency, saved-query, and custom-field evidence.
2. Exercise Custom Search URL, saved query, milestone, version, and product
   complete reads with explicit paging/order/projections plus the supplementary
   read-only commands; assert no mutation occurs during the review segment.
3. Run the default functional suite and expect the new phase green.
4. Add and test the demo helper against that live fixture; require its report to
   distinguish facts, assumptions, assessment, limitations, and bug IDs.
5. Record a terminal session whose visible flow is the example agent prompt and
   final PM artifact, inspect the cast for credentials/private paths, render the
   GIF, and document regeneration in `docs/bzr-release-readiness.md`.

Acceptance: the real-server commands work, and the published demo shows the
requested analysis and usable result rather than implementation plumbing.

## Task 4: Review and release proof

Files: the complete branch diff.

Interfaces: repository guardrails and quest review consume the branch.

Steps:

1. Run `make lint`; expect exit 0.
2. Run `make test`; expect exit 0.
3. Run `make functional-test-all`; expect every supported Bugzilla version green.
4. Run branch adversarial and security reviews, disposition every finding, and
   simplify the diff without weakening behavior.
5. Push, create a PR closing #568, wait for CI, and publish the verified review
   handoff. Do not merge.

Acceptance: a green, mergeable PR carries the complete skill, real-server proof,
and prompt-to-PM-artifact demonstration.
