---
name: bzr-reference
description: Use when working with Bugzilla via the bzr CLI — viewing, searching, creating, or updating bugs, comments, attachments, products, or saved queries. Covers the command surface, the --json output contract, authentication, and the read-before-write rule.
---

# Using bzr (Bugzilla CLI)

`bzr` is a command-line client for Bugzilla. Use it to view, search, file, and
update bugs and their comments, attachments, flags, and saved queries against a
configured Bugzilla server. It shells out to a real server — there is no local
database.

## Output: prefer `--json`

When you will parse output, pass `--json` (bzr's canonical shorthand;
`--output json` is the long form). Output also auto-flips to JSON when stdout is
piped, but pass `--json` explicitly so intent is clear. Pipe to `jq`:

```
bzr bug view 12345 --json | jq -r '.summary'
bzr bug search "crash on boot" --json | jq -r '.[].id'
```

See `reference/json-recipes.md` for extraction patterns.

## Authentication

A server is configured with `bzr config set-server`. The API key comes from an
environment variable or the OS keychain. Check the active server and identity:

```
bzr config show     # lists servers and credential sources
bzr whoami          # confirms the authenticated user
```

If `whoami` fails, the server or credentials are not set up — see the
`bzr-setup` skill.

## Command groups

The full surface (with one example each) is in `reference/commands.md`. The
groups: `bug`, `comment`, `attachment`, `config`, `product`, `field`, `user`,
`group`, `whoami`, `server`, `classification`, `component`, `template`, `query`,
plus two local (no-network) top-level commands: `completion` (shell completion
scripts) and `schema` (published JSON Schemas for `--json` output).

Two surface facts agents get wrong:

- **Components** have their own read verbs as of 0.5.0:
  `bzr component list --product <product>` and
  `bzr component view <product> <component>`. `bzr product view <product>`
  still lists components inline alongside versions and milestones, which is
  often the convenient one-shot view.
- **Flags** are changed through `bzr bug update --flag ...` — there is **no**
  standalone `bzr flags` command.

## Global flags worth knowing

These work on any command (place them before the subcommand):

- `--json` / `--output ndjson` — JSON, or newline-delimited JSON (one record per
  line) for streaming into `jq -c` or agents.
- `--dry-run` — preview a bug mutation (`create`/`update`/`clone`/`resolve`/
  `close`/`reopen`/`dup`) without writing; prints the would-be payload.
- `-y` / `--yes` — skip the confirmation prompt for a batch mutation touching
  more than 10 bugs (interactive terminals only).
- `--timeout <secs>` / `--retry <n>` — tune per-request timeout and transient
  retry (reads only, with backoff).
- `--config <path>` — use an alternate `config.toml` (sandboxes the run).
- `--server-url <url>` (+ `--server-api-key-env <env>`, optional
  `--server-email <email>`) — a fully stateless inline server, no config file
  needed; ideal for CI and agents. See the `bzr-setup` skill.

## Cardinal rules

- **Prefer `--json` when parsing.** Tables are for humans.
- **Read before write.** Never run `bzr bug update` without first viewing the
  bug's current state (`bzr bug view`), or you risk clobbering fields. The
  `bzr-triage-bug` skill walks this through.
- **Keep writes explicit and minimal.** Change only the fields you intend to.

This reference is authored against **bzr 0.5.0**. If `bzr --version` is much
newer and a command here is rejected, the surface may have moved; check
`bzr <group> --help`.
