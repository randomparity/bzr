# Issue 367: Admin Mutation Dry-Run Previews

## Context

`--dry-run` is global but currently supported only by bug mutations. Dispatch
rejects the flag for every other command so it cannot be silently ignored. The
admin write commands for products, components, users, and groups still require a
real write to inspect the outbound request payload.

## Decision

Extend `--dry-run` to exactly these admin mutations:

- `product create`
- `product update`
- `component create`
- `component update`
- `user create`
- `user update`
- `group create`
- `group update`

Do not include local config/template/query mutations or group membership changes
in this issue. Those operations have different semantics and should not inherit
dry-run behavior without a separate design.

The command path still loads config, resolves auth, and builds a client before
previewing, matching existing bug dry-run behavior. After the command validates
and builds the normal request params, it emits `DryRunResult` and returns before
calling POST/PUT.

`dry-run-result.json` already permits `product`, `component`, `user`, and
`group` resources and keeps `changes` open, so the published schema does not
need to change.

## Validation

Create commands rely on clap-required arguments. Update commands must reject an
empty update before either previewing or writing:

- product: no `description`, `default_milestone`, or `is_open`
- component: no `name`, `description`, or `default_assignee`
- user: no `real_name`, `email`, or resolved `login_denied_text` from
  `--disable-login`
- group: no `description` or `is_active`

This makes dry-run previews match an actual write request instead of presenting
an empty payload as useful.

## Output

Use the existing dry-run result shape:

```json
{
  "resource": "product",
  "action": "dry-run",
  "ids": [],
  "changes": {}
}
```

`ids` is populated only when the admin command targets a numeric resource ID
(`component update`). Name-keyed create/update commands use an empty array, as
bug create and bug clone already do.
