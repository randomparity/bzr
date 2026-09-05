# ADR 0047: Centralize terminal-width table wrapping

## Status

Accepted

## Context

`bzr` builds grid tables in one shared formatter and four bespoke bug-output sites. None of those
sites accounts for stdout width, so a terminal hard-wraps an oversized rendered line through cell
content and borders. Redirected output must retain its deterministic, unbounded layout, while tests
and functional runs need a width source that does not require a pseudo-terminal.

Rust 1.89 provides terminal detection but no portable terminal-size query. The existing `tabled`
0.21.0 dependency provides whole-table `Width::wrap`, including Unicode display-width handling,
but it needs a width and must be applied consistently after each builder is complete.

## Decision

Add exact dependency `terminal_size = "=0.4.4"`. Resolve the optional table width once where
`main.rs` owns the real stdout destination, store it in `Writers`, and pass it explicitly only to
the resource writers that build grids. `Writers::new` retains a `None` default for library and test
buffers; an explicit constructor supplies a width without process-global environment mutation.

The stdout-bound resolver reads a positive `BZR_TABLE_WIDTH` value first. Otherwise it queries the
stdout handle with `terminal_size_of`; unsupported platforms or failed queries produce no width.
It accepts `OsStr` so an invalid-Unicode value remains distinguishable from absence, warns, and
falls back normally. No resolved width preserves today's rendering byte-for-byte.

Route the shared record writer and every bespoke bug grid through this renderer. Keep the existing
72-character summary and 60-character description content limits: they are a separate concise-list
policy, while this decision bounds the aggregate grid. JSON, NDJSON, and non-grid detail output do
not consult the width resolver.

`BZR_TABLE_WIDTH` is deliberately bzr-specific and is honored even when stdout is redirected, so
functional tests can request the same layout without a PTY. Invalid, zero, or out-of-range values
are ignored with a warning before normal stdout detection. Before applying `Width::wrap`, the
finalizer clamps the requested width to the default grid's minimum of one display cell plus two
padding cells per column and one separator per boundary: `4 * columns + 1`. This prevents tabled
0.21.0's zero-content-width behavior from erasing headers and values.

## Consequences

- Interactive grids wrap at terminal cell boundaries without each resource owning layout policy.
- Pipes and redirects remain unbounded unless the operator explicitly sets `BZR_TABLE_WIDTH`.
- The explicit override becomes a documented environment contract and a functional-test seam.
- The exact dependency adds platform implementations for Unix and Windows; unsupported platforms
  compile with an unbounded fallback.
- Very narrow widths are raised to the content-preserving structural minimum, so the valid grid may
  exceed the requested width rather than erase selected values.

## Considered & rejected

- **Implement Unix ioctl and Windows console calls in `bzr`.** judgment: this duplicates portable
  OS boundary code maintained by a focused crate and adds unsafe, platform-specific code to the CLI.
- **Re-detect stdout inside the generic table renderer.** verified: `Writers::new` can target a
  `Vec<u8>` while process stdout remains a terminal, so the detected handle would not describe the
  writer destination and captured output would become ambient-terminal-dependent.
- **Pass width through every command and every output function.** judgment: only commands whose
  resource writers build grids need the copied `Option<usize>`; unrelated output stays unchanged.
- **Honor `COLUMNS` for redirected output.** judgment: a shell-owned ambient variable would make
  otherwise deterministic pipes change layout; a bzr-specific opt-in states that intent.
- **Drop columns when the grid cannot fit.** judgment: silently omitting selected data violates the
  table-output contract; preserving the smallest valid grid is the safer failure mode.
- **Do nothing and rely on terminal hard wrapping.** verified: issue #695 records that hard wrapping
  splits cells and box-drawing borders, destroying the grid alignment the table format provides.
