# Writing IBM Bob Skills for bzr

A guide for creating [IBM Bob skills](https://bob.ibm.com/docs/ide/features/skills) that drive `bzr`. Bob skills are reusable instruction sets that let Bob run Bugzilla workflows consistently using the `bzr` CLI.

`bzr` is a strong fit for Bob skills because the CLI has stable subcommands, global `--json` output, consistent exit behavior, and broad Bugzilla coverage across bugs, comments, attachments, products, users, groups, templates, queries, and config.

For the full command reference, see [bzr-cli.md](bzr-cli.md). For Claude Code's skill format, see [skills.md](skills.md).

## Bob Skill Basics

According to IBM Bob's skills documentation:

- Skills require **Advanced mode**
- Skills live in either `<project>/.bob/skills/` or `~/.bob/skills/`
- Each skill lives in its own directory with a `SKILL.md`
- `SKILL.md` uses YAML front matter followed by instructions
- The required front matter fields are `name` and `description`
- Bob can also read supporting files placed in the same skill directory

Project-level skills override global skills with the same name.

## Directory Layout

Project-local Bob skills:

```text
your-project/
  .bob/
    skills/
      bzr-investigate/
        SKILL.md
      bzr-bug-summary/
        SKILL.md
      bzr-review/
        SKILL.md
        checklist.md
```

Global Bob skills:

```text
~/.bob/skills/
  bzr-investigate/
    SKILL.md
  bzr-bug-summary/
    SKILL.md
```

## Key Patterns

Three patterns make `bzr` skills work well in Bob.

### Always prefer `--json`

Use JSON output whenever Bob needs to inspect command results:

```bash
bzr --json bug view 12345
bzr --json comment list 12345
bzr --json query run my-open-bugs
```

The `--json` flag is global, so it must appear before the subcommand.

### Keep writes explicit

Bob skills should use direct, non-interactive commands for side effects:

```bash
bzr bug update 12345 --status RESOLVED --resolution FIXED
bzr comment add 12345 --body "Investigated and confirmed locally"
bzr attachment upload 12345 fix.patch --flag "review?(alice@example.com)"
```

Avoid workflows that depend on `$EDITOR` or ambiguous follow-up input unless the skill explicitly asks the user first.

Examples:

- `bzr comment add 12345` at a TTY opens `$EDITOR`, which stalls unattended agent runs. Use `bzr comment add 12345 --body "text"` instead.
- "File a bug" is ambiguous if the skill has not collected `--product`, `--component`, and a usable description yet. Ask for the missing values first, or use `bzr bug create --template <name> --summary "..." --description "..."`.
- "Resolve this bug" is ambiguous when the valid status or resolution values are server-specific. Check them first with `bzr field list status` and `bzr field list resolution`, then call `bzr bug update`.

### Keep the main skill short

Bob's docs recommend keeping `SKILL.md` focused on the workflow and moving detailed checklists, templates, and examples into supporting files in the same directory.

## Bob Skill Template

Use this as a starting point:

```yaml
---
name: bzr-investigate
description: Gather full Bugzilla context for a bug using bzr, including details, comments, history, and attachments
---

Investigate bug **$ARGUMENTS** using `bzr`.

<Steps>
<Step>
Fetch the bug details:
`bzr --json bug view $ARGUMENTS`
</Step>

<Step>
Fetch recent comments and change history:
`bzr --json comment list $ARGUMENTS`
`bzr --json bug history $ARGUMENTS`
</Step>

<Step>
Fetch attachments when they may affect diagnosis:
`bzr --json attachment list $ARGUMENTS`
</Step>

<Step>
Summarize the current status, owner, recent activity, blockers, and next action.
</Step>
</Steps>
```

Bob's docs show `<Steps>` and `<Step>` as a good pattern for clear, actionable workflows. Plain Markdown instructions also work, but the step structure makes longer skills easier to follow.

## Example Skills

These examples mirror the most useful `bzr` workflows for Bob users.

### `bzr-investigate`

```yaml
---
name: bzr-investigate
description: Gather full Bugzilla context for a bug using bzr, including details, comments, history, and attachments
---

Investigate bug **$ARGUMENTS**.

<Steps>
<Step>
View the bug:
`bzr --json bug view $ARGUMENTS`
</Step>

<Step>
Fetch comments:
`bzr --json comment list $ARGUMENTS`
</Step>

<Step>
Fetch change history:
`bzr --json bug history $ARGUMENTS`
</Step>

<Step>
Fetch attachments:
`bzr --json attachment list $ARGUMENTS`
</Step>

<Step>
Summarize the bug's status, key discussion points, recent changes, and unresolved blockers.
</Step>
</Steps>
```

### `bzr-bug-summary`

```yaml
---
name: bzr-bug-summary
description: Summarize one or more Bugzilla bugs with current status, blockers, and recommended next actions
---

Summarize the bugs in **$ARGUMENTS**.

<Steps>
<Step>
For each bug, fetch core details:
`bzr --json bug view <BUG_ID>`
</Step>

<Step>
Fetch change history:
`bzr --json bug history <BUG_ID>`
</Step>

<Step>
Fetch comments when recent discussion matters:
`bzr --json comment list <BUG_ID>`
</Step>

<Step>
Identify status, assignee, component, dependencies, recent activity, blockers, and next actions.
</Step>

<Step>
For a single bug, produce a concise summary.
For multiple bugs, begin with a comparison table and then summarize each bug.
</Step>
</Steps>
```

### `bzr-review`

```yaml
---
name: bzr-review
description: Review patch attachments on a bug and summarize risks, missing tests, and next actions
---

Review the patch attachments for bug **$ARGUMENTS**.

<Steps>
<Step>
List attachments and identify active patches:
`bzr --json attachment list $ARGUMENTS`
</Step>

<Step>
Download relevant patch attachments to a known local path:
`bzr attachment download <ATTACHMENT_ID> -o /tmp/bzr-review-<ATTACHMENT_ID>.patch`
</Step>

<Step>
Read the patch files and identify behavior changes, correctness risks, regressions, and missing tests.
</Step>

<Step>
Present a review summary and suggest a review disposition.
</Step>
</Steps>
```

## Supporting Files

Bob can read files placed next to `SKILL.md`, which makes it easier to keep the skill itself short.

Example:

```text
.bob/skills/bzr-review/
  SKILL.md
  checklist.md
  severity-guide.md
```

Good supporting files for `bzr` skills:

- `checklist.md` for review criteria
- `bug-summary-template.md` for summary structure
- `field-guide.md` for team-specific status, severity, or priority conventions
- `query-examples.md` for reusable search patterns

Reference supporting files directly from the instructions, for example:

```text
Use the review checklist in `checklist.md` before writing findings.
```

## Mapping Existing Claude Skills to Bob

The existing [skills.md](skills.md) guide is Claude Code specific, but the workflows translate cleanly:

- Claude `allowed-tools` does not carry over to Bob; remove it
- Claude `argument-hint` does not carry over to Bob; describe expected input in the instructions instead
- Claude `disable-model-invocation: true` does not carry over to Bob; keep write-heavy Bob skills narrowly scoped and explicit
- Command recipes, output conventions, templates, and saved-query workflows carry over directly

In practice, most `bzr` Claude skills can be converted to Bob by simplifying the front matter and keeping the same command steps.

## Best Practices for Bob Users

- Use project-local skills in `.bob/skills/` when the workflow depends on repository conventions
- Use `~/.bob/skills/` for general Bugzilla workflows that should work across projects
- Prefer one skill per workflow: investigate, summarize, review, triage, setup
- Keep descriptions precise so Bob can decide when the skill is relevant
- Use `bzr field list <field>` before update workflows when status, resolution, severity, or priority values may vary by server
- Use `bzr template save` and `bzr query save` to give Bob reusable local workflows without adding custom wrapper scripts
- Prefer `bzr --json` for reads and scripted analysis
- Prefer `--body` for comments to avoid interactive editor prompts in agent flows

## Quick Reference

| Task | Command |
|------|---------|
| View bug details | `bzr --json bug view ID` |
| View bug history | `bzr --json bug history ID` |
| List comments | `bzr --json comment list BUG_ID` |
| List attachments | `bzr --json attachment list BUG_ID` |
| Search bugs | `bzr --json bug search "query" --limit 20` |
| List my bugs | `bzr --json bug my --all --limit 20` |
| Create bug from template | `bzr bug create --template NAME --summary "S" --description "D"` |
| Update bug | `bzr bug update ID --status S --resolution R` |
| Add comment | `bzr comment add BUG_ID --body "text"` |
| Save query | `bzr query save NAME --product P --status NEW --limit 50` |
| Run saved query | `bzr --json query run NAME` |
| Save template | `bzr template save NAME --product P --component C` |
| Check auth | `bzr --json whoami` |
| Show server info | `bzr --json server info` |
