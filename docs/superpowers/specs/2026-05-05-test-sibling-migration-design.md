# Test Sibling Migration — Design

**Date:** 2026-05-05
**Branch:** stacked across `refactor/test-siblings-1-client` … `refactor/test-siblings-7-toplevel-and-policy`
**Related:** `docs/specs/2026-04-27-sonar-refactor-design.md`, `sonar-project.properties`

## Background

The project has been gradually moving test code out of `#[cfg(test)] mod tests { ... }` blocks embedded in production source files and into sibling `<name>_tests.rs` files linked via:

```rust
#[cfg(test)]
#[path = "<name>_tests.rs"]
mod tests;
```

This pattern was introduced for a specific operational reason — see commit `1527039` ("chore: split test mods to sibling _tests.rs files for CPD exclusion") and continued in `e871679`, `1128ae9`, `f137d28`. SonarCloud's copy-paste detector (CPD) was flagging per-handler test fixture boilerplate (wiremock setup, action constructors, helpers) as duplication and failing the new-code duplication gate. The fix was to move test code to sibling files and exclude `**/*_tests.rs` from CPD via:

```properties
# sonar-project.properties
sonar.cpd.exclusions=**/*_tests.rs
```

That exclusion is already in place. What is missing is **completion**: of the 79 source files in `src/` that contain `#[cfg(test)]` test code, only 12 have been migrated to siblings. 67 source files still carry inline `mod tests { ... }` blocks, and Sonar continues to flag duplication in those files even though the project has explicitly decided not to deduplicate test-fixture code (per the "no premature abstraction" principle in `CLAUDE.md` — bad abstractions are worse than honest duplication).

This spec describes (a) the migration of the remaining 67 files, and (b) the permanent repo policy and CI guardrail that prevent regression.

## Why this rule exists

Two distinct reasons, both load-bearing:

**1. SonarCloud / CPD interaction.** Test-fixture duplication is structurally different from production duplication. Production duplication is a maintenance risk — when behavior needs to change, every copy must be updated in lockstep, and inconsistent updates cause bugs. Test-fixture duplication is the opposite: each test is supposed to be self-contained and readable in isolation, and forcing a shared abstraction (e.g., a test-builder helper) couples unrelated tests so that one test's change can break unrelated tests. The project has explicitly chosen to allow test-fixture duplication. Sonar's CPD scanner does not have a policy primitive for "test code is OK to duplicate"; the only available lever is path-based exclusion. Sibling `_tests.rs` files give us a path pattern to exclude.

