# ADR 0021: Contributor guidance lives in `CONTRIBUTING.md`

## Status

Accepted

## Context

The repository exposes development commands in `Makefile`, basic test commands in
`README.md`, and vulnerability reporting in `SECURITY.md`, but it has no single
contributor-facing policy. The policy needs to be discoverable without turning the
product README into a second development manual or duplicating security instructions.

## Decision

Keep contributor workflow policy in a root-level `CONTRIBUTING.md` and link it from
`README.md`. The document names the repository's current setup and verification commands,
describes branch, commit, and pull-request expectations, and links to authoritative
specialized documents rather than copying their contents.

## Consequences

- Contributors have one conventional entry point for development expectations.
- `README.md` remains focused on installing and using `bzr`.
- Changes to development tooling must update `CONTRIBUTING.md` when they alter the
  documented workflow.
- Specialized policies such as vulnerability reporting retain one authoritative owner.

## Considered & rejected

- **Put the full policy in `README.md`.** judgment: it mixes contributor workflow with
  product usage and makes both harder to scan.
- **Add templates and a code of conduct in the same change.** judgment: those are separate
  governance and automation surfaces not required by issue #544.
- **Leave guidance distributed across existing files.** verified: issue #544 identifies
  the missing contributor entry point, and `rg --files -g 'CONTRIBUTING*'` at commit
  `3cbd2004cce0d207272f5e254c07ef8bd8b9bdd7` returned no file.
