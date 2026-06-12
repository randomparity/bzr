# Integrate Agent Skills + Installer into the bzr Repo

**Date:** 2026-06-12
**Status:** Design (approved; awaiting spec review)
**Repo:** https://github.com/randomparity/bzr
**Source of content:** the standalone `bzr-skill` repo (`~/src/bzr-skill`)

## Summary

Move the agent skills and their runtime-free installer out of the separate
`bzr-skill` repo and into the `bzr` repo itself, so the skills stay in lockstep
with the CLI they document. The whole skill artifact (five `SKILL.md` folders, a
POSIX `sh` installer with Windows parity, and the shell test suite) lands in a
single self-contained `agent-skills/` subdirectory. The prose skill guides
(`docs/skills.md`, `docs/bob-skills.md`) are deleted and replaced by this
installable model.

## Motivation

The standalone repo drifts: when `bzr`'s command surface changes, nothing forces
the skills to follow. Co-locating the skills with the source — and running their
drift check against the **locally built** binary in CI — turns surface drift into
a build failure on the same PR that changes the surface.

## Goals

- A single, self-contained home for the skills + installer that does not collide
  with the existing root binary installer.
- The command-surface drift check runs in CI against the binary built from this
  repo, not a crates.io download.
- Replace the prose skill docs with installable, validated `SKILL.md` artifacts.
- No new language runtime, no registry — POSIX shell installer, copied from a
  clone of this repo.

## Non-Goals

- No `curl … | sh` bootstrap for the skill installer (the existing root binary
  installer keeps that path for the binary; the skill installer is clone-and-run).
- No new `bzr` subcommand (`bzr skills install`) and no MCP server.
- No porting of the prose-only skills that the curated set subsumes
  (investigate, bug-summary, review, admin, saved-queries, sprint-report,
  review-queue). The five curated skills are the whole surface.
- No preservation of the `bzr-skill` git history; files are copied verbatim.

## Key Constraint: Installer Name Collision

The `bzr` repo **already ships `install.sh` and `install.ps1` at the repo root** —
they install the `bzr` *binary* (`curl -fsSL …/install.sh | sh`, pulling release
tarballs from GitHub). The `bzr-skill` installer is *also* named
`install.sh`/`install.ps1` but copies markdown skill folders into agent skill
directories. Same names, opposite jobs.

Resolution: the skill installer lives **inside** `agent-skills/`, keeping its
own name there. The root installer is never touched, and there is no ambiguity
about which `install.sh` does what — the path disambiguates them.

## Repository Layout

```
bzr/
├── install.sh                      # UNCHANGED — bzr binary installer
├── install.ps1                     # UNCHANGED — bzr binary installer (Windows)
├── agent-skills/                   # NEW — self-contained skill artifact
│   ├── README.md                   # adapted from bzr-skill README (in-repo paths)
│   ├── VERSION                     # skill-set version (0.1.0), distinct from crate version
│   ├── install.sh                  # skill installer (POSIX sh, set -eu)
│   ├── install.ps1                 # Windows parity
│   ├── skills/
│   │   ├── bzr-reference/
│   │   │   ├── SKILL.md
│   │   │   └── reference/
│   │   │       ├── commands.md
│   │   │       ├── commands.yml
│   │   │       └── json-recipes.md
│   │   ├── bzr-setup/SKILL.md
│   │   ├── bzr-file-bug/SKILL.md
│   │   ├── bzr-triage-bug/SKILL.md
│   │   └── bzr-search-report/SKILL.md
│   └── tests/
│       ├── lib.sh
│       ├── validate-skills.sh
│       ├── validate-skills-test.sh
│       ├── drift-check.sh
│       ├── drift-check-test.sh
│       ├── installer-test.sh
│       ├── installer-ps1-test.sh
│       └── run.sh
├── .github/workflows/agent-skills.yml   # NEW — path-filtered skill CI
└── docs/                           # docs/skills.md + docs/bob-skills.md DELETED
```

The installer, tests, and the five skill folders are copied **verbatim** from
`bzr-skill` — they are already complete and CI-green in the standalone repo. The
only content edits are the version re-pin (below) and the README path fixes.

## Versioning Model

Two independent version concepts, kept separate:

- **`agent-skills/VERSION`** — the skill-set's own version (starts at `0.1.0`).
  The installer stamps it into each `.bzr-skill-managed` sentinel and `--list`
  compares it for staleness. It does **not** track the crate version.
- **bzr-surface pin** — the `bzr` version the command surface was authored and
  verified against. Recorded as a content fact in
  `skills/bzr-reference/reference/commands.{md,yml}` and the `SKILL.md` "authored
  against bzr X.Y.Z" lines. Re-pinned from `0.4.2` to **`0.4.4`** (latest stable
  release; the repo is at `0.4.5-dev`).

