# 11. Structured progress stream on stderr (`--progress ndjson`)

Status: Accepted

## Context

Long-running `bzr` operations — paginated fetches (`bug list/search --paginate`,
`query run --paginate`) and batch `--from-json` create/update — give an agent no
progress signal. They look identical to a hung process until completion. Issue
#462 asks for an opt-in structured progress stream so an agent can surface
"page 5, 125 bugs so far" to its user.

The constraints that force the design:

- stdout is the parseable result contract and must stay byte-for-byte unchanged.
- Bugzilla's `Bug.search` returns no total-match count (this is *why*
  `--paginate` exists), so a total/percentage cannot be computed without
  fetching everything first.
- `bug list` reaches the pagination driver without a `CommandContext`, while
  `bug search` / `query run` carry one — an asymmetry in how the mode reaches
  the emit point.

## Decision

1. **Stream on stderr as NDJSON, opt-in via a global `--progress <FORMAT>`
   flag** (`Option<ProgressFormat>`, sole variant `Ndjson`). Events are compact
   single-line JSON discriminated by an `event` key: `page`, `batch`, `done`,
   `error`. stdout is never written by the progress path.

2. **Thread the mode through `CommandContext`** (like `--timeout`/`--retry`),
   and extend `paging::fetch_page` with `progress` + `Writers` parameters so the
   `bug list` path (which lacks `ctx`) can emit without reshaping every read
   handler. Emission is a new `output::progress` module of free functions that
   no-op when the mode is `None`.

3. **`fetched` is cumulative; `total_estimated` is omitted.** No server total is
   available, so we ship no phantom field. The running total satisfies the
   "N so far" motivation.

4. **`done` only on full success; `error` on any failure.** Partial batch
   failure (exit 11) suppresses `done`; `main.rs` emits a final
   redaction-safe `error` event (`error_type` + `exit_code`, no server message)
   before the process exits non-zero. The last progress line is therefore always
   exactly one of `done` or `error`.

5. **Accepted globally, not gated per command.** Unlike `--dry-run`, ignoring
   `--progress` is harmless, and gating would force agents to add the flag
   conditionally.

## Consequences

- Agents get incremental progress for the long operations without parsing
  stdout, and a single machine-readable failure signal.
- `fetch_page`'s signature changes (callers + tests updated); the cost is one
  module and two extra parameters on one function.
- In `--json` mode, stderr carries both the `error` event and the existing JSON
  error line; consumers filter on the `event` key.
- A future progress format slots in as a new `ProgressFormat` variant with no
  flag-surface change.

## Considered & rejected

- **A process-global static for the progress mode.** The stderr writer is a
  borrowed `&mut dyn Write` that cannot live in a static, and explicit threading
  matches the existing `CommandContext` precedent and stays unit-testable.
- **Emitting `total_estimated`/percentage.** Undeliverable without a server
  total; faking it as `null` would be a phantom field.
- **Gating `--progress` per command (the `--dry-run` precedent).** Harmless to
  ignore here, and gating hurts agent ergonomics.
- **Progress on stdout interleaved with results.** Breaks the parseable-stdout
  contract.
