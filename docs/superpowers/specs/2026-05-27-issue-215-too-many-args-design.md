# Enforce `too-many-arguments-threshold = 5` (issue #215)

## Problem

The project guideline is ≤5 positional parameters per function, but clippy's
default `too-many-arguments-threshold` (8) does not enforce it. Three command-
layer helpers currently sit at 6 positional params, all sharing the same
`(…, format: OutputFormat, w: &mut Writers<'_>)` tail:

- `download_batch` — `src/commands/attachment.rs`
- `migrate_to_keyring` — `src/commands/config.rs`
- `view_single` — `src/commands/bug/view.rs`

After lowering the threshold all three trip
`clippy::too-many-arguments`; running `cargo clippy --all-targets -- -D warnings`
with the new `clippy.toml` confirms exactly these three sites and nothing else.

## Design

### 1. `clippy.toml`

Create a new top-level `clippy.toml` containing the single setting:

```toml
too-many-arguments-threshold = 5
```

This is the only Clippy configuration in the file — `Cargo.toml`'s
`[lints.clippy]` block already sets per-lint levels; `clippy.toml` is the
correct location for behavior-tuning knobs like this threshold.

### 2. Per-function param bundling

Rather than introduce a generic `OutputContext` and use it in only 3 of ~40
helpers that share the `format, w` tail (mixed-pattern code drift), each
function gets a small bundle struct for its **command-specific** parameters.
The `format, w` tail is left intact so it remains consistent with the rest of
the command layer. The end state for each is exactly 5 positional params (the
threshold) or fewer.

| Function | New bundling | Final arity |
|---|---|---|
| `view_single` | Reuse existing `ColumnSpec<'_>` (it already wraps `include` / `exclude`) | 5 |
| `download_batch` | New `BatchTargets<'a> { ids, bug_ids, out_dir }` (private to `commands/attachment.rs`) | 4 |
| `migrate_to_keyring` | New `MigrateSpec<'a> { name, service, account }` (private to `commands/config.rs`) | 4 |

Rationale for not introducing an `OutputContext`:

- It would only be used in 3 of the ~40 helpers that pair `format` with
  `Writers`; the other 37 are already at or under threshold 5 and have no
  pressure to adopt it. Mixed adoption is worse than no adoption.
- Folding `format` into `Writers` would be uniformly clean but touches 261
  `.writers()` call sites in tests plus every command-layer helper —
  disproportionate scope for a 3-function lint fix.
- Per-function bundles are local, single-purpose, and document the natural
  grouping (a `view` operates on a `ColumnSpec`; a `download_batch` operates
  on `BatchTargets`). They impose no convention on future helpers; future
  helpers can stay under threshold without adopting any pattern.

### 3. Call-site updates

Only the three direct callers change — none of them are exported:

- `view::execute_view` (the multi-vs-single dispatcher) constructs a
  `ColumnSpec` from the already-computed `inc_canonical` / `exc_canonical` and
  passes it.
- `attachment::execute`'s `Download` arm constructs a `BatchTargets`.
- `config::execute`'s `MigrateToKeyring` arm constructs a `MigrateSpec`.

### 4. CHANGELOG

No entry. This is an internal refactor + lint-threshold change with no
user-visible behavior.

## Testing

- `cargo clippy --all-targets -- -D warnings` — passes (verifies threshold
  enforcement and absence of new lints)
- `cargo test --lib` — 1224 passing tests must still pass (no behavior change)
- `cargo fmt --check` — clean

## Risks

- **Drift on `view_single`'s `ColumnSpec` semantics.** The current
  `view_single` builds a `ColumnSpec` from canonical field names; the multi-ID
  path uses raw field names for its `ColumnSpec`. That pre-existing
  asymmetry is preserved exactly — the refactor only relocates the
  `ColumnSpec` construction one frame up the call stack with the same field
  values. Not in scope to fix here.
- **Future helpers exceeding threshold.** The `clippy.toml` setting is now
  enforced; future helpers will either need to stay ≤5 params or introduce
  their own bundling. That is the intent of the issue.
