# Bug Update Command Refactor — Design

**Date:** 2026-06-23
**Branch:** `refactor/bug-update-418`
**Issue:** #418 — Refactor bug update command implementation

## Background

`src/commands/bug/update.rs` is 587 lines and carries several distinct
responsibilities in one file: CLI→draft merging, field-combination validation,
API-payload construction, batch/single execution, optimistic-concurrency
guarding, confirmation prompting, and output/result formatting. It is a chronic
Desloppify reopener and a recurring file-level complexity hotspot.

The file is also a hub: three sibling modules reach into it through
`super::update::*`, and `mod.rs` dispatches to it.

### External surface that must keep resolving

Callers outside `update.rs` reference these paths today and must keep working
(behavior and call sites unchanged):

| Symbol | Used by |
|---|---|
| `update::handle` | `bug/mod.rs` |
| `update::validate_action` | `bug/mod.rs` |
| `update::resolve_comment` | `bug/verbs.rs` |
| `update::apply_checked` | `verbs.rs`, `update_json.rs` |
| `update::apply_checked_connected` | `verbs.rs`, `update_json.rs` |
| `update::ApplyRequest` | `verbs.rs`, `update_json.rs` |
| `update::ensure_batch_complete` | `create_json.rs`, `update_json.rs` |
| `update::BugUpdateDraft` | `update_json.rs` |
| `update::build_update_params_from_draft` | `update_json.rs` |
| `update::confirm_batch` | `update_json.rs` |
| `update::write_batch_result` | `update_json.rs` |
| `update::ensure_unchanged_since` | `update_json.rs` |

The sibling test file references only `super::build_update_params` (×27) and
four `FLAG_*` constants.

### Prerequisite audit (completed)

The module *path* `crate::commands::bug::update` is unchanged by the
`update.rs` → `update/mod.rs` move, so path references survive. A grep for
`bug::update` / `bug/update` and every moved symbol across `fuzz/`,
`sonar-project.properties`, coverage/CI config, and everything outside
`src/`+`docs/` returned **zero** references. The fuzz crate (outside the cargo
workspace, so `clippy --all-targets` never compiles it — a known footgun for
module/visibility refactors) does **not** reference this module or its symbols,
so no `cargo +nightly check --manifest-path fuzz/Cargo.toml` gate is required
for this change. Visibility of every symbol in the external-surface table is
nonetheless preserved exactly (below).

## Goals (success criteria)

- `src/commands/bug/update.rs` (587 lines today — the captured baseline) is
  replaced by a `src/commands/bug/update/` directory whose largest source file
  is materially smaller (target: every submodule under ~150 source lines, and
  no single function over the repo's 100-line / complexity-8 limits — already
  satisfied today, must stay satisfied).
- Every `super::update::*` path in the table above keeps resolving with **no
  edits to the calling sites** (`mod.rs`, `verbs.rs`, `update_json.rs`,
  `create_json.rs`).
- CLI surface and runtime behavior are byte-for-byte unchanged: the full
  existing `update_tests.rs` suite passes without weakening or deleting any
  assertion.
- The Desloppify structural finding for the old `update.rs` is resolved or
  materially reduced on a fresh scan. Measured objectively against the 587-line
  baseline: the file-level finding for `update.rs` no longer applies because the
  monolithic file no longer exists, and the largest replacement source file is
  well under the original (target under ~150 lines). A fresh Desloppify scan
  must not raise an equivalent file-level structural finding on any single
  replacement file.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `make check-test-layout`, `make check-no-spawn`, and the focused
  `cargo test bug::update` suite are all green.

## Non-goals

- No behavioral change, no new/renamed flags, no new public command paths
  (`docs/bzr-cli.md` is therefore untouched).
- No new generic abstractions. Every extracted unit must remove concrete
  complexity from the original file; no traits or generics introduced solely to
  "tidy" the split (issue acceptance criterion).
- No change to `verbs.rs`, `update_json.rs`, `create_json.rs`, or `mod.rs`
  source beyond what is forced by the move (target: zero forced edits).
