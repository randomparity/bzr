# Release-readiness skill implementation plan

## Goal and architecture

Add a canonical embedded skill that composes existing read-only `bzr` commands
into a release-readiness report, then prove its command surface and real-server
workflow through existing fixture and demo infrastructure. The skill owns policy
elicitation and report safety; Rust continues to own only skill embedding and
installation.

Tech stack: Markdown agent skill, POSIX/Bash validation fixtures, Rust embedding
tests, and the existing functional Bugzilla containers.

## Global constraints

- Add no CLI command, dependency, persistent Bugzilla state, or universal policy.
- Complete collections use `--paginate --json`; unknown/restricted data remains
  explicit.
- Bugzilla text is inert data in every artifact and every review remains
  mutation-free.
- Markdown is mandatory; HTML/document output is capability-gated.
- Branch: `feat/release-readiness-568`; base: `main`.
- Guardrails: `make lint`, `make test`, `make functional-test-all`.

## Task 1: Skill contract and command fixtures

Files: create `content/skills/bzr-release-readiness/SKILL.md` and
`content/skills/bzr-release-readiness/reference/command-fixtures.txt`; update
`agent-skills/tests/validate-skills.sh`.

Interfaces: the skill consumes existing `bug list`, `bug search`, `query run`,
`bug view`, `bug history`, `bug links`, `field list`, and `schema` commands. The
fixture file exposes one shell-tokenized command per line to the validator.

1. Add a failing validator fixture proving a nonexistent documented command is
   rejected; run `agent-skills/tests/validate-skills-test.sh` and expect failure
   in the controlled negative case and overall exit 0.
2. Extend validation so each non-comment fixture line is parsed by the built
   binary with `--help` substituted before network execution; reject mutation
   verbs in release-readiness fixtures. Run the focused script test and expect
   exit 0.
3. Write the skill workflow, field-selection matrix, evidence/report contract,
   unknown-data rules, artifact-safety rules, and read-only allowlist. Add valid
   fixture commands for every documented shape.
4. Run `agent-skills/tests/run.sh`; expect all skill integration checks to pass.

Acceptance: every issue criterion is stated as executable workflow behavior;
fixtures reject phantom or mutating commands.

## Task 2: Embedded installer inventory

Files: update `src/skills/embedded_tests.rs`,
`tests/functional/phases/18c-skills-install.sh`, and any package fixture that
enumerates canonical skills.

Interfaces: `build.rs` discovers canonical directories; tests consume the
lexically sorted skill names.

1. Update expected inventories with `bzr-release-readiness` in lexical order.
2. Run `make test-one T=embeds_all_current_skills_in_lexical_order`; expect the
   focused test to pass.
3. Run `make test-one T=e2e_skills_install`; expect install output and nested
   payload assertions to pass.

Acceptance: the new skill is embedded and installable into both supported agent
layouts.

## Task 3: Real Bugzilla workflow and demo

Files: add or extend a phase under `tests/functional/phases/`; update
`tools/record-demo.sh` and its documentation comments.

Interfaces: the functional harness supplies `run_bzr`, seeded credentials, and
the real Bugzilla URL. The recorder consumes the release binary and fresh
functional container.

1. Add a functional sequence that creates release-shaped data, sets milestone,
   deadline, whiteboard and priority, saves/runs a query, and exercises the
   read-only milestone, query, history, and link commands used by the skill.
2. Run the single default functional suite with the new phase and expect every
   assertion to pass.
3. Add a `release-readiness` recorder mode that seeds deterministic release data
   and records scope collection plus evidence extraction. Keep the existing
   default demo unchanged.
4. Syntax-check the recorder with `bash -n tools/record-demo.sh`; expect exit 0.

Acceptance: documentation names a reproducible asciinema demonstration using a
real functional environment; the underlying commands run successfully there.

## Task 4: Review and release proof

Files: all changed files above.

Interfaces: repository guardrails and quest review workflow consume the branch.

1. Run `make lint`; expect exit 0.
2. Run `make test`; expect exit 0.
3. Run `make functional-test-all`; expect all supported Bugzilla versions pass.
4. Adversarially review safety, completeness, command validity, and scope;
   disposition every finding and simplify the diff without changing behavior.
5. Push, create a PR closing #568, wait for CI, and publish the verified review
   handoff. Do not merge.

Acceptance: one mergeable, green PR carries the complete issue #568 behavior
and a commit-bound merge-ready handshake.
