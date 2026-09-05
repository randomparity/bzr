# Terminal-width table wrapping implementation plan

**Goal:** Wrap every human-readable grid within an explicit or detected stdout width without
changing redirected defaults or JSON-family output.

**Architecture:** `main` resolves width for its real stdout once and stores it in `Writers`;
captured/library writers default to no width and can inject one directly. Affected commands pass the
copied option into their resource grid writers. `output::formatting` owns the only completed-table
write path, including a content-preserving structural clamp. Generic resource tables already enter
through `write_table_records`; bespoke bug grids will call the same finalizer. The design is governed by
[ADR 0047](../../adr/0047-centralize-terminal-width-table-wrapping.md) and the
[specification](../specs/2026-09-04-terminal-width-table-wrapping-design.md).

**Tech stack:** Rust 1.89.0, `tabled` 0.21.0 with its existing `std` feature,
`terminal_size` 0.4.4, Bash functional harness, Docker or Podman Bugzilla.

Expected implementation size: 280–430 changed lines (M) — derived from explicit width propagation
through the affected command/resource seams, one shared resolver/finalizer, focused writer,
formatter, and bug-resource tests, one functional case, dependency metadata, and CLI docs.

## Global Constraints

- Minimum Rust version: 1.89.0.
- Exact dependency pins: `tabled = "=0.21.0"` with `default-features = false, features = ["std"]`;
  `terminal_size = "=0.4.4"`, whose declared MSRV is Rust 1.71.
- Published targets remain Linux x86_64/aarch64/powerpc64le/s390x, macOS arm64, and Windows
  x86_64/aarch64. Host architecture is arm64; effective repository instructions declare no target
  set, so the attunement relationship is `no-target-declared`.
- `BZR_TABLE_WIDTH` accepts decimal integers 1 through 65,535; invalid values warn and fall back to
  stdout detection. It overrides detection even with redirected stdout.
- Width is resolved from `OsStr` once for the real stdout destination and carried explicitly;
  captured writers default to `None` and unit tests never mutate the width environment variable.
- Requested widths are clamped to `4 * column_count + 1`, retaining one display cell, two padding
  cells per column, and every boundary; narrower grids may exceed the requested width.
- Existing 72-character summary and 60-character description truncation remain unchanged.
- JSON, NDJSON, detail/label text, error output, progress output, and persisted configuration stay
  unchanged.
- User-facing output goes through `Writers`; unit tests stay in sibling `*_tests.rs` files.
- Guardrails: `make test-one T=<name-substring>` while iterating; `make lint`; `make test`; and
  `make functional-test` for the real-container proof. CI individually gates format, test layout,
  functional IDs, no-spawn, Clippy, tests, builds, manpages, cross-checks, and platform jobs. No CI
  guard couples ADR files to `docs/adr/README.md`.
- Functional runs require Docker or Podman and take about ten minutes on a warm default-version
  image. `make test` is quiet; use `make test-verbose` only to diagnose a failure.

## File map

- Modify `Cargo.toml` and `Cargo.lock`: exact-pin terminal-size support.
- Modify `src/main.rs`: resolve table width at the stdout-owning boundary.
- Modify `src/output/writers.rs` and `src/output/writers_tests.rs`: `OsStr` parsing, terminal
  detection, stored width, and captured/injected constructors.
- Modify `src/output/formatting.rs`: shared width-clamped table finalization.
- Modify `src/output/formatting_tests.rs`: pure resolver and injected renderer contracts.
- Modify affected command and resource modules: pass `Writers::table_width()` only to grid writers.
- Modify `src/output/resources/bug.rs`: route all four bespoke grid writes through the finalizer.
- Modify `src/output/resources/bug_tests.rs`: prove each bug grid observes width policy.
- Modify `tests/functional/phases/17-global-options.sh`: real CLI table-width behavior.
- Modify `docs/bzr-cli.md`: document the environment contract.

