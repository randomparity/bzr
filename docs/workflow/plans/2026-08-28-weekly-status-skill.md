# Weekly status skill implementation plan

Goal: ship an embedded `bzr-weekly-status` skill with a tested versioned snapshot protocol and a
real-container baseline/comparison demonstration. The implementation is documentation plus portable
shell fixture validation around existing Rust CLI behavior; no new runtime dependency or CLI surface
is added.

## Global constraints

- Canonical source is `content/skills/`; `build.rs` embeds every regular file recursively.
- Snapshot format is version 1 and follows ADR 0023.
- Markdown requires no optional artifact tool; other formats are capability-gated.
- Bugzilla-controlled text is untrusted; spreadsheet formulas and HTML/link injection are blocked.
- The skill never mutates Bugzilla and never stores credentials, comments, or attachment bodies.
- `main` is the base. Run `make skills-test`, `make lint`, `make test`, and
  `make functional-test-all`; functional containers are mandatory for this user-visible skill.

## Task 1: Define and test the snapshot protocol

Files: create `content/skills/bzr-weekly-status/SKILL.md`,
`content/skills/bzr-weekly-status/reference/snapshot-v1.schema.json`,
`content/skills/bzr-weekly-status/scripts/compare-snapshots.jq`,
`content/skills/bzr-weekly-status/scripts/scope-fingerprint.sh`,
`content/skills/bzr-weekly-status/scripts/safe-output.jq`,
`content/skills/bzr-weekly-status/scripts/select-baseline.sh`,
`content/skills/bzr-weekly-status/scripts/publish-run.sh`, an eval-case manifest, and fixtures under
`content/skills/bzr-weekly-status/tests/`; modify `agent-skills/tests/run.sh` to execute them.

Interfaces: `compare-snapshots.jq` consumes two validated snapshot-v1 JSON objects plus effective
rules and emits deterministic JSON groups for membership, field transitions, staleness, and
limitations. The fingerprint helper consumes `query show --json`; the selector consumes current
snapshot, runs directory, and required fields; the comparator consumes snapshot rules and emits
opened/resolved, field-transition, staleness-crossing, and unchanged-attention groups; safe-output
filters own rendered cell/text/link values. `publish-run.sh ROOT RUN STAGING` requires staging below `ROOT/.staging`, validates `STAGING/snapshot.json`, atomically renames
the staging directory to `ROOT/runs/RUN`, and atomically replaces `ROOT/latest` with a relative
symlink. The skill consumes both; Task 2 documents the exact `bzr` collection commands.

Steps:

1. Add failing fixture cases for first run, compatible changes, removed scope, inaccessible data,
   same-name changed query definition, reordered equivalents without tuple collisions, X→Y→X newest-compatible selection, incompatible format/server/scope/fields,
   rendered spreadsheet/HTML/URL payload safety, and a publisher failure after immutable-run creation but before pointer replacement.
   Run the new fixture script directly and expect nonzero because implementation is absent.
2. Add the minimum jq comparator, JSON schema, safe-output rules, executable atomic publisher, and
   eval-case manifest plus static allowlist/denylist assertions.
3. Run the fixture script and expect every named case to print `ok`; run `make skills-test` and expect
   exit 0.
4. Commit as `feat(skills): add snapshot-based weekly status workflow`.

Acceptance: every issue #569 state/compatibility/failure criterion has a deterministic fixture, and
the installed payload contains the skill, schema, script, and tests.

## Task 2: Prove documented CLI collection against Bugzilla

Files: extend the appropriate script in `tests/functional/phases/` and
`content/skills/bzr-weekly-status/SKILL.md`.

Interfaces: the phase exercises `query save --from-url`, `query run --fields --paginate --json`,
`bug history --json`, and selected whiteboard/relationship fields exactly as the skill documents.

Steps:

1. Add a functional assertion that fails if the documented named-query and imported-URL workflows,
   projected fields, whiteboard, relationships, or history shapes drift.
2. Run the focused functional phase and expect the new assertions to pass on the default container.
3. Correct only documented command shapes proven wrong by the live server; rerun until green.
4. Commit as `test(functional): cover weekly status collection workflow`.

Acceptance: a real Bugzilla supplies every deterministic primitive the skill claims; credentialless
behavior is covered where the phase's public server supports the operation.

## Task 3: Record the two-run demonstration

Files: modify `tools/record-demo.sh`; add the generated cast/GIF or linked documentation asset
required by repository convention.

Interfaces: the demo reuses Task 2's seeded query and fields, invokes the installed skill's documented
baseline and comparison flow, and preserves the current README demo behavior.

Steps:

1. Add a recording mode that creates a baseline, changes seeded Bugzilla state through existing CLI
   commands, then produces the second Markdown comparison.
2. Run the recording script against a fresh functional container; expect both report runs and render
   to succeed.
3. Inspect the generated animation for readable commands, baseline notice, detected changes, and no
   credential or temporary-path disclosure.
4. Commit as `docs(skills): demonstrate weekly status comparisons`.

Acceptance: the repository contains a reproducible asciinema baseline/comparison demonstration.

## Task 4: Final verification and cleanup

Files: all files changed above; update the explicit embedded-skill name assertion in
`src/skills/embedded_tests.rs` if its failure proves it is required.

Steps:

1. Run `make skills-test`, `make lint`, and `make test`; expect exit 0 with no warnings.
2. Run `make functional-test-all`; expect every supported Bugzilla version green.
3. Review `git diff main...HEAD` for scope, generated files, secrets, stale version claims, and
   undocumented commands. Remove any speculative surface and commit cleanup separately.

Acceptance: all guardrails pass, the diff stays within the frozen charter, and no deferred work is
hidden.
