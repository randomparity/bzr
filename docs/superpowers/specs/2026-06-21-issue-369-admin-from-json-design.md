# Issue 369: Admin Structured Input

## Context

`bug create` and `bug update` already accept `--from-json`, reject unknown keys,
overlay explicit CLI values on top of JSON fields, and publish input schemas.
Product, component, user, and group create/update still require flag-only input,
which makes agent callers flatten already-structured resource edits into shell
arguments.

## Scope

Add `--from-json <PATH>` to exactly these admin mutations:

- `product create`
- `product update`
- `component create`
- `component update`
- `user create`
- `user update`
- `group create`
- `group update`

Admin `--from-json` accepts a top-level JSON object only. Arrays stay out of
scope because these commands currently have single-action result shapes and no
admin batch/partial-failure model.

## Input Model

Structured keys mirror CLI flag names. Unknown keys are rejected with
`serde(deny_unknown_fields)`.

Create commands validate required fields after overlaying explicit CLI flags:

- product create: `name`, `description`; `version` defaults to `unspecified`
  and `is_open` defaults to `true`
- component create: `product`, `name`, `description`, `default_assignee`
- user create: `email`; `login`, `full_name`, and `password` are optional
- group create: `name`, `description`; `is_active` defaults to `true`

Update commands accept their target either positionally or from JSON:

- product update: positional product name or JSON `name`
- component update: positional component ID or JSON `id`
- user update: positional user login/ID or JSON `user`
- group update: positional group name/ID or JSON `group`

Providing both positional and JSON target values is rejected, matching the bug
update target-conflict behavior. After resolving the target, existing empty
update validation still applies.

## Output And Dry Run

The write path uses the same request params as flag mode, so successful writes
keep the existing `ActionResult` output shape. If global `--dry-run` is enabled,
the command emits the existing `DryRunResult` shape using the merged request
payload and returns before POST/PUT.

## Schemas

Publish these input schemas:

- `product-create-input`
- `product-update-input`
- `component-create-input`
- `component-update-input`
- `user-create-input`
- `user-update-input`
- `group-create-input`
- `group-update-input`

Each schema describes the object shape accepted by that command and disallows
additional properties. The schema docs note that required create fields may be
supplied by JSON or by explicit CLI flags.

## Verification

Tests should cover:

- CLI parsing of `--from-json` for each admin create/update command.
- Parser rejection for unknown keys and wrong top-level shapes.
- CLI flag override of corresponding JSON fields.
- Request body construction for each resource.
- Existing dry-run output with structured input.
- Schema registry and parser-key drift for the new input schemas.