## Task 1: Resolve width and finalize tables once

**Files:** `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/output/writers.rs`,
`src/output/writers_tests.rs`, `src/output/formatting.rs`, `src/output/formatting_tests.rs`.

**Interfaces**

- Consume existing `tabled::builder::Builder::build() -> Table` and
  `tabled::settings::Width::wrap(usize)` from tabled 0.21.0.
- Add `TableWidth(Option<usize>)` and
  `fn resolve_table_width(explicit: Option<&OsStr>, detected: Option<u16>) -> TableWidth` in
  `writers.rs`; non-Unicode values remain distinguishable and warn without printing raw bytes.
- Add `fn detected_stdout_width() -> Option<u16>` using
  `terminal_size::terminal_size_of(std::io::stdout())` on Unix/Windows and `None` elsewhere.
- Extend `Writers::new` to retain its no-width default, add a constructor/builder taking
  `TableWidth`, and add a copied `table_width() -> Option<usize>` getter.
- Add `pub(super) fn write_table<W: Write + ?Sized>(table: Table, width: Option<usize>, out: &mut W)`;
  it clamps to `4 * table.count_columns() + 1` before `Width::wrap`.
- Extend `write_table_records` with one copied `Option<usize>` argument and delegate its completed
  table plus that width to `write_table`; all other parameters and record behavior remain unchanged.

**Verification**

- Mode: focused-test — resolution precedence, invalid fallback, and Unix non-Unicode input in new
  `table_width_*` cases; before implementation they fail to compile because `TableWidth` and its
  `OsStr` resolver are absent; green command: `make test-one T=table_width_` with all matching tests
  passing.
- Mode: focused-test — captured writers default to no width while a direct injected writer retains
  its value, independent of process stdout; new `writers_table_width_*` cases fail red because
  `Writers` carries no metadata; green command: `make test-one T=writers_table_width_`.
- Mode: focused-test — bounded/unbounded and Unicode rendering in new `write_table_*` cases; before
  implementation the injected writer is absent and oversized lines remain oversized; green command:
  `make test-one T=write_table_` with every rendered line at or below the requested viable width.
- Mode: focused-test — existing unbounded generic tables retain their bytes; test
  `write_records_or_empty_prints_empty_message` plus a new populated-table assertion; expected red is
  only the new assertion before the helper exists; green command:
  `make test-one T=write_records_or_empty`.

**Steps**

1. Add resolver tests for explicit precedence, absent values, zero, malformed, out-of-range, and a
   zero detected width; run `make test-one T=table_width_` and observe unresolved function failures.
2. Add writer-context tests for captured default, direct injection, and a cfg-Unix invalid-byte
   `OsStr` whose warning is observed with `TracingCapture`; run the two writer-focused commands and
   observe missing API failures.
3. Add finalizer tests that build long ASCII/Unicode tables, assert line widths, and prove headers
   and cell content remain present below the structural minimum; run `make test-one T=write_table_`
   and observe unresolved function failures.
4. Exact-pin `terminal_size = "=0.4.4"` in `Cargo.toml` and run `cargo update -p terminal_size
   --precise 0.4.4`; expect `Cargo.lock` to contain terminal_size 0.4.4 and its platform dependencies.
5. Implement the `OsStr` resolver, cfg-gated stdout detection, stored `TableWidth`, main-boundary
   construction, structural clamp, and `Width::wrap` finalizer. Make `write_table_records` accept and
   delegate the explicit option.
6. Run the focused commands from Verification and expect all matching tests to pass.
7. Commit as `feat(output): carry terminal width with writers` after `cargo fmt -- --check` and
   `cargo clippy --all-targets --features test-helpers -- -D warnings` pass.

## Task 2: Route bespoke bug grids through the policy

**Files:** `src/output/resources/bug.rs`, `src/output/resources/bug_tests.rs`.

**Interfaces**

