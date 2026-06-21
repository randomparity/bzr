---
name: bzr-triage-bug
description: Use when triaging or updating an existing Bugzilla bug with bzr — read the bug's current state and history first, then change status, resolution, flags, or add a comment without clobbering fields you did not intend to touch.
---

# Triage a bug with bzr (read before write)

The cardinal rule: **read the current state before you change anything.** A
blind `bzr bug update` can overwrite fields another person just set.

## 1. Read

```
bzr bug view <id> --json        # current status, assignee, flags, fields
bzr bug history <id>            # what changed recently, by whom
bzr comment list <id>          # the discussion so far
```

Understand who owns it, the current status/resolution, and any pending flags
before deciding what to change.

## 2. Decide, then change only what you mean to

```
# Move to resolved/fixed
bzr bug update <id> --status RESOLVED --resolution FIXED

# Set or clear a flag (flags go through bug update, not a 'flags' command)
bzr bug update <id> --flag "review+(alice@example.com)"

# Add context as a comment rather than overwriting the description
bzr comment add <id> --body "Reproduced on Fedora 42; root cause is X."

# Tag a comment for workflow
bzr comment tag <id> --add needs-info
```

### Convenience verbs (sugar over `update`)

```
bzr bug resolve <id> --as FIXED        # status RESOLVED + resolution
bzr bug close <id>                     # default status VERIFIED (stock 5.x)
bzr bug reopen <id>                    # default status CONFIRMED (stock 5.x)
bzr bug dup <id> <dupe-of-id>          # mark as a duplicate
```

`close`/`reopen` default to the stock Bugzilla 5.x statuses VERIFIED / CONFIRMED;
pass `--status <STATUS>` for installs with custom statuses (e.g.
`bzr bug close <id> --status CLOSED`).

Pass only the flags for fields you intend to change. If a status transition is
rejected, `bzr field list status --json` shows the allowed `can_change_to`
targets from the current state.

### Guard against mid-air collisions

After reading the bug, capture its `last_change_time` and pass it back on the
write so a concurrent edit is detected instead of clobbered:

```
ts=$(bzr bug view <id> --json | jq -r '.last_change_time')
bzr bug update <id> --status RESOLVED --resolution FIXED \
  --expect-unchanged-since "$ts"      # exits 14 if the bug changed meanwhile
```

This is the cardinal read-before-write rule made enforceable. To rehearse a
change without writing, add `--dry-run` — bzr validates and prints the would-be
payload (`"action":"dry-run"`) without calling the API.

## 3. Verify

```
bzr bug view <id>     # confirm the change landed as intended
```

See `bzr-reference` for the full command surface and the JSON contract.
