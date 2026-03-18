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

### Output format resolution (verified in main.rs)
Precedence: `--json` > `--output` > `BZR_OUTPUT` env > auto-detect (JSON when stdout is not a TTY, table otherwise)
- `--no-color` calls `colored::control::set_override(false)` — it does NOT read NO_COLOR itself.
- `colored` v3.1.1 natively reads `NO_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE`. All three are worth documenting.
- `--quiet` redirects stdout fd to /dev/null via dup2 — operates after format resolution.

### Exit codes (verified in src/error.rs)
- 0 = success (implicit)
- 1 = BzrError::Other (general/unknown)
- 2 = CLI usage error (clap's own exit code, not a BzrError variant)
- 3 = Config / TomlParse / TomlSerialize
- 4 = Api
- 5 = Http
- 6 = Io
Note: Exit code 2 for "CLI usage error" is Clap's built-in behavior, not a BzrError variant.

### Mutation JSON responses (verified in output.rs + command files)
- All mutations use `output::print_result()` with a serde_json::Value.
- Common shape: `{"id": N, "resource": "bug", "action": "created"}`.
- group_membership: `{"user": "...", "group": "...", "resource": "group_membership", "action": "added/removed"}`
- attachment download: includes `"file"` and `"size"` fields (not just id).
- comment tag: uses `"comment_id"` key, not `"id"`.
- `config set-server` and `config set-default` include `"config_file"` path in JSON — key feature of this branch.

### JSON error format (verified in main.rs)
`{"error":{"type":"api","message":"...","exit_code":4}}` — emitted to stderr when format==Json.
The `type` field matches `BzrError::error_type()` strings: "config", "api", "http", "io", "other".