- Consume `output::formatting::write_table(Table, Option<usize>, &mut W)` and
  `Writers::table_width()` from Task 1.
- Extend only grid-producing resource writer signatures with a copied `Option<usize>`; preserve all
  other arguments and behavior.
- Preserve existing cell construction, selected columns, normalization, empty behavior, and
  `SUMMARY_TRUNCATE_WIDTH` use.

**Verification**

- Mode: focused-test — list, links, requests, and canonical adjacency grids share the finalizer;
  new `bug_table_width_*` tests pass `Some(width)` directly, never mutating process environment, and
  fail red because current signatures and direct writes cannot accept it; green command:
  `make test-one T=bug_table_width_`.
- Mode: focused-test — JSON/NDJSON shapes stay independent of width; existing bug JSON-family tests
  plus a new override regression assert identical structured values; red observation is the missing
  regression case, green command: `make test-one T=write_bug_links_json` and
  `make test-one T=write_bug_links_ndjson`.

**Steps**

1. Add direct-width tests for all four completed grids and one JSON-family regression; run the named
   focused commands and observe missing-argument compilation failures before implementation.
2. Propagate `w.table_width()` through only the affected commands and resource grid functions;
   update direct unit-test calls with `None` unless the test exercises width.
3. Import `write_table`, replace each direct `writeln!(..., builder.build())`, and leave record
   construction unchanged.
4. Run `make test-one T=bug_table_width_`, the two JSON-family commands, and the existing adjacency
   and list table tests; expect all to pass with preserved headers and rows.
5. Commit as `refactor(output): share wrapping across bug tables` after
   `cargo fmt -- --check` and the scoped tests pass.

## Task 3: Prove and document the operator path

**Files:** `tests/functional/phases/17-global-options.sh`, `docs/bzr-cli.md`.

**Interfaces**

- Consume public environment variable `BZR_TABLE_WIDTH` and existing `run_bzr_raw`/
  `BZR_STDOUT_RAW` functional helpers.
- Add one stable semantic test ID under group `17-global-options`.
- Add one row to the CLI reference Environment Variables table.

**Verification**

- Mode: focused-test — real `bzr bug list --output table` wraps a long server-backed summary and
  every output line stays within a viable requested width; before implementation at least one line
  exceeds the width; green command: `make functional-test` with the new semantic case passing.
- Mode: focused-test — shell structure and semantic test IDs remain valid; malformed or duplicate
  test additions fail the checks; green commands: `make check-functional-test-ids` and
  `make check-shell`.
- Mode: task-test-not-applicable — the CLI reference prose documents an already machine-tested
  environment contract; no executable consumer validates prose wording, so diff review verifies the
  row rather than snapshotting documentation.

**Steps**

1. Add a functional case that creates or reuses a long-summary bug, invokes
   `BZR_TABLE_WIDTH=60 run_bzr_raw --output table bug list` with a unique selector, and checks both a
   wrapped continuation and that no emitted line exceeds 60 display characters; run the phase via
   `make functional-test` and observe the pre-implementation width assertion fail.
2. After Tasks 1–2 are green, rerun `make functional-test`; expect the case and full default-version
   functional suite to pass.
3. Document `BZR_TABLE_WIDTH` precedence, valid range, table-only effect, and explicit redirected
   behavior in `docs/bzr-cli.md`.
4. Run `make check-functional-test-ids`, `make check-shell`, `make lint`, and `make test`; expect all
   commands to exit 0.
5. Re-read the complete diff for unchanged JSON contracts and no remaining direct table write,
   then commit as `feat(cli): add explicit table width control`.

## Rollback and cleanup

Each implementation commit is independently revertible in reverse order. Reverting all three
removes the environment contract, shared finalizer, and dependency without data migration. Stop the
functional container with `make functional-stop` after verification if the harness leaves it
running. Do not hand-edit `CHANGELOG.md`; release notes derive from the scoped conventional commits.
