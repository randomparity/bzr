---
name: bzr-file-bug
description: Use when filing a new Bugzilla bug with bzr — discovers the right product and component, applies a template if available, and writes a well-formed report with a clear summary and reproduction steps.
---

# File a well-formed bug with bzr

## 1. Find the right product and component

```
bzr product list --json | jq -r '.data[].name'
bzr product view <Product> --json | jq -r '.data.components[].name'
```

`bzr component list --product <Product>` lists a product's components, and
`bzr product view <Product>` shows them inline alongside versions and milestones.

## 2. Reuse a template if one fits

```
bzr template list
bzr bug create --template <name> --summary "..." --description "..."
```

A template supplies saved field defaults so you only fill in what is bug-specific.
Templates can carry routing fields and create metadata: `--product`,
`--component`, `--version`, `--priority`, `--severity`, `--assignee`,
`--op-sys`, `--rep-platform`, `--description`, `--url`, `--whiteboard`,
`--target-milestone`, `--deadline`, `--cc`, `--keywords`, `--groups`, and
`--flag`.

```
bzr template save security-routing \
  --product Security --component Triage \
  --severity critical --cc triage@example.com --flag review?
bzr template update security-routing --target-milestone next --clear assignee
```

## 3. Write a good report

A useful bug has:
- **Summary** — one specific line: what failed, where. Not "it's broken".
- **Description** — what you did, what happened, what you expected, and the
  affected version. Include exact error text and reproduction steps.

```
bzr bug create \
  --product <Product> --component <Component> \
  --summary "Boot fails with 'no root device' after 6.x upgrade" \
  --description "Steps: 1) upgrade to 6.x 2) reboot. Expected: boots. Actual: drops to dracut with 'no root device'. Version: 6.9.0."
```

Set any metadata in the same call — no follow-up `update` is needed. The create
flags that mirror `bug update` are `--alias`, `--url`, `--whiteboard`,
`--target-milestone`, `--deadline` (`YYYY-MM-DD`), `--cc`, `--keywords`,
`--groups`, and `--flag` (Bugzilla flag syntax, e.g. `review?(qa@example.com)`).
`--cc`/`--keywords`/`--groups` take comma-separated values and may repeat:

```
bzr bug create --product <P> --component <C> --summary "..." --description "..." \
  --keywords regression,crash --cc qa@example.com \
  --target-milestone 9.0 --flag "review?(maintainer@example.com)"
```

To file a close variant of an existing bug, prefer `bug clone` and override only
the fields that should differ from the source:

```
bzr bug clone 12345 --summary "Backport to 9.4" \
  --version "9.4" --target-milestone 9.4 --add-depends-on
```

## Structured or batch filing

For machine-generated reports, file from JSON instead of flags. A single object
files one bug; an array files one per element (exit 11 if any element fails):

```
bzr bug create --from-json bug.json
echo '{"product":"P","component":"C","summary":"...","description":"..."}' \
  | bzr bug create --from-json -        # `-` reads JSON from stdin
```

JSON keys match the create flag names (`target_milestone`, `cc`, `keywords`, …);
explicit CLI flags override the corresponding JSON field. Add `--dry-run` to any
create to validate and preview the payload without writing.

## 4. Confirm

```
bzr bug view <new-id>     # check it landed with the fields you intended
```

Set required fields the server demands (version, severity) — if create is
rejected for a missing field, `bzr field list <field>` shows valid values.
See `bzr-reference` for the full surface.
