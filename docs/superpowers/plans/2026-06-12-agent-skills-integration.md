# Agent Skills Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the agent skills and their runtime-free installer from the standalone `bzr-skill` repo into this repo under `agent-skills/`, hardened so command-surface drift fails CI on the PR that introduces it, and replace the old prose skill guides.

**Architecture:** Copy the five `SKILL.md` folders, the POSIX installer (`install.sh`/`install.ps1`), and the shell test suite verbatim into a self-contained `agent-skills/` directory (no collision with the repo's root binary installer). Apply three edit layers on top of the copy: a version re-pin (`0.4.2`→`0.4.4`), a fail-closed change to the drift gate, and repo wiring (a path-filtered CI workflow, a `make skills-test` target, README/CHANGELOG updates, deletion of the old docs).

**Tech Stack:** POSIX `sh`, `shellcheck`, `shfmt -i 2`, `awk`, the locally built `bzr` binary (`cargo build` → `target/debug/bzr`), GitHub Actions. No language runtime for the installer; no `yq` (the manifest is line-based).

**Spec:** `docs/superpowers/specs/2026-06-12-agent-skills-integration-design.md`

**Source of verbatim content:** `~/src/bzr-skill` (a clone on this machine).

---

## File Structure

Files this plan creates or modifies in the `bzr` repo:

- `agent-skills/install.sh` — skill installer (copied verbatim).
- `agent-skills/install.ps1` — Windows parity (copied verbatim).
- `agent-skills/VERSION` — skill-set version `0.1.0` (copied verbatim).
- `agent-skills/README.md` — authored fresh (in-repo paths).
- `agent-skills/skills/bzr-reference/SKILL.md` — copied, then `0.4.2`→`0.4.4`.
- `agent-skills/skills/bzr-reference/reference/commands.md` — copied, then `0.4.2`→`0.4.4`.
- `agent-skills/skills/bzr-reference/reference/commands.yml` — copied, then `0.4.2`→`0.4.4` + group reconcile.
- `agent-skills/skills/bzr-reference/reference/json-recipes.md` — copied verbatim.
- `agent-skills/skills/bzr-setup/SKILL.md` — copied verbatim.
- `agent-skills/skills/bzr-file-bug/SKILL.md` — copied verbatim.
- `agent-skills/skills/bzr-triage-bug/SKILL.md` — copied verbatim.
- `agent-skills/skills/bzr-search-report/SKILL.md` — copied verbatim.
- `agent-skills/tests/lib.sh` — copied verbatim.
- `agent-skills/tests/validate-skills.sh` + `validate-skills-test.sh` — copied verbatim.
- `agent-skills/tests/drift-check.sh` — copied, then fail-closed edit.
- `agent-skills/tests/drift-check-test.sh` — copied, then two-arm contract edit.
- `agent-skills/tests/installer-test.sh` + `installer-ps1-test.sh` — copied verbatim.
- `agent-skills/tests/run.sh` — copied, then observability edit.
- `.github/workflows/agent-skills.yml` — created.
- `Makefile` — add `skills-test` target.
- `README.md` — rewrite the "Agent Integration" section.
- `CHANGELOG.md` — add an entry under `## [Unreleased]`.
- `docs/skills.md`, `docs/bob-skills.md` — deleted.

**Do NOT copy** from `bzr-skill`: `.git`, `.github`, `docs/`, `.gitignore`, `LICENSE` (the repo has its own).

**Branch:** all work happens on `feat/integrate-agent-skills` (already created and checked out). If you are not on it, run `git checkout feat/integrate-agent-skills` first. Never commit on `main`.

---

## Task 1: Copy the verbatim artifact into `agent-skills/`

Brings the installer, skills, and tests into the repo unchanged. No edits yet.

**Files:**
- Create: `agent-skills/{install.sh,install.ps1,VERSION}`, `agent-skills/skills/**`, `agent-skills/tests/**`

- [ ] **Step 1: Confirm the branch**

Run: `git -C "$PWD" rev-parse --abbrev-ref HEAD`
Expected: `feat/integrate-agent-skills`. If not, `git checkout feat/integrate-agent-skills`.

- [ ] **Step 2: Copy the files (excluding repo metadata, docs, LICENSE)**

```bash
SRC="$HOME/src/bzr-skill"
mkdir -p agent-skills
cp -R "$SRC/skills" agent-skills/
cp -R "$SRC/tests" agent-skills/
cp "$SRC/install.sh" "$SRC/install.ps1" "$SRC/VERSION" agent-skills/
chmod +x agent-skills/install.sh
```

- [ ] **Step 3: Verify the tree landed**

Run: `find agent-skills -type f | sort`
Expected (exactly these files):

```
agent-skills/VERSION
agent-skills/install.ps1
agent-skills/install.sh
agent-skills/skills/bzr-file-bug/SKILL.md
agent-skills/skills/bzr-reference/SKILL.md
agent-skills/skills/bzr-reference/reference/commands.md
agent-skills/skills/bzr-reference/reference/commands.yml
agent-skills/skills/bzr-reference/reference/json-recipes.md
agent-skills/skills/bzr-search-report/SKILL.md
agent-skills/skills/bzr-setup/SKILL.md
agent-skills/skills/bzr-triage-bug/SKILL.md
agent-skills/tests/drift-check-test.sh
agent-skills/tests/drift-check.sh
agent-skills/tests/installer-ps1-test.sh
agent-skills/tests/installer-test.sh
agent-skills/tests/lib.sh
agent-skills/tests/run.sh
agent-skills/tests/validate-skills-test.sh
agent-skills/tests/validate-skills.sh
```

- [ ] **Step 4: Sanity-run the copied suite against the local binary**

```bash
cargo build
BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/run.sh
```
Expected: exit 0. The drift check prints only `warn` lines (e.g. `group` verbs `list-users`/`view` not documented) and **no** `ERROR` lines. `installer-ps1-test` runs if `pwsh` is present, else prints a skip line. This proves the copy is wired correctly in the new location before any edits.

- [ ] **Step 5: Commit**

```bash
git add agent-skills
git commit -m "feat(agent-skills): vendor skills + installer from bzr-skill"
```

---

## Task 2: Re-pin the command surface to 0.4.4 and reconcile the manifest

The skills were authored against `bzr 0.4.2`; the repo is past that. Bump the three version strings and silence the two `group` drift warnings by documenting the real verbs. No ERROR lines are expected (the live surface already covers every listed verb).

**Files:**
- Modify: `agent-skills/skills/bzr-reference/SKILL.md`
- Modify: `agent-skills/skills/bzr-reference/reference/commands.md`
- Modify: `agent-skills/skills/bzr-reference/reference/commands.yml`

- [ ] **Step 1: Bump the three `0.4.2` references to `0.4.4`**

Run:
```bash
grep -rn '0\.4\.2' agent-skills/skills/
```
Expected three hits:
- `agent-skills/skills/bzr-reference/reference/commands.yml:1` (`# … authored against bzr 0.4.2.`)
- `agent-skills/skills/bzr-reference/SKILL.md:60` (`This reference is authored against **bzr 0.4.2**.`)
- `agent-skills/skills/bzr-reference/reference/commands.md:1` (`# bzr command surface (authored against bzr 0.4.2)`)

Replace `0.4.2` with `0.4.4` at each. For example with sed:
```bash
sed -i '' 's/0\.4\.2/0.4.4/g' \
  agent-skills/skills/bzr-reference/reference/commands.yml \
  agent-skills/skills/bzr-reference/SKILL.md \
  agent-skills/skills/bzr-reference/reference/commands.md
```
(On Linux use `sed -i` without the `''`.)

- [ ] **Step 2: Reconcile the `group` manifest line**

In `agent-skills/skills/bzr-reference/reference/commands.yml`, the `group` line currently reads:
```
group: add-user remove-user create update
```
The real surface also has `list-users` and `view`, which the drift check flags as warnings. Replace the line with:
```
group: add-user remove-user list-users view create update
```

- [ ] **Step 3: Verify no `0.4.2` remains and the drift check is clean**

```bash
grep -rn '0\.4\.2' agent-skills/skills/ && echo "STILL PRESENT" || echo "clean"
BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/drift-check.sh; echo "exit=$?"
```
Expected: `clean`; drift check `exit=0` with **no** `ERROR` lines and **no** `group` warnings for `list-users`/`view`. (Other groups must remain ERROR-free; if any ERROR appears, the manifest lists a verb the binary lacks — fix that line to match `bzr <group> --help`.)

- [ ] **Step 4: Commit**

```bash
git add agent-skills/skills/bzr-reference
git commit -m "docs(agent-skills): re-pin command surface to bzr 0.4.4"
```

---

## Task 3: Make the drift gate fail closed (TDD)

The copied `drift-check.sh` skips (exit 0) whenever the binary is missing, which lets CI go green with the drift check never run. Change it so a **set-but-unresolvable** `BZR_BIN` errors, while an **unset** `BZR_BIN` with no `bzr` on `PATH` still skips — with a visible notice. Update the self-test to the new contract first (it currently asserts the old skip behavior).

**Files:**
- Modify: `agent-skills/tests/drift-check-test.sh`
- Modify: `agent-skills/tests/drift-check.sh`
- Modify: `agent-skills/tests/run.sh`

- [ ] **Step 1: Rewrite the test's skip case into two arms (write the failing test)**

In `agent-skills/tests/drift-check-test.sh`, replace this block:

```sh
# no bzr available -> skip gracefully, exit 0
out=$(BZR_BIN="$WORK/nope" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "absent bzr skips" "0" "$rc"
assert_contains "absent bzr message" "$out" "skip"
```

with:

```sh
# BZR_BIN set but unresolvable -> fail closed (non-zero), names BZR_BIN.
out=$(BZR_BIN="$WORK/nope" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "set-but-missing BZR_BIN fails closed" "1" "$rc"
assert_contains "fail-closed names BZR_BIN" "$out" "BZR_BIN"

# BZR_BIN unset and bzr not on PATH -> legitimate skip, exit 0, visible notice.
# Build a tool dir with the coreutils drift-check needs but no bzr, then run with
# only that on PATH. BZR_BIN is never set in this test process, so it stays unset.
mkdir -p "$WORK/toolbin"
for t in dirname awk tr cat sh; do
  p=$(command -v "$t" 2>/dev/null) && ln -sf "$p" "$WORK/toolbin/$t"
done
out=$(PATH="$WORK/toolbin" "$DRIFT" "$WORK/commands.yml" 2>&1) && rc=0 || rc=$?
assert_eq "unset BZR_BIN + no PATH bzr skips" "0" "$rc"
assert_contains "skip notice printed" "$out" "SKIPPED"
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `sh agent-skills/tests/drift-check-test.sh`
Expected: FAIL — the old `drift-check.sh` exits 0 for `BZR_BIN="$WORK/nope"` (so "set-but-missing BZR_BIN fails closed" fails), and prints `skip`, not `SKIPPED`.

- [ ] **Step 3: Implement fail-closed in `drift-check.sh`**

In `agent-skills/tests/drift-check.sh`, replace this block:

```sh
if ! command -v "$BZR" >/dev/null 2>&1; then
  printf 'drift-check: bzr not found (%s); skip.\n' "$BZR"
  exit 0
fi
```

with:

```sh
if ! command -v "$BZR" >/dev/null 2>&1; then
  if [ -n "${BZR_BIN:-}" ]; then
    printf 'drift-check: ERROR BZR_BIN set to "%s" but it is not an executable\n' "$BZR" >&2
    exit 1
  fi
  printf 'drift-check: SKIPPED (no binary): BZR_BIN unset and bzr not on PATH\n'
  exit 0
fi
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `sh agent-skills/tests/drift-check-test.sh`
Expected: PASS — `drift-check-test: N run, 0 failed`, exit 0.

- [ ] **Step 5: Add an observability line to `run.sh`**

In `agent-skills/tests/run.sh`, find:

```sh
# Content checks
sh "$HERE/validate-skills.sh" || rc=1
sh "$HERE/drift-check.sh" || rc=1
```

and insert a line directly above it so logs always show whether the gate was wired:

```sh
printf 'run: drift check uses BZR_BIN=%s\n' "${BZR_BIN:-<unset; will try PATH>}"

# Content checks
sh "$HERE/validate-skills.sh" || rc=1
sh "$HERE/drift-check.sh" || rc=1
```

- [ ] **Step 6: Lint the three edited scripts**

Run: `shellcheck agent-skills/tests/drift-check.sh agent-skills/tests/drift-check-test.sh agent-skills/tests/run.sh && shfmt -i 2 -d agent-skills/tests/drift-check.sh agent-skills/tests/drift-check-test.sh agent-skills/tests/run.sh`
Expected: no output, exit 0.

- [ ] **Step 7: Prove fail-closed end to end**

```bash
BZR_BIN="$PWD/does-not-exist" sh agent-skills/tests/drift-check.sh; echo "exit=$?"
```
Expected: prints an `ERROR BZR_BIN set to … not an executable` line and `exit=1` (the gate cannot pass by skipping).

- [ ] **Step 8: Commit**

```bash
git add agent-skills/tests/drift-check.sh agent-skills/tests/drift-check-test.sh agent-skills/tests/run.sh
git commit -m "feat(agent-skills): fail-closed drift gate when BZR_BIN is set but unresolvable"
```

---

## Task 4: Add the path-filtered CI workflow

A dedicated workflow that builds the binary and runs the suite, triggered by changes to the skills **or** the source that defines the command surface.

**Files:**
- Create: `.github/workflows/agent-skills.yml`

- [ ] **Step 1: Write the workflow**

Create `.github/workflows/agent-skills.yml`:

```yaml
name: agent-skills

on:
  push:
    paths:
      - "agent-skills/**"
      - "src/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - ".github/workflows/agent-skills.yml"
  pull_request:
    paths:
      - "agent-skills/**"
      - "src/**"
      - "Cargo.toml"
      - "Cargo.lock"
      - ".github/workflows/agent-skills.yml"

permissions:
  contents: read

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
        with:
          persist-credentials: false
      - name: Install shell tooling
        run: |
          set -eux
          sudo apt-get update
          sudo apt-get install -y shellcheck
          go install mvdan.cc/sh/v3/cmd/shfmt@v3.13.1
          echo "$HOME/go/bin" >> "$GITHUB_PATH"
      - name: Assert pwsh is present
        run: command -v pwsh
      - name: Build bzr
        run: cargo build
      - name: Run agent-skills checks
        run: BZR_BIN="$PWD/target/debug/bzr" sh agent-skills/tests/run.sh
```

- [ ] **Step 2: Lint the workflow**

Run: `actionlint .github/workflows/agent-skills.yml && zizmor .github/workflows/agent-skills.yml`
Expected: no findings. (If `actionlint`/`zizmor` are not installed locally, skip — CI will run them; but prefer running them.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/agent-skills.yml
git commit -m "ci: validate agent-skills against the locally built bzr"
```

---

## Task 5: Add the `make skills-test` target

Local parity with CI: build the binary and run the suite with `BZR_BIN` wired to it.

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Add `skills-test` to `.PHONY`**

In `Makefile`, the `.PHONY` block (starts at line 4) lists targets across several continued lines. Add `skills-test` to it. Change the line:

```make
        build release test coverage fmt clippy lint check-test-layout check-no-spawn clean help man \
```
to:
```make
        build release test coverage fmt clippy lint check-test-layout check-no-spawn clean help man \
        skills-test \
```

- [ ] **Step 2: Add the target**

Add this target after the existing `test:` target (near line 77). Use a literal tab for the recipe lines:

```make
skills-test: ## Build bzr and run the agent-skills shell suite (drift, installer, lint)
	$(CARGO) build
	BZR_BIN="$$PWD/target/debug/bzr" sh agent-skills/tests/run.sh
```

- [ ] **Step 3: Run it**

Run: `make skills-test`
Expected: builds, then the suite exits 0 — drift check runs against `target/debug/bzr` (no ERROR lines), installer tests pass, `shellcheck`/`shfmt` clean. The `run:` line shows `BZR_BIN=…/target/debug/bzr`, confirming the gate is wired (not skipped).

- [ ] **Step 4: Commit**

```bash
git add Makefile
git commit -m "build: add make skills-test target for the agent-skills suite"
```

---

## Task 6: Author `agent-skills/README.md`

The copied installer needs an in-repo README (the upstream one says "git clone bzr-skill").

**Files:**
- Create: `agent-skills/README.md`

- [ ] **Step 1: Write the README**

Create `agent-skills/README.md`:

```markdown
# Agent Skills for bzr

Installable agent skills that teach AI coding agents (Claude Code, Codex, IBM
Bob, and other agents that read `~/.agents/skills`) how to drive the
[`bzr`](https://github.com/randomparity/bzr) Bugzilla CLI correctly. These ship
inside the `bzr` repo so they stay in lockstep with the CLI they document.

## Skills

| Skill | Purpose |
|-------|---------|
| `bzr-reference` | Foundational reference: command surface, `--json`, auth, rules |
| `bzr-setup` | Configure a Bugzilla server and credentials |
| `bzr-file-bug` | File a well-formed bug |
| `bzr-triage-bug` | Read-before-write triage of an existing bug |
| `bzr-search-report` | Search and build a digest with `--json` + `jq` |

## Install

From a clone of this repo, run the installer in this directory and pick a target:

```
cd agent-skills
./install.sh --agent all        # ~/.agents/skills and ~/.claude/skills
```

Targets: `standard`/`bob`/`codex` → `~/.agents/skills`, `claude` →
`~/.claude/skills`, `all` → both. Run with no `--agent` to be prompted. Other
flags: `--dry-run`, `--force` (overwrite a foreign same-named folder),
`--uninstall`, `--list`. Windows: `install.ps1` with the same options.

The skills call the real `bzr` binary — install it from
<https://github.com/randomparity/bzr> if you have not.

## Updating

Pull the repo and re-run `./install.sh`. `./install.sh --list` shows which
installed skills are stale relative to your checkout.

## Development

```
make skills-test    # from the repo root: builds bzr, runs the full suite
```

`make skills-test` validates skill frontmatter, runs the command-surface drift
check against the freshly built `bzr` (it fails closed — a set-but-missing
`BZR_BIN` errors rather than silently skipping), exercises the installer's
guards hermetically, and lints all shell sources. The command surface is authored
against `bzr` 0.4.4. The installer copies whole skill folders, marks each with a
`.bzr-skill-managed` sentinel so it only ever touches its own folders, and refuses
to write through a symlinked destination or overwrite an unmarked ("foreign")
folder without `--force`.
```

- [ ] **Step 2: Verify the skill suite still passes (README adds no code path, but confirm nothing broke)**

Run: `make skills-test`
Expected: exit 0.

- [ ] **Step 3: Commit**

```bash
git add agent-skills/README.md
git commit -m "docs(agent-skills): add in-repo README with install + dev instructions"
```

---

## Task 7: Rewrite the repo README section, delete old docs, add CHANGELOG entry

Replace the prose-guide model with the installable model and remove the dead docs and their links.

**Files:**
- Modify: `README.md` (the "Agent Integration" section)
- Delete: `docs/skills.md`, `docs/bob-skills.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Replace the "Agent Integration" section**

In `README.md`, replace the entire "Agent Integration" section (from the `## Agent Integration` heading through the end of the IBM Bob subsection, i.e. the paragraph ending "…without custom wrappers.") with:

```markdown
## Agent Integration

`bzr` ships a set of installable agent skills under [`agent-skills/`](agent-skills/)
that teach AI coding agents to use the CLI correctly — the `--json` contract,
the authentication model, the read-before-write rule, and the real command
surface. They live in this repo so they track the CLI as it changes (CI runs a
command-surface drift check against the built binary).

Install them from a clone:

```bash
cd agent-skills
./install.sh --agent all     # ~/.agents/skills and ~/.claude/skills
```

`standard`/`bob`/`codex` install to `~/.agents/skills`; `claude` installs to
`~/.claude/skills`; `all` does both. Windows users run `install.ps1`. See
[`agent-skills/README.md`](agent-skills/README.md) for the full skill list,
flags (`--dry-run`, `--list`, `--uninstall`, `--force`), and development notes.

The skills shell out to the real `bzr` binary and are agent-agnostic: global
flags are consistent, machine-readable output is built in, and saved templates
and queries let agents reuse local workflows without custom wrappers.
```

- [ ] **Step 2: Delete the old docs**

```bash
git rm docs/skills.md docs/bob-skills.md
```

- [ ] **Step 3: Confirm no dangling references remain**

Run: `rg -n 'docs/skills\.md|docs/bob-skills\.md|skills\.md|bob-skills' README.md docs/`
Expected: no matches. If any remain (other than historical spec/plan files under `docs/specs`, `docs/plans`, `docs/superpowers` describing this work), update or remove them.

- [ ] **Step 4: Add the CHANGELOG entry under `## [Unreleased]`**

In `CHANGELOG.md`, the top section is `## [Unreleased]` (currently empty, directly above `## [0.4.4] - 2026-06-12`). Replace:

```markdown
## [Unreleased]

## [0.4.4] - 2026-06-12
```

with:

```markdown
## [Unreleased]

### Added

- Bundled agent skills for driving `bzr` from AI coding agents, with a
  runtime-free installer. Five skills (`bzr-reference`, `bzr-setup`,
  `bzr-file-bug`, `bzr-triage-bug`, `bzr-search-report`) live under
  `agent-skills/`; `agent-skills/install.sh` (POSIX) and `install.ps1` (Windows)
  copy selected skills into agent skill directories (`~/.agents/skills`,
  `~/.claude/skills`) with a `.bzr-skill-managed` ownership sentinel. CI runs a
  command-surface drift check against the built binary. Replaces the previous
  `docs/skills.md` and `docs/bob-skills.md` guides.

## [0.4.4] - 2026-06-12
```

- [ ] **Step 5: Commit**

```bash
git add README.md CHANGELOG.md docs/skills.md docs/bob-skills.md
git commit -m "docs: replace prose skill guides with the agent-skills installer"
```

---

## Task 8: Final verification

Confirm the whole integration is green and self-consistent before handing off.

- [ ] **Step 1: Full suite, fail-closed proof, and lint**

```bash
make skills-test
BZR_BIN="$PWD/nonexistent" sh agent-skills/tests/drift-check.sh; echo "fail-closed exit=$?"
shellcheck agent-skills/install.sh agent-skills/tests/*.sh
shfmt -i 2 -d agent-skills/install.sh agent-skills/tests/*.sh
```
Expected: `make skills-test` exits 0 with no drift ERROR lines; `fail-closed exit=1`; `shellcheck`/`shfmt` produce no output.

- [ ] **Step 2: Confirm the root binary installer is untouched**

Run: `git diff --name-only main...HEAD -- install.sh install.ps1`
Expected: no output (the repo-root binary installer was never modified).

- [ ] **Step 3: Confirm the dev skills under `.claude/skills/` are untouched**

Run: `git diff --name-only main...HEAD -- .claude/skills`
Expected: no output.

- [ ] **Step 4: Review the full branch diff**

Run: `git diff --stat main...HEAD`
Expected: only `agent-skills/**`, `.github/workflows/agent-skills.yml`, `Makefile`, `README.md`, `CHANGELOG.md`, the deleted `docs/skills.md`/`docs/bob-skills.md`, and the `docs/superpowers/{specs,plans}` files for this work.

This is the end of the plan. The branch is ready for a PR (use the `gh-pr` skill or the project's PR workflow). Do **not** add `agent-skills.yml` to branch protection's required checks (see the spec's "Branch-protection stance").
```
