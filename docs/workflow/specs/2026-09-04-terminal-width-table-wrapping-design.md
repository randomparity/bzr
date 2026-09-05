# Terminal-width table wrapping design

Issue: [#695](https://github.com/randomparity/bzr/issues/695)  
Decision: [ADR 0047](../../adr/0047-centralize-terminal-width-table-wrapping.md)

## Goal

Keep every grid produced by `--output table` readable at a terminal by wrapping cells inside the
detected width, while preserving deterministic redirected output and leaving JSON-family formats
unchanged.

## Requirements

1. When stdout is a terminal and its width can be detected, each rendered grid must apply that
   width before writing. Cell content may occupy multiple display lines; borders and column
   boundaries must remain structurally valid.
2. When stdout is not a terminal, or terminal-size detection fails, table rendering must remain
   byte-for-byte compatible with the current unbounded path unless an explicit override is set.
3. A positive `BZR_TABLE_WIDTH` integer from 1 through 65,535 must override terminal detection,
   including for redirected stdout. Invalid Unicode, non-integer, zero, and out-of-range values
   must be ignored with a warning and normal detection must continue.
4. The shared resource table writer, `bug list`/`search`/`my`, `bug links`, and both `bug adjacency`
   grids must use one finalization helper. No direct `Builder::build()` write may remain in product
   table rendering.
5. Existing summary and description truncation limits remain unchanged. This change wraps the
   aggregate table; it does not redefine which list fields are abbreviated.
6. `--json` and `--output ndjson`, detail/label text, errors, and progress output must not consult
   or change because of table width.
7. Unit tests must inject width directly into the renderer or pure resolver rather than depend on
   the ambient terminal. A functional test must invoke real `bzr` table output with
   `BZR_TABLE_WIDTH` and prove every emitted grid line respects the requested width for a width
   above that grid's structural minimum.

## Architecture

`src/output/formatting.rs` owns three related operations: resolving the optional width, applying it
to a completed `tabled::Table`, and writing that table. Production resolution reads the explicit
override and otherwise asks `terminal_size` about stdout. A pure resolver and an explicit-width
rendering seam let tests cover precedence and layout without process-terminal assumptions.

All generic resource grids already pass through `write_table_records`; it will delegate its built
table to the finalizer. `src/output/resources/bug.rs` will replace its four direct table writes with
the same finalizer. Resource code remains responsible only for headers and cell values.

The process flow is:

```text
records -> Builder -> Table -> shared finalizer -> optional Width::wrap -> stdout writer
                                      ^
                         BZR_TABLE_WIDTH or stdout size
```

## Width resolution and failures

Resolution order is `BZR_TABLE_WIDTH` then detected stdout width then `None`. Only decimal values
that parse to non-zero `u16` are accepted. An invalid explicit value does not fail the command: it
emits a tracing warning and falls back to detection, matching the existing `BZR_TIMEOUT` invalid
environment-value policy.

`terminal_size_of(std::io::stdout())` is used rather than the crate's multi-stream convenience
function. That prevents an interactive stderr or stdin from making redirected stdout behave like a
terminal. Unix and Windows use the dependency's handle-specific implementation. Other platforms
return `None` behind a local `cfg` fallback.

`Width::wrap` shrinks content until the table reaches the requested width or its structural minimum.
If borders, padding, and at least one display cell per selected column already exceed the width, the
renderer preserves all columns and the valid grid at that minimum. It never drops data columns,
panics, or turns the table into malformed text.

## Public contract

`BZR_TABLE_WIDTH` is a new environment variable documented in `docs/bzr-cli.md`. It affects only
table grids. It intentionally works with redirected stdout so scripts and the functional harness can
request a fixed human-readable layout. Unset or invalid values preserve the current non-terminal
behavior.

No CLI flag, config key, JSON schema, exit code, or persisted data changes.

## Security and trust boundaries

### Boundary inventory

- Added boundary: a local operator or parent process supplies `BZR_TABLE_WIDTH` as untrusted text.
- Added boundary: `bzr` asks the local OS terminal handle for a width through `terminal_size`.
- Added supply-chain boundary: `terminal_size` and its locked transitive platform crates enter the
  build.
- Widened boundaries: none; no network, credential, Bugzilla, or file-content boundary changes.

### Actor model and controls

The relevant untrusted actor is a local parent process that controls the environment. Parsing into a
non-zero `u16` bounds the value before it reaches table layout; malformed input is logged without
echoing other environment data and cannot fail the command. The local OS is trusted to return a
terminal cell count, which the dependency already represents as `u16`; zero is treated as absent.

The dependency is exact-pinned at the current 0.4.4 release, recorded in `Cargo.lock`, and checked by
the repository's native and cross-target CI. It requires Rust 1.71, below this repository's 1.89.0
floor. Table cells remain ANSI-free, so `tabled`'s `ansi` feature is not enabled.

### Out of scope threats

This design does not prevent a local parent process from choosing an inconvenient but valid width;
that process already controls bzr's arguments and output destination. It does not attempt to make an
arbitrarily large number of selected columns fit into fewer cells than their borders require. It
does not alter terminal escape handling because these grid cells contain no ANSI sequences today.

## Testing

- Resolver unit tests cover explicit-over-detected precedence, absent values, zero, malformed,
  out-of-range, and detected zero.
- Renderer unit tests cover unbounded byte compatibility, a bounded ASCII row, word wrapping, and
  Unicode display width. Each line is checked against the injected width.
- Bug resource tests cover list, links, and both adjacency grids through the common finalizer,
  including the structural-minimum behavior for a many-column table.
- Existing resource tests protect non-terminal snapshots because the production resolver returns
  `None` under captured stdout when no explicit override is present.
- `tests/functional/phases/17-global-options.sh` runs a real bug-list table with a fixed override and
  asserts that all lines are within the requested width and the long summary wraps inside the grid.
- Verification runs focused tests, `make lint`, `make test`, and `make functional-test` before
  shipping. Cross-platform compilation remains covered by CI's Linux, macOS, Windows, aarch64, and
  powerpc64le jobs.

## Rollback

Reverting the implementation and dependency commits restores unbounded table rendering. The change
does not migrate data or require cleanup; an unknown `BZR_TABLE_WIDTH` is inert in older binaries.
