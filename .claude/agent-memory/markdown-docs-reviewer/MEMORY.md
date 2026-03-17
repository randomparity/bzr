# markdown-docs-reviewer — Project Memory

## Project: bzr (Bugzilla CLI, Rust)

### Repository structure
- Root docs: `README.md`, `CLAUDE.md`, `AGENTS.md`. No `/docs/` directory exists.
- Agent definitions: `.claude/agents/*.md`
- No `CONTRIBUTING.md` exists yet (as of first review session).

### Documentation conventions observed
- Heading capitalization: README uses Title Case; CLAUDE.md uses Sentence case. Both consistent internally.
- Bullet marker: `-` used throughout all docs.
- Code block language hints: `bash`, `toml` used in README; bare fences are a recurring gap to watch.
- Em dash ` — ` used as module descriptor separator in CLAUDE.md key-modules list.

### Known recurring gaps
- New CLI commands added to `src/commands/` and `src/cli.rs` are NOT automatically reflected in:
  - README.md Command Tree
  - README.md Features list
  - CLAUDE.md Key modules bullet for `commands/`
  Check these three locations whenever a `feat(cli)` commit appears on the branch.

### Key accuracy items to check on each review
- `rust-version` in `Cargo.toml` vs "Rust 1.70+" in README — needs verification match.
- Auth description in README ("never in query strings") is misleading; query-param is a real fallback.
- `safe_url()` exists in `src/client.rs:364` — confirmed accurate in CLAUDE.md Conventions.

### AGENTS.md vs CLAUDE.md distinction
- Both files exist at root. CLAUDE.md is Claude Code guidance; AGENTS.md is for other agent tooling.
- As of feat/integration-testing branch: AGENTS.md heading was changed to `# CLAUDE.md` (wrong)
  and content became identical to CLAUDE.md — a known blocker to fix.
- AGENTS.md intro should use generic language ("AI coding agents"), not Claude-specific.

### markdown-docs-reviewer.md agent definition issue
- File at `.claude/agents/markdown-docs-reviewer.md` has wrong Project Context block.
- Refers to "Video Policy Orchestrator / VPO" — this is the wrong project.
- Rules it specifies (`/docs/`, `INDEX.md`, CONTRIBUTING.md) do not apply to bzr.
- Fix: replace with bzr-specific rules (no /docs/, root-only docs, no INDEX required).

### Commands present in codebase (verify against cli.rs on each review)
- `bug`, `comment`, `attachment`, `config` — documented in README
- `product`, `field`, `user` — added in feat(cli) commit, NOT yet in README as of this branch
- `shared.rs` — provides `build_client()` helper, not a command but part of commands/ module
