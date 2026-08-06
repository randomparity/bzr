# Agent Skills for bzr

Installable agent skills that teach AI coding agents (Claude Code, Codex, IBM
Bob, and other agents that read `~/.agents/skills`) how to drive the
[`bzr`](https://github.com/randomparity/bzr) Bugzilla CLI correctly. These ship
inside the `bzr` repo so they stay in lockstep with the CLI they document:
`agent-skills/VERSION` matches the crate version in `Cargo.toml`, as does every
"authored against `bzr` X.Y.Z" claim in `content/skills/`. `make skills-test` enforces
that (`tests/version-check.sh`), so refreshing the skills for a new CLI surface
and bumping the version are one change, not two.

## Skills

| Skill | Purpose |
|-------|---------|
| `bzr-reference` | Foundational reference: command surface, `--json`, auth, rules |
| `bzr-setup` | Configure a Bugzilla server and credentials |
| `bzr-file-bug` | File a well-formed bug |
| `bzr-triage-bug` | Read-before-write triage of an existing bug |
| `bzr-search-report` | Search and build a digest with `--json` + `jq` |
| `bzr-bulk-triage` | Stream a query and mutate many bugs safely (preview before write) |

## Install

With `bzr` installed, use the offline payload embedded in that exact binary release.
Choose the destination scope explicitly:

```
bzr skills install --agent all --global
bzr skills install --agent all --project .
```

`standard`/`bob`/`codex` install to `.agents/skills`; `claude` installs to
`.claude/skills`; `all` installs both. Global scope places those layouts below the
current user's home. Project scope places them below the supplied existing directory;
`.` means the current repository. The binary refuses foreign same-named skill folders
and symlinked destination components, and re-running it updates managed folders.

Without `bzr`, install straight from GitHub with the standalone installer:

```
curl -fsSL https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.sh | sh -s -- --agent all
```

Windows (PowerShell):

```
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.ps1))) -Agent all
```

When run without the repository's canonical `content/skills/` directory (the piped
case), the standalone installer downloads the skill payload from `main`. Override the
source with `BZR_SKILL_REF` (a branch, tag, or commit) or `BZR_SKILL_TARBALL_URL` (a
full tarball/zip URL, or a local path for offline installs). This path may provide a
newer or explicitly pinned payload; unlike `bzr skills install`, it is not tied to the
version of a locally installed binary.

From a clone of this repo, run the installer in this directory instead:

```
cd agent-skills
./install.sh --agent all        # ~/.agents/skills and ~/.claude/skills
```

Standalone targets: `standard`/`bob`/`codex` → `~/.agents/skills`, `claude` →
`~/.claude/skills`, `all` → both. Run with no `--agent` to be prompted (a
terminal is required; the piped form must pass `--agent`). Other flags:
`--dry-run`, `--force` (overwrite a foreign same-named folder), `--uninstall`,
`--list`. Windows: `install.ps1` with the same options.

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
`BZR_BIN` errors rather than silently skipping), enforces the version contract
above, exercises the installer's guards hermetically, and lints all shell
sources. The command surface is authored
against `bzr` 0.8.2-dev. The installer copies whole skill folders, marks each
with a `.bzr-skill-managed` sentinel so it only ever touches its own folders, and
refuses to write through a symlinked destination or overwrite an unmarked
("foreign") folder without `--force`.
