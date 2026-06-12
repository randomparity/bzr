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
`group`, `whoami`, `server`, `classification`, `component`, `template`, `query`.

Two surface facts agents get wrong:

- **Components** are read from `bzr product view <product>` — there is **no**
  `bzr component list` (`bzr component` only has admin `create`/`update`).
- **Flags** are changed through `bzr bug update --flag ...` — there is **no**
  standalone `bzr flags` command.

## Cardinal rules

- **Prefer `--json` when parsing.** Tables are for humans.
- **Read before write.** Never run `bzr bug update` without first viewing the
  bug's current state (`bzr bug view`), or you risk clobbering fields. The
  `bzr-triage-bug` skill walks this through.
- **Keep writes explicit and minimal.** Change only the fields you intend to.

This reference is authored against **bzr 0.4.2**. If `bzr --version` is much
newer and a command here is rejected, the surface may have moved; check
`bzr <group> --help`.