Re-pin work:
- Update every "0.4.2" reference in `agent-skills/skills/**` to `0.4.4`.
- Reconcile `commands.yml` against the **locally built** binary so the drift
  check produces **no ERROR lines** (a listed verb the binary lacks is an error;
  a binary verb not listed is an acceptable warning). Build with `cargo build`
  and run `BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/drift-check.sh`.

## Installer Behavior (unchanged from bzr-skill)

The installer's design is carried over wholesale and is **not** redesigned here.
For the full contract see the upstream design doc; the load-bearing properties
that the in-repo tests must keep proving:

- Target map: `standard`/`bob`/`codex` → `~/.agents/skills`, `claude` →
  `~/.claude/skills`, `all` → both.
- `.bzr-skill-managed` ownership sentinel; the installer only ever replaces or
  removes folders carrying it.
- Foreign-folder guard (refuse to overwrite an unmarked same-named folder;
  `--force` overrides only this).
- Symlink guard (refuse unconditionally; `--force` does **not** override — it is a
  home-directory-escape boundary).
- Same-filesystem staging + atomic rename; rename-aside replace/uninstall so a
  mid-failure never half-installs or half-strips.
- `mkdir`-based per-root lock with dead-PID stale recovery.
- `--dry-run`, `--list` (present/absent/shadowed/stale), `--uninstall`.
- `BZR_SKILL_DEST_ROOT` override (default `$HOME`) so all installer tests are
  hermetic and never touch the real home directory.
- Non-fatal `bzr` presence probe on install.

POSIX `#!/bin/sh` + `set -eu` is retained deliberately: it matches the repo's
existing root `install.sh` and the Windows-parity goal, and is portable beyond
bash.

## CI Integration

A dedicated workflow `.github/workflows/agent-skills.yml`, **path-filtered** to
`agent-skills/**` and the workflow file itself, so it never slows the Rust
`ci.yml` and only runs when the skills change.

Steps:
1. `actions/checkout` (SHA-pinned, `persist-credentials: false`).
2. Install `shellcheck` and `shfmt`.
3. `cargo build` to produce `target/debug/bzr` (the drift check needs a real
   binary; building locally is the point of co-location).
4. `BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/run.sh` — runs
   frontmatter validation, the drift check against the local binary, the
   installer + ps1 self-tests, and `shellcheck`/`shfmt` lint.

This **replaces** `bzr-skill`'s own `ci.yml`, which installed a pinned `bzr` from
crates.io; in-repo we build the binary under test instead.

## Repo Touch-Ups

- **README.md** "Agent Integration" section: rewrite to describe the
  `agent-skills/` directory and its installer. Drop the inline `~/.claude/skills/`
  example tree and the `docs/bob-skills.md` link. Point Claude Code, Bob, Codex,
  and standard-agent users at `agent-skills/install.sh`.
- **Makefile**: add a `skills-test` target running `sh agent-skills/tests/run.sh`
  for local parity with CI.
- **CHANGELOG.md**: add an entry under the existing `0.4.5-dev` section noting the
  bundled agent skills + installer (changelog written as the work lands, per repo
  convention).
- **Delete** `docs/skills.md` and `docs/bob-skills.md`; remove their links from
  `README.md`. Confirm no other doc references them.
- **Leave `.claude/skills/` untouched** — those are bzr *development* skills
  (desloppify, gh-issue, gh-pr), unrelated to these *usage* skills.

## Testing & CI

- `agent-skills/tests/run.sh` is the single entry point, run identically locally
  (`make skills-test`) and in CI.
- The full suite copied from `bzr-skill` keeps proving: skill frontmatter
  (name==folder, non-empty `description` ≤ 500 chars, resolving reference links),
  the bidirectional drift check, and every installer guard (foreign-folder,
  symlink, idempotency, lock, uninstall, list-staleness) hermetically via
  `BZR_SKILL_DEST_ROOT`.
- Acceptance: `make skills-test` exits 0 with **no drift ERROR lines** against the
  locally built `bzr`, and `shellcheck`/`shfmt -i 2 -d` are clean on all shell
  sources under `agent-skills/`.

## Risks & Mitigations

- **Surface re-pin reveals real drift between 0.4.2 and 0.4.4.** Mitigation: the
  reconciliation step against the local binary is explicit; any ERROR means the
  manifest must be corrected before merge — exactly the signal co-location is
  meant to surface.
- **Repo-wide tooling sweeping `agent-skills/`.** Mitigation: the shell suite is
  self-contained and hermetic; Rust tooling (clippy, `make check-test-layout`)
  does not touch shell files. If any pre-commit/whole-tree check picks up the
  directory, scope it out with a path glob.

## Out of Scope (future)

- A verified `curl … | sh` bootstrap for the skill installer.
- A `bzr skills install` subcommand or MCP server.
- Re-adding the dropped prose-only skills as installable folders.
