# Post-release bundled-skill version bump design

**Issue:** [#521](https://github.com/randomparity/bzr/issues/521)
**Decision:** [ADR 0017](../../adr/0017-post-release-skill-version-update-stays-inline.md)

## Scope charter

- **Interaction:** interactive
- **Scope identity:** `https://github.com/randomparity/bzr/issues/521`
  `scope-521-20260805-a1f6`
- **Outcome:** The stable post-release bump job updates every bundled-skill version stamp
  together with Cargo metadata.
- **Completion criteria:** The generated branch updates `agent-skills/VERSION` and all four
  required authored-against claims, stages those files, runs the existing version-contract
  check before commit or push, and passes `make skills-test`.
- **Provenance:** Issue #521 supplies the outcome and criteria. Linked PR #520 and
  `agent-skills/tests/version-check.sh` corroborate the failure and required sites.
- **Exclusions:** No change to patch-bump policy, bundled-skill behavior or content,
  version-contract semantics, or unrelated release jobs.
- **Surface:** `.github/workflows/release.yml` and directly coupled workflow/version-contract
  tests or documentation needed to prove the change.
- **Ambiguities:** none.

## Context

After a stable tag, `post-release-bump` computes the next patch development version, updates
`Cargo.toml` and `Cargo.lock`, prepares an Unreleased changelog section, commits, pushes, and
opens a pull request. The bundled skills are contractually locked to the crate version, but
the job neither updates nor stages their five version sites. PR #520 demonstrated the result:
the generated commit needed manual repairs before its skill checks could pass.

## Approaches

1. **Inline a bounded mutation step in `release.yml` (selected).** An explicit file list and
   exact replacement counts keep the job self-contained and fail when claim structure drifts.
   The existing checker remains the contract authority.
2. **Add a repository updater script.** This is independently testable but adds a new one-use
   maintenance surface and splits post-release sequencing across files.
3. **Use broad `sed` replacements across `agent-skills/`.** This is compact but can change
   version-like historical prose outside the authored-against contract.

ADR 0017 records why the first approach best fits the repository's preference for existing
entry points and explicit behavior.

## Design

Add an `Update bundled-skill versions` step after the Cargo version update. It receives
`NEXT` through the step environment and runs Python 3, already used elsewhere in
`release.yml`.

The step writes `NEXT` plus a newline to `agent-skills/VERSION`. For each of the four files
named by the current version contract, it replaces the semantic-version token following the
bounded `authored against ... bzr` phrase. Each file must produce exactly one replacement;
zero or multiple replacements terminate the job with the affected path. Literal replacement
through a callback prevents version text from being interpreted as a regular-expression
backreference.

After `Cargo.lock` and `CHANGELOG.md` are refreshed, a separate `Verify bundled-skill version
contract` step runs `sh agent-skills/tests/version-check.sh`. It precedes `Commit and push
branch`, so drift cannot create remote state. The commit step explicitly stages the five
skill-version paths.

## Failure behavior

- A missing or reworded required claim fails during the update step before staging.
- A duplicate matching claim fails rather than updating an ambiguous file.
- Any remaining mismatch fails the existing version checker before Git configuration,
  branch creation, commit, or push.
- A rerun still stops at the existing remote-branch guard; this change does not alter release
  retry semantics.

## Threat model

### Boundary inventory

- **Existing boundary widened:** a tag-triggered GitHub Actions job uses `contents: write` and
  `pull-requests: write` to create a branch and pull request. The new version text enters this
  existing mutation path through `NEXT`.
- **New boundary:** none. No new trigger, permission, secret, network destination, or caller is
  introduced.

### Actor model and trust

The relevant untrusted input is the tag ref that triggers the workflow. GitHub supplies the
checked-out repository and runner environment; the job already trusts the protected release
process to create stable tags. Repository contributors can propose workflow or claim-text
changes only through the normal review path.

### Controls

- `NEXT` remains derived by the existing numeric major/minor/patch parser. The Python step
  treats it as data and writes only the five explicit paths.
- Exactly-one replacement checks bound mutation and make claim drift visible.
- The existing version-contract checker validates the final local tree before the first new
  external write. The existing remote-branch check prevents overwriting a prior bump branch.
- Failure messages identify the file whose contract shape changed and do not expose secrets.

### Explicitly out of scope

This change does not harden tag creation, change workflow permissions, make the release job
idempotent after a branch is pushed, or validate the semantic meaning of skill content. Those
concerns are unchanged and are not required to repair the version-stamp drift in issue #521.

## Verification

Add `agent-skills/tests/post-release-bump-workflow-test.sh` and invoke it from the existing
agent-skills test runner. The regression test reads `release.yml`, extracts the actual shell body
of `Update bundled-skill versions`, and executes that body in temporary fixtures rather than
copying its mutation logic into the test.

The test first fails on the current workflow because the named update step is absent. After the
workflow change, it proves:

- a normal fixture updates `agent-skills/VERSION` and all four claims to a synthetic development
  version;
- a fixture with one missing claim fails and names that file;
- a fixture with a duplicate claim fails and names that file;
- the commit step stages all five skill-version paths; and
- `Verify bundled-skill version contract` occurs after metadata mutation but before `Commit and
  push branch`.

The existing version checker then validates the successful fixture and the real checkout. This
keeps the test coupled to the shipped workflow body without creating a supported updater entry
point.

Run these repository guardrails before committing implementation:

- `actionlint .github/workflows/release.yml`
- `zizmor .github/workflows/release.yml`
- `sh agent-skills/tests/post-release-bump-workflow-test.sh`
- `make skills-test`
- `make lint`
- `cargo test --locked --features test-helpers`
- `make functional-test` (required for an internal workflow change before opening the PR)

The ADR index is not coupled to an individually hard-gated CI check; this solo branch updates
the index only as repository documentation.

## Durable workflow state

- Branch: `feat/update-bundled-skill-versions-521`
- Base branch: `main`
- Current phase: design
- Open findings: none before adversarial review
