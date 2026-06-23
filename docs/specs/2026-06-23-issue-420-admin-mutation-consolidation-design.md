# Admin Create/Update Boilerplate Consolidation — Design

**Date:** 2026-06-23
**Branch:** `feat/consolidate-admin-create-update-420`
**Issue:** #420 — Consolidate admin create and update command boilerplate

## Background

The admin resource commands (`product`, `component`, `group`, `user`) each
implement `create` and `update` handlers that repeat an identical control-flow
skeleton. Desloppify reports duplication clusters across these create/update
paths.

Concretely, every one of the following handlers repeats the same five steps:

1. `let format = ctx.format();`
2. on `ctx.dry_run()`: build a `DryRunResult`, `write_result`, `return Ok(())`
3. `connect_and_configure(ctx).await?`
4. call the single resource API method
5. build an `ActionResult`, `write_result`, `Ok(())`

The handlers that follow this shape exactly:

| Handler | API call | Success result |
|---|---|---|
| `product/create` | `create_product` | `created_named` |
| `product/update` | `update_product` | `updated_named` |
| `group/create` | `create_group` | `created_named` |
| `group/update` | `update_group` | `updated_named` |
| `user/create` | `create_user` | `created_named` |
| `user/update` | `update_user` | `updated_named` |
| `component/create` | `create_component` | `created` |

Only four things vary across them: the `ResourceKind`, the dry-run message
wording, the single API call, and the success `ActionResult` + message. The
param-building, field-merging, and validation logic is already resource-specific
and lives in each handler's `build_params` — that is **not** duplication and must
stay where it is.

### The component/update outlier

`component/update` does **not** share this skeleton. It supports two distinct
target forms (numeric component ID, or `--product`/`--component` name pair),
which means:

- its dry-run branch renders one of two different serializable payloads
  (`UpdateComponentParams` for the ID form, a bespoke `NamedComponentUpdateDryRun`
  for the name form), not a single params object; and
- between connect and the API call it performs an extra network round-trip to
  resolve a name pair to a numeric ID (`get_product` + `find_component_id`).

Folding this into the shared seam would require the seam to accept closures for
dry-run rendering and for a pre-call resolution step — i.e. it would become the
"broad generic command framework" the issue explicitly prohibits. Therefore
`component/update` stays bespoke and is out of scope for the extraction.

## Goals (success criteria)

- A single shared seam under `src/commands/runtime/` carries the five-step
  skeleton, so the seven handlers in the table above stop repeating it.
- Request payloads, dry-run output (human + JSON), mutation output (human +
  JSON), and error behavior are **unchanged** for every migrated handler —
  verified by the existing `create_tests.rs` / `update_tests.rs` suites passing
  without weakening or deleting any assertion.
- The shared seam is covered by its own focused tests (dry-run path emits the
  preview and skips connect; commit path runs `execute` and writes its result).
- Resource-specific param building and validation stay in each handler,
  unchanged.
- No broad generic command framework: the seam handles only the linear
  create-shaped / name-keyed mutation flow. `component/update` is untouched.
- `cargo clippy --all-targets --all-features -- -D warnings` and focused
  `cargo test` are green.

## Design

### New module: `src/commands/runtime/mutation.rs`

```rust
/// Dry-run preview for a create/update mutation.
pub(crate) struct DryRunPreview<P: Serialize> {
    pub(crate) resource: ResourceKind,
    pub(crate) params: P,
    pub(crate) message: String,
}

/// The committed result of a mutation that ran for real.
pub(crate) struct Committed {
    pub(crate) result: ActionResult,
    pub(crate) message: String,
}

/// Drive an admin create/update command: emit the dry-run preview and stop, or
/// connect and run `execute`, then write its committed result.
pub(crate) async fn run<P, F, Fut>(
    ctx: &CommandContext,
    w: &mut Writers<'_>,
    preview: DryRunPreview<P>,
    execute: F,
) -> Result<()>
where
    P: Serialize,
    F: FnOnce(BugzillaClient, P) -> Fut,
    Fut: Future<Output = Result<Committed>>;
```

`run` builds `DryRunResult::new(preview.resource, &[], &preview.params)` on the
dry-run path. The empty `ids` slice is correct for every migrated handler: all
seven are create-shaped or name-keyed and pass `&[]` today. This constraint is
documented on the function; id-keyed previews (only `component/update`) are out
of scope.

Ownership: `preview` owns `params`. On the dry-run path `params` is borrowed for
serialization; on the commit path it is moved into `execute`. The two paths are
mutually exclusive, so there is no borrow conflict and no clone.

### Migrated handler shape

```rust
pub(super) async fn handle(args, ctx, w) -> Result<()> {
    let params = build_params(args)?;
    let message = format!("Would create product '{}'", params.name);
    mutation::run(
        ctx,
        w,
        DryRunPreview { resource: ResourceKind::Product, params, message },
        |client, params| async move {
            let id = client.create_product(&params).await?;
            Ok(Committed {
                result: ActionResult::created_named(id, params.name.as_str(), ResourceKind::Product),
                message: format!("Created product #{id} '{}'", params.name),
            })
        },
    )
    .await
}
```

Update handlers capture the resolved target name in the `execute` closure and
keep their `validate_params` call inside `build_params`, exactly as today.

### Module wiring

Add `pub(crate) mod mutation;` to `src/commands/runtime/mod.rs` and a one-line
doc-comment entry, matching the existing module list. Tests live in the sibling
`src/commands/runtime/mutation_tests.rs` per the repo's test-layout rule.

## Non-goals

- No change to `component/update`.
- No change to any `build_params` / validation logic.
- No change to client API method signatures, request payloads, or output types.
- No generic dispatch over resource kinds; each handler still names its own
  `ResourceKind`, API call, and messages literally.

## Risks

- **Behavior drift in output.** Mitigated: the existing per-resource test suites
  assert exact dry-run and success output and must pass unchanged.
- **Borrow-checker friction with the `execute` closure.** Mitigated by the
  owned-`params`/move-on-commit ownership model above.

## Considered & rejected

- **A generic `Mutation` trait with per-resource impls.** Rejected: it is the
  prohibited generic command framework and would obscure the resource-specific
  validation the issue says to preserve.
- **Including `component/update` via dry-run/resolve closures.** Rejected: see
  "The component/update outlier" — it pushes the seam into framework territory
  for one caller.
- **Extracting only the dry-run branch (not connect+commit).** Rejected: leaves
  half the duplicated skeleton in place for marginal benefit.
