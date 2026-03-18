# Writing Claude Code Skills for bzr

A guide for writing [Claude Code skills](https://docs.anthropic.com/en/docs/claude-code/skills) that drive bzr. Skills turn natural-language prompts into repeatable Bugzilla workflows your agent can execute.

bzr is a good fit for skills because every command supports `--json`, flags are consistent across subcommands, and the CLI covers the full Bugzilla lifecycle (bugs, comments, attachments, flags, products, users, groups, config).

For the full command reference, see [bzr-cli.md](bzr-cli.md).

## Directory Layout

Skills live in `.claude/skills/` as `SKILL.md` files. Since bzr skills are server-agnostic (use `--server` to target any configured instance), place them in your home directory so they work across all projects:

```
~/.claude/skills/
  bzr-triage/SKILL.md
  bzr-investigate/SKILL.md
  bzr-file-bug/SKILL.md
  bzr-review/SKILL.md
  bzr-setup/SKILL.md
  bzr-admin/SKILL.md
```

One skill per directory. The directory name becomes the `/slash-command` name.

## Key Patterns

Three patterns every bzr skill should use:

### `--json` goes before the subcommand

The `--json` flag is global — it must appear before the subcommand:

```bash
# preferred (shorthand)
bzr --json bug list --product Fedora

# also works (original form)
bzr --output json bug list --product Fedora

# wrong — flag is ignored
bzr bug list --product Fedora --output json
```

Always use `--json` in skills so the agent gets structured data it can parse.

### `allowed-tools` scopes permissions

Restrict what the skill can run. Add `Bash(jq *)` when the skill needs to parse JSON:

```yaml
allowed-tools: Bash(bzr *), Bash(jq *)
```

### `$ARGUMENTS` passes dynamic input

Use `$ARGUMENTS` for bug IDs, search terms, product names. The user provides them when invoking the skill:

```
/bzr-investigate 12345
```

Inside the skill body, `$ARGUMENTS` expands to `12345`.

## Skill Examples

Six complete skills covering common Bugzilla workflows. Copy any of these into `~/.claude/skills/<name>/SKILL.md`.

### bzr-triage — Search and prioritize NEW bugs

```yaml
---
name: bzr-triage
description: Search for NEW bugs in a product and help prioritize them
argument-hint: [product]
allowed-tools: Bash(bzr *), Bash(jq *)
---

# Triage NEW Bugs

Search for untriaged bugs in the **$ARGUMENTS** product and help prioritize them.

## Steps

1. List NEW bugs:
   `bzr --json bug list --product "$ARGUMENTS" --status NEW --limit 50`
2. Look up valid priorities and severities:
   `bzr --json field list priority`
   `bzr --json field list severity`
3. Present a summary table of bugs (ID, summary, severity, assignee)
4. For each bug the user selects, update priority/severity/assignee:
   `bzr bug update <ID> --priority <P> --severity <S>`

Always confirm before updating bugs.
```

### bzr-investigate — Deep-dive into a single bug

```yaml
---
name: bzr-investigate
description: Gather all context for a bug — details, comments, history, and attachments
argument-hint: [bug-id]
allowed-tools: Bash(bzr *), Bash(jq *)
---

# Investigate Bug

Gather full context for bug **$ARGUMENTS**.

## Steps

1. View the bug:
   `bzr --json bug view $ARGUMENTS`
2. List comments:
   `bzr --json comment list $ARGUMENTS`
3. View change history:
   `bzr --json bug history $ARGUMENTS`
4. List attachments:
   `bzr --json attachment list $ARGUMENTS`
5. Summarize findings: current status, key discussion points,
   recent changes, and any pending review requests or needinfo flags.
```

### bzr-file-bug — File a new bug interactively

```yaml
---
name: bzr-file-bug
description: File a new bug with product/component validation
argument-hint: [product]
disable-model-invocation: true
allowed-tools: Bash(bzr *), Bash(jq *)
---

# File a New Bug

File a bug in the **$ARGUMENTS** product.

## Steps

1. Verify the product exists and list its components:
   `bzr --json product view "$ARGUMENTS"`
2. Look up valid priorities and severities:
   `bzr --json field list priority`
   `bzr --json field list severity`
3. Ask the user for: component, summary, description, priority, severity
4. Create the bug (always pass `--description` to avoid opening `$EDITOR`):
   `bzr bug create --product "$ARGUMENTS" --component <C> --summary "<S>" --description "<D>" --priority <P> --severity <S>`
5. Show the created bug ID.
```

### bzr-review — Review patches on a bug

```yaml
---
name: bzr-review
description: Download and review patch attachments on a bug
argument-hint: [bug-id]
allowed-tools: Bash(bzr *), Bash(jq *), Bash(cat /tmp/bzr-review-*)
---

# Review Patches

Review patch attachments on bug **$ARGUMENTS**.

## Steps

1. List attachments and filter for patches:
   `bzr --json attachment list $ARGUMENTS | jq '[.[] | select(.is_patch == true and .is_obsolete == false)]'`
2. For each active patch, download to `/tmp`:
   `bzr attachment download <ID> -o /tmp/bzr-review-<ID>.patch`
3. Read and review each patch file
4. Present a review summary for each patch:
   - What the patch changes
   - Potential issues or risks
   - Suggested review flag (review+, review-, review?)
5. If the user approves, post a review comment:
   `bzr comment add $ARGUMENTS --body "<review text>"`
   And optionally set the review flag:
   `bzr attachment update <ID> --flag "review+(user@example.com)"`
```

### bzr-setup — Configure a Bugzilla server

```yaml
---
name: bzr-setup
description: Walk through configuring a new Bugzilla server connection
disable-model-invocation: true
allowed-tools: Bash(bzr *)
---

# Set Up a Bugzilla Server

Walk the user through connecting to a Bugzilla server.

## Steps

1. Ask for: server alias name, URL, API key, and optionally email
   (email is needed for Bugzilla 5.0 or earlier)
2. Configure the server:
   `bzr config set-server <NAME> --url <URL> --api-key <KEY>`
   Add `--email <EMAIL>` if provided.
3. Verify the connection:
   `bzr --server <NAME> whoami`
4. Show server info:
   `bzr --server <NAME> server info`
5. If this is the first server, it becomes the default automatically.
   Otherwise ask if the user wants to set it as default:
   `bzr config set-default <NAME>`
6. Show final config:
   `bzr config show`
```

### bzr-admin — Admin: products, users, groups

```yaml
---
name: bzr-admin
description: Admin operations — create/update products, components, users, and groups
disable-model-invocation: true
allowed-tools: Bash(bzr *), Bash(jq *)
---

# Bugzilla Admin Operations

Perform admin operations on the Bugzilla server. All commands below
require admin privileges.

## Available operations

Ask the user what they want to do:

### Products
- List products: `bzr --json product list`
- View product: `bzr --json product view "<NAME>"`
- Create product: `bzr product create --name "<N>" --description "<D>"`
- Update product: `bzr product update "<NAME>" --description "<D>"`

### Components
- Create: `bzr component create --product "<P>" --name "<N>" --description "<D>" --default-assignee <E>`
- Update: `bzr component update <ID> --name "<N>"`

### Users
- Search: `bzr --json user search "<QUERY>"`
- Create: `bzr user create --email <E> --full-name "<N>"`
- Update: `bzr user update <USER> --real-name "<N>"`

### Groups
- View: `bzr --json group view <GROUP>`
- List members: `bzr --json group list-users --group <G>`
- Create: `bzr group create --name "<N>" --description "<D>"`
- Add user: `bzr group add-user --group <G> --user <U>`
- Remove user: `bzr group remove-user --group <G> --user <U>`

Confirm before every write operation.
```

## Composing Multi-Step Skills

Skills can chain multiple bzr commands into a pipeline. Two examples:

### Bug-to-patch review pipeline

This skill searches for bugs with unreviewed patches and builds a review queue:

```yaml
---
name: bzr-review-queue
description: Find bugs with unreviewed patches and present a review queue
argument-hint: [product]
allowed-tools: Bash(bzr *), Bash(jq *), Bash(cat /tmp/bzr-review-*)
---

# Patch Review Queue

Find bugs in **$ARGUMENTS** that have unreviewed patches.

## Steps

1. List open bugs in the product:
   `bzr --json bug list --product "$ARGUMENTS" --status NEW --status ASSIGNED --limit 100`

2. For each bug, check for non-obsolete patch attachments:
   `bzr --json attachment list <BUG_ID> | jq '[.[] | select(.is_patch == true and .is_obsolete == false)]'`

3. Collect bugs that have at least one active patch into a review queue.

4. For each bug in the queue, download the latest patch:
   `bzr attachment download <ATTACHMENT_ID> -o /tmp/bzr-review-<ATTACHMENT_ID>.patch`

5. Present the review queue as a numbered list:
   - Bug ID, summary, patch count, last updated

6. Let the user pick a bug to review, then show the patch content
   and help them write a review comment.
```

### Sprint report

This skill generates a summary of resolved bugs over a date range:

```yaml
---
name: bzr-sprint-report
description: Summarize resolved bugs in a product for a date range
argument-hint: [product]
allowed-tools: Bash(bzr *), Bash(jq *)
---

# Sprint Report

Generate a report of resolved bugs in **$ARGUMENTS**.

## Steps

1. Ask the user for the date range (start and end dates).

2. List resolved bugs:
   `bzr --json bug list --product "$ARGUMENTS" --status RESOLVED --limit 200`

3. Use `jq` to filter bugs by last-change date within the range
   and group by resolution:
   `jq '[.[] | select(.last_change_time >= "START" and .last_change_time <= "END")]
        | group_by(.resolution)
        | map({resolution: .[0].resolution, count: length, bugs: [.[].id]})'`

4. Present a summary:
   - Total bugs resolved
   - Breakdown by resolution (FIXED, WONTFIX, DUPLICATE, etc.)
   - List of bug IDs per resolution category

5. Optionally drill into any category for full bug details.
```

## Tips and Gotchas

- **Use `--json` shorthand**: `--json` is equivalent to `--output json` but shorter. Skills can also set `BZR_OUTPUT=json` in the environment to avoid repeating the flag.
- **Auto-detection**: When stdout is not a TTY (piped or redirected), bzr outputs JSON automatically. No flags needed in pipe contexts.
- **Flag positioning**: `--json` and `--server` go *before* the subcommand. `bzr --json bug list`, not `bzr bug list --json`.
- **Exit codes**: bzr uses distinct exit codes (0=success, 2=usage, 3=config, 4=API, 5=network, 6=IO). Skills can check `$?` to handle errors without parsing stderr.
- **JSON errors**: When `--json` is active, errors are emitted as JSON on stderr: `{"error":{"type":"api","message":"...","exit_code":4}}`.
- **Mutation responses**: Create/update commands return structured JSON with `--json`: `{"id":123,"resource":"bug","action":"created"}`. No need for regex to extract IDs.
- **Pipe comments**: `echo "text" | bzr comment add 12345` works when stdin is not a TTY. No need for `--body` in pipe contexts.
- **Always pass `--body`**: When adding comments in a skill, always use `--body "<text>"`. Omitting it at a TTY opens `$EDITOR`, which hangs in non-interactive contexts.
- **Attachment downloads**: Use `-o /tmp/<name>` to save attachments to a known path the agent can read back.
- **Validate before updating**: Use `bzr field list <field>` to check valid values for status, priority, severity, and resolution before calling `bug update`.
- **Flag syntax**: `review?(user@example.com)` requests review, `review+` grants, `review-` denies. See [Flag Syntax](bzr-cli.md#flag-syntax).
- **Default limit is 50**: `bug list` and `bug search` return at most 50 results by default. Set `--limit` explicitly when you need more (or fewer).
- **`disable-model-invocation: true`**: Use this for skills with side effects (creating bugs, updating configs, admin ops) so Claude only runs them when you explicitly invoke `/skill-name`.
- **Multi-server**: Pass `--server <name>` to target a non-default server. Works with every command.

## Quick Reference

Common tasks mapped to bzr commands for use in skills:

| Task | Command |
|------|---------|
| List open bugs | `bzr --json bug list --product P --status NEW` |
| Full-text search | `bzr --json bug search "query" --limit 20` |
| View bug details | `bzr --json bug view ID` |
| View bug history | `bzr --json bug history ID` |
| Create a bug | `bzr bug create --product P --component C --summary "S" --description "D"` |
| Update bug status | `bzr bug update ID --status S --resolution R` |
| Set a flag | `bzr bug update ID --flag "review?(user@example.com)"` |
| List comments | `bzr --json comment list BUG_ID` |
| Add a comment | `bzr comment add BUG_ID --body "text"` |
| List attachments | `bzr --json attachment list BUG_ID` |
| Download attachment | `bzr attachment download ID -o /tmp/file` |
| Upload attachment | `bzr attachment upload BUG_ID file.patch --flag "review?(user)"` |
| List products | `bzr --json product list` |
| View product | `bzr --json product view NAME` |
| Look up field values | `bzr --json field list priority` |
| Search users | `bzr --json user search "query"` |
| Check auth | `bzr --json whoami` |
| Server version | `bzr --json server info` |
| Show config | `bzr --json config show` |
