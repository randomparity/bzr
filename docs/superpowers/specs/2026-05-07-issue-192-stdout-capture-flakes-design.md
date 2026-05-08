# Issue #192: Eliminate stdout-capture races by plumbing writers

**Issue:** [#192](https://github.com/randomparity/bzr/issues/192) — *Flaky stdout-capture tests: dup2 fd-1 redirection races concurrent writers*
**Status:** Design approved 2026-05-07
**Branch:** `feat/issue-192-stdout-capture-migration`

## Problem

`test_helpers::capture_stdout` redirects file descriptor 1 process-wide via `dup`/`dup2` to capture output written during a future. Cargo runs tests in parallel by default, so during the redirection window any concurrently running test that writes to fd 1 — `writeln!(io::stdout(), …)`, ANSI escapes from `colored`, or any of the project's many `print_*` helpers — lands its bytes in the capturing test's temp file. Three CI runs in 24 hours have flaked on this race, including on `main`.

The doc-comment on `capture_stdout` claims tests are serialized via `ENV_LOCK`. They are not — `ENV_LOCK` only serializes tests that explicitly take it, and most output-writing tests don't.

The `extract_json` helper papered over the race for tests asserting on JSON. It cannot help two cases that have flaked recently:

- `output::attachment::tests::print_attachment_batch_json_emits_typed_payload` — calls `serde_json::from_str(out.trim())` directly.
- `commands::bug::view::tests::view_multi_strict_json_failure_emits_no_partial_json` — asserts captured stdout is *empty*, which a stray byte from a concurrent writer breaks.

## Goal

Eliminate the race class entirely by removing `capture_stdout`. Replace the implicit "global stdout, captured by fd redirection" model with explicit `&mut dyn Write` references plumbed from `main` through `dispatch` through `commands::*::execute()` to the output-formatter helpers. Tests construct their own buffers; nothing is process-global.

## Non-goals

- Refactoring `BzrError` or any error variant.
- Changing the `tracing` subscriber, log routing, or `RUST_LOG` behavior.
- Adding `--no-color` plumbing. The `colored` crate already disables ANSI on non-TTY writers; tests writing into `Vec<u8>` never see escape bytes.
- Splitting oversized files or other unrelated cleanup.
- Touching `ENV_LOCK`. It serializes `XDG_CONFIG_HOME` mutation and is unrelated to the stdout race.

## Architecture

A new `Writers` newtype bundles the two output streams and rides as the final argument through every command-layer signature.

```rust
// src/output/writers.rs
pub struct Writers<'a> {
    pub out: &'a mut dyn std::io::Write,
    pub err: &'a mut dyn std::io::Write,
}

impl<'a> Writers<'a> {
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Self { out, err }
    }
}
```

Re-exported from `src/output/mod.rs` so `crate::output::Writers` is the canonical path.

A test helper replaces `capture_stdout`:

```rust
// src/test_helpers.rs
pub struct CapturedIo { pub out: Vec<u8>, pub err: Vec<u8> }

impl CapturedIo {
    pub fn new() -> Self { Self { out: Vec::new(), err: Vec::new() } }
    pub fn writers(&mut self) -> Writers<'_> {
        Writers::new(&mut self.out, &mut self.err)
    }
    pub fn out_str(&self) -> &str { std::str::from_utf8(&self.out).unwrap_or("") }
    pub fn err_str(&self) -> &str { std::str::from_utf8(&self.err).unwrap_or("") }
}
```

Each test owns its own `CapturedIo`. No shared global, no fd manipulation, no lock requirement.

### Writer-type rationale

Three options were considered for the writer parameter shape:

| Option | Shape | Pros | Cons |
|---|---|---|---|
| A | `&mut dyn Write` (one per stream) | One indirection; `dispatch` stays non-generic | Two parameters when both streams are needed |
| B | `&mut impl Write` / `<W: Write>` | Monomorphized; matches existing `output/formatting.rs` | Generics propagate through 14 `execute()` signatures and `dispatch`; contagious |
| C | `Writers<'a>` newtype with `dyn Write` inside | Single parameter; non-generic; extensible | One extra type to learn |

**Selected: C.** Avoids generic contagion at the command layer, keeps `dispatch()` a plain `pub fn`, and makes "the two streams" a single concept. The leaf `output/` helpers continue using `&mut impl Write` (they're terminal — generics aren't contagious there).

## Components and signatures

### Layer 1 — `src/output/` formatter helpers

Every `print_*` is renamed in place to `write_*` and takes `&mut impl Write` (or two `&mut impl Write` parameters when both streams are needed). This matches the existing pattern used by `output/formatting.rs::write_field`, `output/bug.rs::write_bug_detail`, and `output/attachment.rs::write_attachment_batch_table`. Generics at the leaf layer monomorphize cleanly and don't propagate contagiously, since these helpers are terminal — they call `write!`/`writeln!` and don't pass writers to anyone else. Per CLAUDE.md "replace, don't deprecate," `print_*` is not retained as a wrapper.

Example:

```rust
// before
pub fn print_attachments(items: &[Attachment], format: OutputFormat) { … }

// after
pub fn write_attachments(items: &[Attachment], format: OutputFormat, out: &mut impl Write) { … }
```

Call sites in `commands/` pass `w.out` (which is `&mut dyn Write`) — Rust accepts this because `dyn Write` itself implements `Write`, so `&mut dyn Write` satisfies `&mut impl Write` at the call boundary. No double indirection, no `Box`.

`output/attachment.rs::write_attachment_batch_table` already follows this shape and is unchanged.

### Layer 2 — `src/commands/*::execute()`

All 14 `execute()` functions gain a final `w: &mut Writers<'_>` parameter:

```rust
pub async fn execute(
    action: &XAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()>
```

This is 5 positional parameters — the project's hard limit. Acceptable here as a one-time tipping point. Any future addition to this signature must refactor to a context struct (out of scope).

The `commands::bug` submodules (`view`, `update`, `clone`, `list`, `create`, `history`, `my`, `search`) propagate the writer through their internal call graphs.

### Layer 3 — `src/lib.rs::dispatch()`

`dispatch` gains `w: &mut Writers<'_>` and forwards it to each `commands::*::execute()`. Remains non-generic.

### Layer 4 — `src/main.rs`

Locks both streams once at the top of `main`:

```rust
let stdout = io::stdout();
let stderr = io::stderr();
let mut out = stdout.lock();
let mut err = stderr.lock();
let mut writers = Writers::new(&mut out, &mut err);
bzr::dispatch(&cli, format, &mut writers).await
```

## Data flow (final state)

```
main.rs            : io::stdout().lock(), io::stderr().lock() → Writers
  → dispatch(&cli, format, &mut writers)
    → commands::X::execute(action, server, format, api, &mut writers)
      → write_*(record, w.out)        // happy path
      → write_*(error,  w.err)        // error/status path
```

All output flows through explicit writer references. No global fd state. No process-wide redirection. No race window.

## Error handling

`write!` and `writeln!` return `io::Result<()>`. Three policies were considered:

| Policy | Approach | Verdict |
|---|---|---|
| i | Bubble through `BzrError::Io` everywhere | Noisy; every formatter signature returns `Result`, `?` peppers every helper |
| ii | Discard at write sites: `let _ = writeln!(…)` | Matches CLAUDE.md output convention; broken pipe is `head` / `less q` exit, not a real bug |
| iii | Discard at leaves; flush-and-error at command boundary | Over-engineered for this codebase |

**Selected: ii.** Existing crate convention already discards write results at `writeln!(io::stdout(), …)` sites. The failure mode policy iii would catch (broken pipe at end of command) is not a meaningful CLI error.

## Testing strategy

- **Output unit tests** (`src/output/*_tests.rs`): become sync `#[test]`s, write into `&mut Vec<u8>`, assert on `String::from_utf8`. No `tokio::test`, no `ENV_LOCK`, no `#[cfg(unix)]` gate. Faster and deterministic.
- **Command-layer tests** (`src/commands/**/*_tests.rs`): construct `CapturedIo`, pass `&mut io.writers()` to `execute()`, assert on `io.out_str()` / `io.err_str()`. Still `#[tokio::test]` because of `wiremock`. Still take `ENV_LOCK` via `setup_test_env` if they touch `XDG_CONFIG_HOME` — that concern is unchanged.
- **Integration tests** (`tests/integration.rs`): same pattern as command tests, but called through `dispatch` rather than `execute`.

### Verification gates

1. `rg 'capture_stdout|extract_json' src/ tests/` returns zero hits at the end of Phase 3.
2. `rg '#\[cfg\(unix\)\]\s*\n#\[tokio::test\]' src/output/*_tests.rs` returns zero hits — output-layer tests are now sync.
3. `cargo test --locked` runs cleanly under load (`taskset -c 0,1` on a Linux CI runner).
4. Stress run before merging: 50 consecutive `cargo test --locked` invocations in a draft PR. If all pass clean, the race class is empirically gone.

## Phase plan

Each phase leaves the tree green and lands as its own PR.

### Phase 0 — Tier 1 immediate flake mitigation

Lands first as a small standalone PR ahead of the migration.

- `output::attachment_tests::print_attachment_batch_json_emits_typed_payload`: `serde_json::from_str(out.trim())` → `extract_json(&out)`.
- `commands::bug::view_tests::view_multi_strict_json_failure_emits_no_partial_json`: refactor the asserting block to use a buffer (prefigures Phase 2), or fall back to an `extract_json`-based "no JSON found" probe if the refactor is non-trivial.
- Replace the misleading `capture_stdout` doc-comment with one that accurately describes the fd-1 redirection and the deprecation path.

### Phase 1 — Scaffolding

- Add `src/output/writers.rs` with `Writers`.
- Add `CapturedIo` in `src/test_helpers.rs`.
- No callers yet. Pure additive change.

### Phase 2 — Full migration (output, command, dispatch, main, all tests)

This is the bulk of the work and **must land as a single PR**. The reason is structural: the moment `execute()` gains its `Writers` parameter, every `capture_stdout(execute(…))` test call breaks (signature mismatch). There is no intermediate state where output helpers, command signatures, and tests can be partially migrated — they all observe each other through the type system. Splitting into smaller PRs would require either a transient migration helper (rejected: violates "replace, don't deprecate") or transient `print_*` shims (same objection).

Within the single PR, the work proceeds bottom-up and the commit history may be split into reviewable chunks:

- Rename `print_* → write_*` across `src/output/*.rs`. New shape: `&mut impl Write` (or a pair of them for stdout/stderr-emitting helpers), matching the existing leaf-layer convention. ~14 output files, ~60–80 helper renames.
- Update output unit tests (`src/output/*_tests.rs`) to call `write_*` directly with `&mut Vec<u8>`. Drop `capture_stdout`, `tokio::test`, and the `#[cfg(unix)]` gate from these files. ~50 tests touched.
- Add `w: &mut Writers<'_>` to every `commands::*::execute()` (14 functions) and to internal helpers in `commands::bug` submodules that emit output.
- Command-layer call sites previously calling `print_*` now call `write_*` with `w.out` / `w.err`.
- `dispatch()` gains `w: &mut Writers<'_>` and forwards it.
- `main.rs` constructs `Writers` from locked stdout/stderr.
- Migrate command-layer tests (`src/commands/**/*_tests.rs`, `src/lib_tests.rs`, `tests/integration.rs`) from `capture_stdout(execute(…))` to:

  ```rust
  let mut io = CapturedIo::new();
  let result = execute(&action, server, format, api, &mut io.writers()).await;
  assert!(io.out_str().contains("…"));
  ```

  ~22 test files touched.
- Public API of the lib crate changes — acceptable per `lib.rs`'s docstring (lib crate is for integration tests, not external consumers).

After Phase 2, `capture_stdout` and `extract_json` have zero call sites in `src/` and `tests/`. They survive in `test_helpers.rs` only to be deleted in Phase 3.

### Phase 3 — Tier 2 cleanup + tier 3 collapse

Delete:

- `test_helpers::capture_stdout`
- `test_helpers::extract_json` and `try_parse_from`
- the `dup`/`dup2`/`close` extern block and `#[cfg(unix)]` gate
- any `test_helpers_tests.rs` cases that exercised the deleted helpers

Verify with `rg capture_stdout` returning zero hits across `src/` and `tests/`.

The original tier 3 ("lock everything" or "single-threaded test binary") becomes structurally satisfied: there is no fd-1 redirection to race against. Hardening the construct is replaced by removing it.

## Risks and rollout

- **Risk: a print-helper rename misses a call site.** The compiler catches this — `print_*` no longer exists after Phase 2.
- **Risk: a test forgets to read `io.out_str()` after the call.** Type system doesn't catch this, but the assertion patterns in migrated tests will fail visibly during Phase 2.
- **Risk: `colored` writes ANSI escapes into `Vec<u8>`.** It does not — `colored::control::SHOULD_COLORIZE` reads `is_terminal(stdout)`, which is false for a `Vec<u8>` writer. A regression assertion in Phase 2 confirms no `\x1b` byte appears in any captured output.
- **Risk: PR size.** Phase 2 is large by design — it touches ~14 output files, ~50 output tests, 14 `execute()` signatures, ~22 command-layer test files, plus `dispatch` and `main`. The alternative (splitting into smaller PRs) requires either transient migration helpers in `test_helpers` or transient `print_*` shims in `src/output/`, both of which violate "replace, don't deprecate" and leave the tree in an awkward intermediate state. Once `execute()` gains its `Writers` parameter, every test calling `execute()` must update in lockstep — there is no green-tree intermediate. Phase 3 (deletion) must remain its own PR so the cleanup is reviewable in isolation.
- **Empirical confirmation.** Phase 3 closes with the 50-iteration stress run described in *Verification gates*. If any iteration fails, the migration is not complete and the failing test must be diagnosed before issue #192 is closed.

## CHANGELOG

Per project convention, each phase that ships user-visible behavior writes its CHANGELOG entry as the work lands. Phases 0 and 3 are user-visible (CI flake fix; final removal of the deprecated helper); intermediate phases are internal refactors and do not require CHANGELOG entries unless they change observable CLI behavior (none expected).
