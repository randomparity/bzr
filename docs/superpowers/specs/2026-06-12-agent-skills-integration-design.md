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
drift check against the **locally built** binary in CI — catches drift on the same
PR that changes the surface.

Be precise about *which* drift fails the build, because the inherited check is
asymmetric (see "Drift Gate Coverage"): a **removed or renamed** documented verb
fails CI (the manifest references a verb the binary no longer has), but an
**added** verb only warns, and a brand-new top-level command group is not examined
at all. So this gate's hard guarantee is "the skills never document a verb that no
longer exists." Keeping the skills current as the surface *grows* is a softer,
warning-driven, periodic task — not enforced by a red build.

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

The installer and the five skill folders are copied **verbatim** from
`bzr-skill` — they are already complete and CI-green in the standalone repo. Three
kinds of edits are made on top of the copy: the version re-pin (below), the
README path fixes, and the drift-gate fail-closed change — which touches
`drift-check.sh`, its self-test `drift-check-test.sh`, and `run.sh` (see "Drift
Gate Must Fail Closed"). `install.sh`, `install.ps1`, and every `SKILL.md` except
the re-pinned strings are unchanged.

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

## Drift Gate Must Fail Closed

The integration's core promise — surface drift becomes a build failure — only
holds if the drift check cannot pass by *skipping*. As copied from `bzr-skill`,
`drift-check.sh` runs `command -v "$BZR"` and, on miss, prints `skip` and exits
`0`; `run.sh` invokes it with no `BZR_BIN`, falling back to whatever `bzr` is on
`PATH` (or skipping entirely). That makes the gate silently bypassable: on a dev
box without `bzr` on `PATH`, or in CI if the build step is reordered or `BZR_BIN`
is mistyped, the suite reports green with the drift check never run.

This integration changes that behavior so the gate is enforced, not optional:

- **Strict mode in `drift-check.sh`.** When `BZR_BIN` is set but does not resolve
  to an executable, the check **errors (exit non-zero)** instead of skipping. The
  bare skip-on-missing path remains only for the case where `BZR_BIN` is unset
  *and* no `bzr` is on `PATH` (e.g. an unrelated contributor running the suite by
  hand) — and even that prints a visible `SKIPPED (no binary)` line, not a silent
  pass.
- **`drift-check-test.sh` is updated to the new contract.** The copied self-test
  currently asserts `BZR_BIN=<nonexistent>` → exit 0 + "skip". That assertion is
  **replaced**, not retained, because it now encodes the bypass this section
  removes. The strict mode needs two test arms: (a) `BZR_BIN=<nonexistent>` →
  exit **non-zero** (fail-closed); (b) `BZR_BIN` unset with a stripped `PATH` →
  exit 0 with a visible `SKIPPED` line. Both arms must be covered so neither the
  fail nor the legitimate-skip path regresses. This is why `drift-check-test.sh`
  appears in the edit list above.
- **`BZR_BIN` is wired through both entry points.** The `make skills-test` target
  (below) first builds the binary (`cargo build`) and exports
  `BZR_BIN=$PWD/target/debug/bzr`; the CI job does the same. Local and CI runs are
  therefore identical, and the drift check runs for real in both — the skip path
  is reachable only when the binary genuinely cannot be built.

The skip arm is deliberately narrow but not airtight: if `BZR_BIN` is unset *and*
a stray, different-version `bzr` happens to be on `PATH` (e.g. a system install),
the check runs against that binary and may emit spurious ERRORs or mask real
drift. The canonical entry points (`make skills-test`, CI) avoid this entirely by
always pinning `BZR_BIN` to the freshly built binary — so those are the
authoritative runs, and a bare `sh run.sh` with no `BZR_BIN` is best-effort only.

Acceptance for this change: with `BZR_BIN` pointing at a path that does not exist,
`sh agent-skills/tests/drift-check.sh` exits non-zero (not 0); with it pointing at
the freshly built binary, it exits 0 with no ERROR lines; and `drift-check-test.sh`
covers both the fail-closed and legitimate-skip arms.

### Drift Gate Coverage

The inherited `drift-check.sh` is asymmetric, and this integration does **not**
change that (the drift mechanism is carried over as-is; only its skip/fail
behavior is hardened above):

- **Fails the build (exit 1):** a verb listed in `commands.yml` that the binary no
  longer reports — i.e. a removed or renamed command. This is the enforced
  guarantee.
- **Warns only (exit 0):** a real verb the binary reports that the manifest does
  not list — i.e. a newly added verb under an existing group.
- **Not detected at all:** an entirely new top-level group, because the check
  iterates only the groups present in `commands.yml` and never enumerates the
  binary's full group list.

Consequence: the gate guarantees the skills never document a command that has
disappeared, but it does **not** guarantee the skills keep pace with *additions*.
Catching new commands/groups stays a manual, warning-driven, periodic task.
"Detect added commands and new top-level groups (enumerate the binary's group
list and diff against the manifest)" is recorded under Out of Scope as a known
limitation, not an implied capability of this integration.

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

