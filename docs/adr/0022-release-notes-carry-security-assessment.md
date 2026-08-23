# ADR 0022: Release notes carry an explicit security assessment

## Status

Accepted

## Context

GitHub releases copy their notes from `CHANGELOG.md`, but neither the changelog format nor the
release workflow requires maintainers to state whether a release fixes a publicly identified
runtime vulnerability in `bzr`. Dependency advisories have appeared under security headings, so
their presence can also be mistaken for disclosure of a project vulnerability.

## Decision

Every release-note section carries exactly one explicit project-vulnerability assessment. A
release that fixes no qualifying vulnerability says so. A release that does lists every
qualifying public identifier known at release-preparation time and records affected `bzr`
versions, the first fixed version, runtime impact, advisory link, and upgrade guidance.

Keep GitHub Security Advisories as the canonical public inventory. Keep dependency-only advisory
notes separate from the project assessment. Validate the assessment in the release workflow
before creating the GitHub Release.

## Consequences

- Users can tell from one release's notes whether updating fixes a known project vulnerability.
- Release preparation includes an explicit review instead of treating silence as “none.”
- Dependency security updates remain visible without satisfying the project-vulnerability rule.
- Automation checks structure and presence, but maintainers remain responsible for completeness
  against the advisory inventory.

## Considered & rejected

- **Rely on free-form security prose.** verified: `CHANGELOG.md` at commit
  `356cf59a5dd4d634c745bc88e03d60b4361a23a8` contains a transitive dependency CVE under a
  `Security` heading, while `.github/workflows/release.yml` only checks that the extracted section
  is non-empty; those facts cannot establish a project-vulnerability assessment.
- **Duplicate a complete advisory inventory in the repository.** judgment: two independently
  maintained inventories would drift, while release notes only need the vulnerabilities fixed by
  that release.
- **Make a pull-request checkbox the only guard.** judgment: a template is useful guidance but
  does not stop a tag-triggered release whose notes omitted the assessment.
- **Query GitHub advisories during the release job.** judgment: network and API state would add a
  failure mode without proving that maintainers correctly analyzed affected versions.
