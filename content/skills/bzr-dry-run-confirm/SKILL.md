---
name: bzr-dry-run-confirm
description: Use when mutating Bugzilla data with bzr — preview every write with global --dry-run, verify the JSON payload with jq, then re-run without the flag to commit. Covers bug create/update/clone/resolve/close/reopen/dup and product/component/user/group create+update, batch previews via --from-json, exit-code triage, and --yes batch-confirm semantics.
---

# Dry-run, verify, then commit

`bzr` mutations are agent-native "tool call with preview": run any supported
mutation with the **global** `--dry-run` flag, parse and verify the printed
payload, then re-run the *same command without* `--dry-run` to apply it.

There is **no `--confirm` flag**. Confirmation is positional in time: the
dry-run is the preview; dropping `--dry-run` on the re-run is the commit.

## The round-trip

```
# 1. Preview — validates the request and prints what WOULD be sent
bzr bug update 42 --status ASSIGNED --json --dry-run | tee /tmp/preview.json

# 2. Verify the payload is exactly what you intend (see below)
jq -e '.data.action == "dry-run" and .data.ids == [42]
      and .data.changes.status == "ASSIGNED"' /tmp/preview.json

# 3. Commit — same command, minus --dry-run
bzr bug update 42 --status ASSIGNED
```

The dry-run payload is a `DryRunResult`: with `--json` it arrives inside the
versioned envelope as `{"schema_version":…,"data":{"resource":"bug",
"action":"dry-run","ids":[42],"changes":{…}}}` where `changes` is the exact
request body the write API would receive (`ids` lists affected existing bugs,
empty for creates). With `--output ndjson` the record is bare — read
`.action`/`.changes` directly, no `.data`.

Never pipe the dry-run into the real run blindly: parse it first. The
verification step is not optional — that is the point of this skill.

## When to skip dry-run

- **Read-only commands** never need it: `bug search/list/view/my/history`,
  `comment list`, `attachment list/download`, `product/component/user/group`
  lookups, `query run`. There is nothing to preview.
- **Idempotent re-runs** of an already-applied change can skip it: setting
  `--status ASSIGNED` on a bug that is already `ASSIGNED` converges to the
  same state, so re-running after an ambiguous failure is safe without a
  second preview.
- Everything else should dry-run first. Note `--dry-run` is fail-fast, not
  silently ignored: on a command that does not support it, `bzr` exits 7
  rather than writing anyway. If you asked for a preview and got exit 7, you
  were on an unsupported command — see Failure modes.

## Comparing payloads

Verify field-by-field before committing:

```
# Assert the action marker and target IDs
jq -e '.data.action == "dry-run"' /tmp/preview.json
jq -e '.data.ids | index(42) != null' /tmp/preview.json

# Diff the planned changes against your intended change set
jq -n --slurpfile p /tmp/preview.json '
  ($p[0].data.changes) as $got
  | ({"status": "ASSIGNED"}) as $want
  | [ $want | keys[] | select(($got[.] // "absent") != $want[.]) ]
  | if length == 0 then "payload matches intent" else {mismatched: .} end'
```

Simpler, per-field assertions are usually enough for one-off updates:

```
jq -e '.data.changes | keys == ["status"]' /tmp/preview.json        # exactly one field touched
jq -r '.data.changes.status' /tmp/preview.json                      # value sanity check
```

For `bug clone` the preview includes the copied field set — compare it against
a fresh `bug view <source-id> --json` when you need to know which fields the
clone inherits.

## Batch dry-run

```
bzr bug create --from-json array.json --json --dry-run
```

An array in `--from-json` files one bug per element; with `--dry-run` every
element is validated and previewed without writing anything. Run this before
any batch file so a malformed row fails cheaply at the preview instead of
halfway through the real batch. Compound plans (first comment, attachments)
appear inside `changes` — attachment entries show `file_name`, `summary`,
`content_type`, and `size` metadata only.

## Failure modes

Errors print to stderr as structured JSON under `error` (with `--json`), or
`error: …` text otherwise. Triage by `type` and `exit_code`:

| Exit | Type | Meaning |
|------|------|---------|
| 7 | `input` | `InputValidation` — request rejected before any API call; names `field`/`value` |
| 5 | `http` | `HttpStatus` — server rejected the request (auth, permissions, bad state) |
| 4 | `api` | Bugzilla API error surfaced from the response body |

Exit 7 also fires when `--dry-run` is used on an unsupported command (for
example standalone `attachment upload`) — the tool refuses to silently ignore
a requested preview. Surface the offending field before retrying:
```
bzr bug create --from-json batch.json --json --dry-run 2>/tmp/err.json
jq -r '.error.field, .error.value' /tmp/err.json   # e.g. "product", then the rejected value
```

Fix the named field and re-preview; do not retry blind.

**Attachment note:** standalone `attachment upload` / `attachment update` do
NOT support `--dry-run` (exit 7). Attachments appear in previews only through
the compound `bug create --from-json` plan above.

## Confirm semantics

Agents running non-interactively are **never prompted**: confirmation prompts
require a TTY, so piped/agent runs proceed directly. Two rules still matter:

- **≤ 10 bugs:** no prompt exists even interactively — the dry-run round-trip
  above is the entire safety net. Use it.
- **> 10 bugs** in one `bug update`/`resolve`/`close`/`reopen` invocation:
  interactive sessions prompt ("About to modify N bugs"); `-y`/`--yes`
  bypasses that prompt. Non-TTY runs auto-bypass, so agents don't need
  `--yes` — but they carry full responsibility for having verified the
  preview first.

`--yes` is not a substitute for `--dry-run`; it only answers the batch prompt.

## Mutation surface

Supported by `--dry-run`: `bug create`, `bug update`, `bug clone`,
`bug resolve`, `bug close`, `bug reopen`, `bug dup`, and `create`/`update`
for `product`, `component`, `user`, and `group`.

Domain-specific guidance lives in its own skill — do not duplicate it here:
file a well-formed bug with `bzr-file-bug`; single-bug read-before-write
triage with `bzr-triage-bug`; query-driven bulk sweeps with `bzr-bulk-triage`.
Command-surface reference (flags, JSON envelope, auth): `bzr-reference`.

This reference is authored against **bzr 0.8.3-dev**.