**2. Source-file focus and tool legibility.** Production source files in this repo are kept small and focused (see `CLAUDE.md`'s hard limits and the layered CLI architecture). When a 200-line production module carries a 700-line test block, the file becomes a ~900-line file in every tool's view — `wc`, `git log --stat`, IDE outlines, code review diffs, LLM context windows, the lot. Splitting tests to siblings restores the focus.

These two reasons together justify a **strict** rule with no size threshold. A 5-line test mod and a 700-line test mod both contribute to both problems. A threshold rule would also require a judgment call on every new test, which is operational drag.

## The rule

Every `#[cfg(test)]` test module in `src/` MUST live in a sibling `<name>_tests.rs` file linked from the source file via:

```rust
#[cfg(test)]
#[path = "<name>_tests.rs"]
mod tests;
```

Inline `#[cfg(test)] mod tests { ... }` blocks are not permitted in `src/`. There is no size threshold and no exception path.

The same separation applies to test-only helper modules (modules gated on `#[cfg(test)]` that exist purely to support tests). Example: the helpers currently inlined at `src/client/mod.rs` move to `src/client/test_helpers.rs`, declared as `#[cfg(test)] pub(super) mod test_helpers;` in `src/client/mod.rs`. (No `#[path]` is needed when the filename matches the module name.)

The `tests/` directory (integration, functional, installer) is unaffected — it already uses separate files and is not scanned by Sonar's CPD with the exclusion above.

## The guardrail

A `make` target — wired into `make lint`, the existing `prek` pre-commit config, and CI — that fails the build on any inline test module in `src/`:

```makefile
check-test-layout: ## Verify all test code lives in sibling *_tests.rs files
	@if rg -l '^mod tests \{' src/ 2>/dev/null; then \
	  echo "ERROR: inline 'mod tests { ... }' blocks found in src/."; \
	  echo "Move tests to a sibling <name>_tests.rs file linked via"; \
	  echo "  #[cfg(test)] #[path = \"<name>_tests.rs\"] mod tests;"; \
	  echo "See docs/superpowers/specs/2026-05-05-test-sibling-migration-design.md"; \
	  exit 1; \
	fi
```

The regex `^mod tests \{` (start of line, literal `mod tests {`) matches only the inline form. The sibling form `mod tests;` on its own line passes. The check is anchored to start-of-line with no leading whitespace, so it matches only file-scope module declarations and ignores doc-comment text.

CI integration: a fast lint step in `.github/workflows/ci.yml` runs `make check-test-layout` early — well before `cargo build` — so violations fail in seconds.

A short-form description of the rule is also added to the project `CLAUDE.md` under `### Conventions`, with a back-reference to this document.

## Mechanics of a single-file migration

For each source file `src/path/foo.rs` with an inline `#[cfg(test)] mod tests { ... }`:

**Before — `src/path/foo.rs`:**
```rust
// production code...

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    // ... tests
}
```

**After — `src/path/foo.rs`:**
```rust
// production code...

#[cfg(test)]
#[path = "foo_tests.rs"]
mod tests;
```

**After — new file `src/path/foo_tests.rs`:**
```rust
#![expect(clippy::unwrap_used)]

use super::*;
// ... tests (verbatim from old block)
```

Mechanical rules:

1. **Inner attributes.** `#[expect(...)]` / `#[allow(...)]` on the original `mod tests` line become file-level `#![expect(...)]` / `#![allow(...)]` at the top of the sibling file.
2. **`use super::*;`** stays — it now refers to the parent module of the `mod tests;` declaration, which is the same scope as before.
3. **No content edits.** Test bodies, helpers, and fixtures are copied verbatim. Indentation drops one level (the outer `mod tests { }` braces are gone); `cargo fmt` handles this.
4. **`mod.rs` files** become `mod_tests.rs` (e.g., `src/client/mod.rs` → `src/client/mod_tests.rs`). Already established in the partial migration.
5. **Top-level files.** `src/lib.rs` → `src/lib_tests.rs`; `src/main.rs` → `src/main_tests.rs`. `#[path]` resolves relative to the source file, so this works.
6. **`src/test_helpers.rs`** itself contains a small `#[cfg(test)] mod tests { ... }` block that tests the helpers; it gets the same treatment, producing `src/test_helpers_tests.rs`.

Edge case — `src/client/mod.rs` test_helpers extraction:

The current `pub(super) mod test_helpers { ... }` block at `src/client/mod.rs:565–620` becomes its own file `src/client/test_helpers.rs` containing the body of the module. In `src/client/mod.rs` it is replaced by:

```rust
#[cfg(test)]
pub(super) mod test_helpers;
```

No `#[path]` directive is needed because the filename matches the module name. Importers (the existing `src/client/*_tests.rs` files) keep `use crate::client::test_helpers::{...};` unchanged — visibility is preserved because `pub(super)` resolves identically across the file boundary.

## Migration plan

67 files migrated across 7 waves, each landing as one PR. Sequencing puts the `client/test_helpers.rs` extraction first (other tests' fixture imports depend on it) and the guardrail last (so its first run is on a green tree). Each wave is one branch off the latest `main`, one PR.

| # | Wave | Files | ~test lines | Branch |
|---|---|---|---|---|
| 1 | `client/` + `client/auth/` + extract `client/test_helpers.rs` + this design doc + `.gitignore` change | 10 + helper move + docs | ~1,328 | `refactor/test-siblings-1-client` |
| 2 | `commands/` + remaining `commands/bug/` | 17 | ~3,247 | `refactor/test-siblings-2-commands` |
| 3 | `output/` | 13 | ~2,337 | `refactor/test-siblings-3-output` |
| 4 | `types/` | 8 | ~1,059 | `refactor/test-siblings-4-types` |
| 5 | `xmlrpc/` | 5 | ~1,321 | `refactor/test-siblings-5-xmlrpc` |
| 6 | `tls/` + `credentials/` | 6 | ~834 | `refactor/test-siblings-6-tls-credentials` |
| 7 | top-level + `cli/mod.rs` + guardrail + `CLAUDE.md` paragraph | 8 + policy | ~2,946 | `refactor/test-siblings-7-toplevel-and-policy` |

### Wave 1 file list (and rationale)

Wave 1 carries the policy doc, the `.gitignore` change (allowing specs to be committed while keeping plans ignored), the `client/test_helpers.rs` extraction, and these source migrations:

- `src/client/classification.rs`
- `src/client/component.rs`
- `src/client/field.rs`
- `src/client/product.rs`
- `src/client/server.rs`
- `src/client/user.rs`
- `src/client/version.rs`
- `src/client/auth/mod.rs`
- `src/client/auth/valid_login.rs`
- `src/client/auth/whoami.rs`

The doc lands in Wave 1, not Wave 7, so reviewers of every subsequent wave have the rationale already in `main`.

### Per-wave checklist

1. Branch from latest `main`.
2. Migrate each file per the mechanical rules above.
3. After each file: `cargo test --lib <module-path>` for that file's tests.
4. After all files in the wave: `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test`.
5. Commit. Format: `refactor(test-layout): extract <wave> tests to sibling _tests.rs files`. One commit per wave is the default; split into sub-commits within the PR if the diff is unwieldy (e.g., Wave 2's `commands/` + `commands/bug/` could be two commits).
6. Open PR; link to this design doc.
7. Merge; update this doc's migration-plan table with a status line for the wave; start the next wave.

### Wave 7 specifics

Wave 7 lands the guardrail. Order within the PR matters:

1. Migrate the 8 remaining files first (top-level: `config`, `error`, `http`, `lib`, `main`, `test_helpers`, `url_parser`; plus `cli/mod.rs`).
2. Then add the `Makefile` target, the CI step in `.github/workflows/ci.yml`, the `prek` hook entry, and the `CLAUDE.md` paragraph — all in the same commit.

Result: when the guardrail runs for the first time, the tree is already compliant.

## Success criteria

Verifiable on a green Wave-7 build:

1. `rg -l '^mod tests \{' src/` returns no matches.
2. Every `#[cfg(test)]` line in `src/` is paired with `#[path = "..._tests.rs"]` (or the test-helpers form), with no inline body.
3. `cargo test` passes; the lib + bin + integration test counts are unchanged from the pre-migration baseline (no tests dropped).
4. `cargo clippy --all-targets --all-features -- -D warnings` clean.
5. `cargo fmt --check` clean.
6. `src/client/mod.rs` no longer contains the inline `pub(super) mod test_helpers { ... }` block.
7. `make check-test-layout` exits 0.
8. CI step `make check-test-layout` runs and passes on `main`.
9. SonarCloud's next scan post-Wave-7 shows duplication density materially lower (the larger test footprint is now CPD-excluded).
10. `CLAUDE.md` documents the convention; this design doc is referenced from there.

## Non-goals

- No changes to test *content* — pure mechanical move. If a test currently passes, it passes after; if it has a bug, that bug stays.
- No new test coverage, no test rewrites, no fixture refactoring.
- No changes to `tests/` (integration, functional, installer) — already separate files, unaffected.
- No changes to `sonar-project.properties` — `**/*_tests.rs` is already excluded; the migration just feeds more files into that exclusion.
- No `git mv` history preservation. These are *splits*, not renames; the original file's history stays attached to the production code, and `git log --follow` does not apply to the new sibling.
- No reorganization of the `tests/` directory layout.

## Risks and rollback

- **Risk:** a sibling file's `use super::*;` resolves to a different scope than the inline tests' did, due to a nested `mod tests` or unusual visibility setup. **Mitigation:** the per-file `cargo test --lib <module-path>` after every move catches this immediately; nothing lands red.
- **Risk:** very small inline test mods produce tiny `_tests.rs` siblings (e.g., 5-line files in `types/`). **Accepted:** the strict policy is the point — see "Why this rule exists" above.
- **Risk:** Wave 1's `client/test_helpers.rs` extraction subtly changes visibility (`pub(super)` semantics across a file boundary). **Mitigation:** the existing `client/*_tests.rs` files already import `crate::client::test_helpers::{...}`; if those compile after the move, visibility is intact. The compiler will catch any regression.
- **Rollback:** each wave can be reverted independently. The policy doc lands in Wave 1 and the guardrail in Wave 7, so reverting a middle wave doesn't undo either; reverting Wave 7 turns the guardrail off without disturbing the migration; reverting Wave 1 takes the doc with it but is otherwise local.

## References

- `sonar-project.properties` — current `sonar.cpd.exclusions=**/*_tests.rs`.
- `docs/specs/2026-04-27-sonar-refactor-design.md` — earlier Sonar refactor; established that test-fixture duplication is intentional and that bad abstractions are worse than the duplication.
- Commit `1527039` — "chore: split test mods to sibling _tests.rs files for CPD exclusion" — first sibling migration batch and rationale.
- Commit `e871679` — "refactor(client): extract attachment and mod tests to sibling files".
- Commit `1128ae9` — "test(comment): extract tests to comment_tests.rs sibling file".
- Commit `f137d28` — "refactor(mutants): move inline test mods to sibling *_tests.rs files".
- `CLAUDE.md` — project conventions; `### Conventions` block where the short-form rule is added.
