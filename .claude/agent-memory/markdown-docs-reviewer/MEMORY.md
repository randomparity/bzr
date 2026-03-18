# markdown-docs-reviewer — Project Memory

## Project: bzr (Bugzilla CLI, Rust)

### Repository structure
- Root docs: `README.md`, `CLAUDE.md`, `AGENTS.md`. `/docs/bzr-cli.md` exists (CLI reference).
- Agent definitions: `.claude/agents/*.md`
- No `CONTRIBUTING.md` exists yet.

### Documentation conventions observed
- Heading capitalization: README uses Title Case; CLAUDE.md uses Sentence case. `docs/bzr-cli.md` uses Title Case.
- Bullet marker: `-` used throughout all docs.
- Code block language hints: `bash`, `toml` used in README and bzr-cli.md; bare fences are a recurring gap to watch.
- Em dash ` -- ` (double hyphen) used as subcommand descriptor separator in bzr-cli.md headings (e.g. `## \`bzr bug\` -- Bug Operations`).

### Flag table style in bzr-cli.md
- 4-col table `| Option | Required | Default | Description |` — commands with optional flags that have defaults.
- 3-col table `| Option | Required | Description |` — commands with required and optional flags, no notable defaults.
- 2-col table `| Flag | Description |` — boolean flags only (no argument value). Introduced with `--details`.
- POTENTIAL INCONSISTENCY: Global Options table uses `| Option | Description |` (2-col), not `| Flag | Description |`.
  For consistency, consider standardizing to `| Option | Description |` everywhere for 2-col boolean-only tables.

### Known recurring gaps
- New CLI flags added to `src/cli.rs` are NOT automatically reflected in:
  1. The Command Tree (fenced text block) in `docs/bzr-cli.md`
  2. The per-command flag/option table in `docs/bzr-cli.md`
  - `user search --details` on feat/user-details branch: added to the example block and flag table,
    but MISSING from the Command Tree at line 101 of `docs/bzr-cli.md`. This is the canonical gap to watch.
- Also check README.md when new top-level commands are added.

### Key accuracy items to check on each review
- `rust-version` in `Cargo.toml` vs "Rust 1.70+" in README — needs verification match.
- Auth description in README ("never in query strings") is misleading; query-param is a real fallback.
- `safe_url()` exists in `src/client.rs` — confirmed accurate in CLAUDE.md Conventions.

### AGENTS.md vs CLAUDE.md distinction
- Both files exist at root. CLAUDE.md is Claude Code guidance; AGENTS.md is for other agent tooling.
- AGENTS.md intro should use generic language ("AI coding agents"), not Claude-specific.

### Commands present in codebase (verify against cli.rs on each review)
- All commands in bzr-cli.md: `bug`, `comment`, `attachment`, `product`, `field`, `user`, `group`,
  `whoami`, `server`, `classification`, `component`, `config`
- `user search` has `--details` flag (feat/user-details branch)
- `shared.rs` — provides `build_client()` helper, not a command

### --details flag behavior (verified in output.rs, user.rs, cli.rs)
- Boolean flag (no argument value) on `user search`.
- Table mode: switches from compact 3-col to 5-col DetailedUserRow (id, name, real_name, email, can_login, groups).
- JSON mode: `--details` has NO effect — full BugzillaUser struct is always serialized.
- Doc should note that `--details` only affects table output; JSON always includes all fields.

### Output format resolution (verified in main.rs)
Precedence: `--json` > `--output` > `BZR_OUTPUT` env > auto-detect (JSON when stdout is not a TTY, table otherwise)
- `--no-color` calls `colored::control::set_override(false)`.
- `colored` v3.1.1 natively reads `NO_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE`.
- `--quiet` redirects stdout fd to /dev/null via dup2.

### Exit codes (verified in src/error.rs)
- 0=success, 1=Other, 2=CLI usage (clap), 3=Config/TOML, 4=Api, 5=Http, 6=Io

### Mutation JSON responses (verified in output.rs + command files)
- Common shape: `{"id": N, "resource": "bug", "action": "created"}`.
- group_membership: `{"user": "...", "group": "...", "resource": "group_membership", "action": "added/removed"}`
- attachment download: includes `"file"` and `"size"` fields.
- comment tag: uses `"comment_id"` key, not `"id"`.

### JSON error format (verified in main.rs)
`{"error":{"type":"api","message":"...","exit_code":4}}` — emitted to stderr when format==Json.
