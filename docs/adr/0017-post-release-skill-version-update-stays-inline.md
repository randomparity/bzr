# ADR 0017: Post-release skill version updates stay inline

## Status

Accepted

## Context

The stable-release workflow opens a follow-up pull request that advances the crate to the
next patch development version. The bundled agent skills have five version-contract sites:
`agent-skills/VERSION` and four authored-against claims. The workflow currently updates only
Cargo metadata, so its generated pull request violates the existing contract check.

The update can live directly in the release job or behind a new repository script. The
operation has one caller, and the existing `agent-skills/tests/version-check.sh` already owns
contract validation.

## Decision

Keep the mutation in the `post-release-bump` job. Use one bounded Python step with an explicit
list of the four claim files, require exactly one replacement per file, and write
`agent-skills/VERSION` directly. Run the existing version-contract checker after all metadata
updates and before the commit step.

The commit step explicitly stages the five skill-version files alongside the existing Cargo
and changelog files.

## Consequences

- A stable release cannot push its generated bump branch when a required skill claim is
  missing, duplicated, or left at the released version.
- The workflow remains the sole owner of post-release bump sequencing; no one-use helper
  becomes a supported maintenance entry point.
- Adding or moving a required version claim requires updating both the existing contract
  checker and the workflow's explicit mutation list.
- The job continues to rely on Python 3 from the GitHub-hosted runner, which the same workflow
  already uses.

## Considered & rejected

- **Add a reusable skill-version updater script.** This would be easy to unit-test, but it
  creates a new repository entry point for a single workflow caller and duplicates ownership
  with the existing checker.
- **Replace version-looking text through a glob or broad search.** This is shorter, but it can
  rewrite historical or explanatory prose that is not part of the version contract.
- **Update only `agent-skills/VERSION` and let the checker tolerate stale claims.** This keeps
  the original defect: installed reference material would advertise a different CLI surface.
