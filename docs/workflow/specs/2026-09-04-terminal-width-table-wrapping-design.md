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
   must be ignored with a warning and normal detection must continue. The value is resolved once at
   the stdout-owning process boundary, never by inspecting an unrelated generic writer.
4. The shared resource table writer, `bug list`/`search`/`my`, `bug links`, and both `bug adjacency`
   grids must use one finalization helper. No direct `Builder::build()` write may remain in product
   table rendering.
5. Existing summary and description truncation limits remain unchanged. This change wraps the
   aggregate table; it does not redefine which list fields are abbreviated.
6. `--json` and `--output ndjson`, detail/label text, errors, and progress output must not consult
   or change because of table width. The process boundary must skip override parsing, warnings, and
   terminal detection entirely for JSON-family formats.
7. Unit tests must inject width directly into the renderer or pure resolver rather than depend on
   the ambient terminal. A functional test must invoke real `bzr` table output with
   `BZR_TABLE_WIDTH` and prove every emitted grid line respects the requested width for a width
   above that grid's structural minimum.

## Architecture

`src/output/writers.rs` owns `TableWidth`, parsing an optional `OsStr`, and stdout-specific
detection. After `resolve_format`, `main.rs` invokes that resolver only for `OutputFormat::Table`,
then locks stdout and constructs `Writers` with the value. JSON and NDJSON construct width-free
writers without reading `BZR_TABLE_WIDTH` or querying the terminal.
`Writers::new` defaults to no width for library callers and captured tests; tests that need a width
construct `Writers` with one directly. The copied width is passed only through command/resource
functions that render grids.

`src/output/formatting.rs` owns applying a supplied width to a completed `tabled::Table` and writing
that table. It wraps with `PriorityMax::right()`, shrinking the widest column first. Its
explicit-width seam lets layout tests avoid process-terminal assumptions and global environment
mutation.

All generic resource grids already pass through `write_table_records`; it will delegate its built
table to the finalizer. `src/output/resources/bug.rs` will replace its four direct table writes with
the same finalizer. Resource code remains responsible only for headers and cell values.

The process flow is:

```text
main(table only): BZR_TABLE_WIDTH or stdout size -> Writers.table_width
                                               |
records -> Builder -> Table -> shared finalizer(width) -> optional Width::wrap -> writer
```

## Width resolution and failures

Resolution order is `BZR_TABLE_WIDTH` then detected stdout width then `None`. The environment is
read with `std::env::var_os`. Only an `OsStr` that converts to Unicode decimal text and parses to a
non-zero `u16` is accepted. Invalid Unicode remains an explicit invalid state rather than
collapsing to absence. An invalid explicit value does not fail the command: it emits a tracing
warning and falls back to detection, matching the existing `BZR_TIMEOUT` invalid-value policy.

`terminal_size_of(std::io::stdout())` is used rather than the crate's multi-stream convenience
function. That prevents an interactive stderr or stdin from making redirected stdout behave like a
terminal. Unix and Windows use the dependency's handle-specific implementation. Other platforms
return `None` behind a local `cfg` fallback.

Before wrapping, the renderer computes `5 * table.count_columns() + 1`: two display cells and two
default-padding cells per column, plus every vertical boundary. It clamps the supplied width to that
structural minimum, then applies `Width::wrap(...).priority(PriorityMax::right())`. The priority
shrinks the widest column first until the aggregate target is reached. A column whose content begins
at one or two display cells is therefore not reduced below that natural width while a wider neighbor
still has cells to surrender; wider columns converge at no less than two content cells. The floor
and priority together avoid tabled 0.21.0's zero-content erasure and its replacement of a width-two
scalar by U+FFFD when only one content cell is available. If the terminal is narrower, the renderer
preserves all columns and individual width-one or width-two scalars, accepting minimum-width
overflow.

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

The relevant untrusted actor is a local parent process that controls the environment. An
`OsStr`-aware parse into a non-zero `u16` bounds the value before it reaches table layout; malformed
input is logged without echoing its raw bytes or other environment data and cannot fail the command.
The local OS is trusted to return a terminal cell count, which the dependency already represents as
`u16`; zero is treated as absent.

The dependency is exact-pinned at the current 0.4.4 release, recorded in `Cargo.lock`, and checked
by the repository's native and cross-target CI. It requires Rust 1.71, below this repository's
1.89.0 floor. Table cells remain ANSI-free, so `tabled`'s `ansi` feature is not enabled.

### Out of scope threats

This design does not prevent a local parent process from choosing an inconvenient but valid width;
that process already controls bzr's arguments and output destination. It does not attempt to make an
arbitrarily large number of selected columns fit into fewer cells than their borders require. It
does not alter terminal escape handling because these grid cells contain no ANSI sequences today.

## Testing

- Writer-context unit tests cover explicit-over-detected precedence, absent values, zero, malformed,
  out-of-range, detected zero, invalid Unicode on Unix, captured-writer default `None`, and direct
  injected width without environment mutation.
- Renderer unit tests cover unbounded byte compatibility, a bounded ASCII row, word wrapping, and
  Unicode display width. An exact-floor regression places a very long ASCII value beside a
  width-two scalar and asserts the original scalar remains, not merely a non-empty cell. Each line
  is checked against the injected viable width.
- Bug resource tests cover list, links, and both adjacency grids through the common finalizer,
  including non-empty cells at the structural minimum for a many-column table.
- Existing resource tests protect non-terminal snapshots because `CapturedIo::writers()` constructs
  `Writers` with no width regardless of the process stdout TTY.
- `tests/functional/phases/17-global-options.sh` runs a real bug-list table with a fixed override
  and an ASCII-only fixture under `LC_ALL=C`; byte length then equals display-cell width, so `awk`
  can assert every line is within the requested width and the long summary wraps inside the grid.
  Wide Unicode remains covered by Rust unit tests.
- The same functional phase invokes JSON and NDJSON with an invalid override and asserts their
  stdout remains valid and stderr contains no table-width warning, proving the format gate.
- Verification runs focused tests, `make lint`, `make test`, and `make functional-test` before
  shipping. Cross-platform compilation remains covered by CI's Linux, macOS, Windows, aarch64, and
  powerpc64le jobs.

## Rollback

Reverting the implementation and dependency commits restores unbounded table rendering. The change
does not migrate data or require cleanup; an unknown `BZR_TABLE_WIDTH` is inert in older binaries.
