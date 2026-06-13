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

Pass only the flags for fields you intend to change. If a status transition is
rejected, `bzr field list status --json` shows the allowed `can_change_to`
targets from the current state.

## 3. Verify

```
bzr bug view <id>     # confirm the change landed as intended
```

See `bzr-reference` for the full command surface and the JSON contract.
