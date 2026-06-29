# Agent Skills for bzr

Installable agent skills that teach AI coding agents (Claude Code, Codex, IBM
Bob, and other agents that read `~/.agents/skills`) how to drive the
[`bzr`](https://github.com/randomparity/bzr) Bugzilla CLI correctly. These ship
inside the `bzr` repo so they stay in lockstep with the CLI they document:
`agent-skills/VERSION` matches the crate version in `Cargo.toml`.

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

Install without a clone, straight from GitHub:

```
curl -fsSL https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.sh | sh -s -- --agent all
```

Windows (PowerShell):

```
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/randomparity/bzr/main/agent-skills/install.ps1))) -Agent all
```

When run without a local `skills/` directory beside it (the piped case) the
installer downloads the skill payload from `main` and installs the extracted
copy. Override the source with `BZR_SKILL_REF` (a branch, tag, or commit) or
`BZR_SKILL_TARBALL_URL` (a full tarball/zip URL, or a local path for offline
installs).

From a clone of this repo, run the installer in this directory instead:

```
cd agent-skills
./install.sh --agent all        # ~/.agents/skills and ~/.claude/skills
```

Targets: `standard`/`bob`/`codex` → `~/.agents/skills`, `claude` →
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
`BZR_BIN` errors rather than silently skipping), exercises the installer's
guards hermetically, and lints all shell sources. The command surface is authored
against `bzr` 0.6.1-dev. The installer copies whole skill folders, marks each
with a `.bzr-skill-managed` sentinel so it only ever touches its own folders, and
refuses to write through a symlinked destination or overwrite an unmarked
("foreign") folder without `--force`.