- No redistribution of the test file into per-submodule siblings (see
  Considered & rejected). The suite tests the command's public surface and stays
  as one sibling attached to the module root.

## Module decomposition

`update.rs` → `update/` directory:

| File | Owns | Approx source lines |
|---|---|---|
| `update/mod.rs` | Submodule declarations, the `pub(super)`/`pub(crate)` re-export surface, the `handle` entry point, `validate_action`, `#[path] mod tests` link | ~70 |
| `update/draft.rs` | `BugUpdateDraft` struct, `from_cli`, `overlay_cli`, the `merge_copy`/`merge_bool_true`/`merge_vec_u64` helpers | ~150 |
| `update/validate.rs` | `validate_draft`, `validate_args` (field-combination rules) | ~40 |
| `update/payload.rs` | `FLAG_*` constants, `clean_string_list`, `id_list_update`, `string_list_update`, `resolve_comment`, `build_update_params`, `build_update_params_from_draft` | ~130 |
| `update/execute.rs` | `ApplyRequest`, `apply_checked`, `apply_checked_connected`, `apply_connected`, `update_single`, `update_batch`, `ensure_batch_complete`, `ensure_unchanged_since`, `confirm_batch` | ~150 |
| `update/output.rs` | `COMMENT_SUFFIX`/`comment_suffix`, `write_batch_result`, `write_update_dry_run` | ~60 |

### Visibility strategy

- Each submodule item used **only** within the `update` tree is `pub(super)`
  (visible to `update/mod.rs` and its descendant test module) or
  `pub(crate)` as the existing markers require.
- `update/mod.rs` re-exports the external surface so existing
  `super::update::X` paths in siblings resolve unchanged. Items consumed by
  modules **outside** `update` (the table above) are re-exported with
  `pub(super) use ...` (= `pub(in crate::commands::bug)`), matching today's
  effective reach.
- The test file keeps its `super::build_update_params` and `super::FLAG_*`
  references; `mod.rs` re-exports those names so the references resolve with no
  test edits. A `pub(super) use` re-export is an export, not flagged unused.
  **Fallback:** if a re-export trips an unused/visibility lint, or a moved
  symbol trips a different pedantic lint across the new boundary, the
  `clippy --all-targets --all-features -- -D warnings` guardrail fails before
  the commit lands; recover by qualifying the affected test references
  (`super::payload::build_update_params`, `super::payload::FLAG_*`) — a
  mechanical, behavior-neutral edit — rather than weakening the lint.

### Dependency direction (acyclic)

```
mod.rs  ──→ draft, validate, payload, execute, output
payload ──→ validate            (build_update_params_from_draft validates first)
execute ──→ payload (none direct), output, draft? (no)
execute ──→ output              (writes batch/dry-run results)
output  ──→ (leaf)
draft   ──→ runtime::shared     (merge_set/merge_vec, unchanged)
validate──→ (leaf, error only)
```

No submodule imports `mod.rs`; no cycles.

## Considered & rejected

- **Extract helpers within the single file (no directory).** Rejected: leaves
  `update.rs` a large file, so the file-level Desloppify hotspot persists. The
  issue explicitly targets the file as a "large/complexity hotspot."
- **Split the 1596-line `update_tests.rs` into per-submodule siblings.**
  Rejected for this issue: the hotspot is the source file (SonarCloud CPD and
  Desloppify already exclude `*_tests.rs`); the suite exercises the command's
  public surface (`build_update_params`, `handle` via dispatch) rather than
  submodule internals, and the shared test fixtures (`make_*`, `mock_*`) are
  used across all groups, so a per-leaf split would force a shared test-helper
  module and ~1600 lines of mechanical movement — net new risk to a
  behavior-preserving refactor with no metric benefit. Tests stay as one
  sibling attached to `update/mod.rs`.
- **Rename any external symbol or change a call site to "improve" naming.**
  Rejected: out of scope; widens the diff and the regression surface for a
  pure structural move.
- **Introduce a trait/generic "field merger" abstraction.** Rejected by the
  issue's "avoid generic abstractions that do not remove concrete complexity."