A dedicated workflow `.github/workflows/agent-skills.yml`, separate from the Rust
`ci.yml` (so it never slows it) but **path-filtered to both the skills and the
source that defines the command surface**:

```
on:
  push: { paths: [ "agent-skills/**", "src/**", "Cargo.toml", "Cargo.lock", ".github/workflows/agent-skills.yml" ] }
  pull_request: { paths: [ same as above ] }
```

The surface-source paths are load-bearing, not incidental. The command surface
lives in `src/cli/**` (and the rest of `src/**`), **not** in `agent-skills/`. A
filter of `agent-skills/**` alone would mean a PR that renames or removes a `bzr`
verb — the exact drift this gate exists to catch — never triggers the workflow,
and the drift would surface only later on some unrelated PR that happens to touch
`agent-skills/`, bisecting to the wrong change. Including `src/**` and the cargo
manifests makes the Motivation true: surface drift fails CI on the PR that
introduces it. Pure-docs PRs (e.g. editing `README.md` only) still skip the
workflow, so unrelated work is not slowed. The only added cost is one debug
`cargo build` on code PRs, in a workflow that runs alongside — not in front of —
`ci.yml`.

Steps:
1. `actions/checkout` (SHA-pinned, `persist-credentials: false`).
2. Install `shellcheck` and `shfmt`. The `ubuntu-latest` runner ships `pwsh`,
   which the `install.ps1` smoke test needs (see Testing); the workflow asserts
   `command -v pwsh` up front so a runner image that ever drops PowerShell fails
   loudly instead of silently skipping the only `install.ps1` coverage.
3. `cargo build` to produce `target/debug/bzr` (the drift check needs a real
   binary; building locally is the point of co-location).
4. `BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/run.sh` — runs
   frontmatter validation, the drift check against the local binary (fail-closed
   per "Drift Gate Must Fail Closed"), the installer + ps1 self-tests, and
   `shellcheck`/`shfmt` lint.

This **replaces** `bzr-skill`'s own `ci.yml`, which installed a pinned `bzr` from
crates.io; in-repo we build the binary under test instead.

### Branch-protection stance

`main` enforces strict required status checks. A path-filtered workflow that does
**not** trigger never reports a status, so GitHub leaves it "Expected — waiting"
forever. The widened trigger above still does **not** fire on every PR — a
pure-docs PR (README-only, say) skips it — so the deadlock hazard is real.
Therefore `agent-skills.yml` is **not** added to the repo's required status
checks: it gates merges only via the normal PR-checks UI on PRs whose paths match
the trigger. This stays correct precisely *because* the trigger is selective; do
not "fix" the selectivity by making the check required, which would hang every
PR that doesn't match the filter. If a future maintainer genuinely wants it
required for *all* PRs, the correct implementation is an always-triggered job with
an internal path-change gate (e.g. `dorny/paths-filter`) that reports success on a
no-op — **not** a top-level `on.*.paths` filter plus a required-check setting.
This decision is recorded here so it is not re-litigated by whoever next edits
branch protection.

## Repo Touch-Ups

- **README.md** "Agent Integration" section: rewrite to describe the
  `agent-skills/` directory and its installer. Drop the inline `~/.claude/skills/`
  example tree and the `docs/bob-skills.md` link. Point Claude Code, Bob, Codex,
  and standard-agent users at `agent-skills/install.sh`.
- **Makefile**: add a `skills-test` target that builds the binary and runs the
  suite with `BZR_BIN` wired to it, for true parity with CI:
  `cargo build && BZR_BIN="$$PWD/target/debug/bzr" sh agent-skills/tests/run.sh`.
  Running the suite *without* a resolvable `BZR_BIN` must not silently skip the
  drift check (see "Drift Gate Must Fail Closed").
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
- `install.ps1` is exercised only by `installer-ps1-test.sh`, which requires
  `pwsh`. The CI workflow asserts `pwsh` is present (step 2) so this coverage
  cannot vanish silently; a contributor running the suite locally without `pwsh`
  sees an explicit `pwsh not found; skipping` line. The accepted gap: outside CI,
  `install.ps1` is unverified when `pwsh` is absent.
- Acceptance:
  - `make skills-test` exits 0 with **no drift ERROR lines** against the locally
    built `bzr`, and `shellcheck`/`shfmt -i 2 -d` are clean on all shell sources
    under `agent-skills/`.
  - **Fail-closed proof:** running `drift-check.sh` with `BZR_BIN` set to a
    non-existent path exits **non-zero** (the gate cannot be passed by skipping).

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
- Detecting *added* commands and new top-level groups in the drift check
  (enumerate the binary's full group list and diff it against `commands.yml`,
  escalating new-group/new-verb from warning to a stronger signal). Today the gate
  only fails on removed/renamed documented verbs; closing the addition side is a
  known limitation deferred here (see "Drift Gate Coverage").
