# Issue #462 — Structured progress stream (`--progress ndjson`)

Status: Draft
Issue: https://github.com/randomparity/bzr/issues/462
Related ADR: [0011](../../adr/0011-progress-ndjson-stream.md)

## Problem

Long-running `bzr` operations give an agent no signal of progress. A
`bug list --product Huge --paginate --json` that walks 20 pages, or a
`bug create --from-json batch.json` that posts 200 bugs, is indistinguishable
from a hung process until it completes. The agent cannot tell its user "page 5
of ~20, 125 bugs so far".

## Goal

Add an opt-in `--progress ndjson` global flag that emits structured progress
events as newline-delimited JSON on **stderr**, one event per line, while a
long operation runs. stdout (the parseable result document) is never touched.
Absent the flag, behavior is byte-for-byte unchanged.

## Non-goals

- No progress on short, single-request operations (`bug view`, `bug list`
  without `--paginate`, `config` reads). There is nothing long to report.
- No progress format other than `ndjson`. The flag takes a value so a future
  format can be added without a breaking change, but only `ndjson` exists now.
- No total-count estimation (see "Resolved ambiguities").

## Surface

```
bzr bug list --product Huge --paginate --json --progress ndjson 2>progress.ndjson
bzr bug search 'foo' --paginate --progress ndjson
bzr query run big --paginate --progress ndjson
bzr bug create --from-json batch.json --progress ndjson
bzr bug update --from-json batch.json --progress ndjson
```

`--progress <FORMAT>` is a global clap flag (`global = true`), accepted after any
subcommand, parsed into `Option<ProgressFormat>` where `ProgressFormat::Ndjson`
is the only variant. It mirrors `--api`/`--output`: a small enum with `FromStr`
+ `Display`, no `total`-style validation.

## Event schema

All events are compact single-line JSON on stderr, discriminated by the `event`
key.

### `page` — emitted after each fetched page (pagination paths)

```json
{"event":"page","n":2,"fetched":100}
```

- `n` — 1-based page number just fetched.
- `fetched` — **cumulative** bugs fetched so far (not per-page). The motivation
  ("125 bugs so far") needs a running total; see "Resolved ambiguities".

### `batch` — emitted after each item (batch create/update array form)

```json
{"event":"batch","n":3,"total":10,"ok":2,"failed":1}
```

- `n` — 1-based index of the item just processed.
- `total` — total items in the batch.
- `ok` / `failed` — cumulative successes / failures so far.

### `done` — emitted last, only on full success

```json
{"event":"done","fetched":500}
```

- `fetched` — total records produced: bugs fetched (pagination) or items
  processed (batch). Emitted as the final event when the operation fully
  succeeds. **Not** emitted on a partial batch failure (that exits 11 and is a
  failure, not a success — see `error`).

### `error` — emitted on any failure, before the process exits non-zero

```json
{"event":"error","error_type":"http","exit_code":5}
```

- `error_type` — the `BzrError::error_type()` string (the same value the JSON
  error object carries as `type`).
- `exit_code` — the `BzrError::exit_code()` the process will return.
- No `message`: the human/structured error message is already written to stderr
  by the existing error path; the progress `error` event is a minimal,
  redaction-safe machine signal that does not echo untrusted server text.

## Where events are emitted

| Operation | Trigger | Events |
|-----------|---------|--------|
| `bug list --paginate` | `paging::fetch_page` (paginate branch) | `page`*, `done` |
| `bug search --paginate` | same | `page`*, `done` |
| `query run --paginate` | same | `page`*, `done` |
| `bug create --from-json` array | `create_json` batch loop | `batch`*, `done` |
| `bug update --from-json` array | `update_json` batch loop | `batch`*, `done` |
| any command, on failure | `main.rs` after `dispatch` returns `Err` | `error` |

`bug update --from-json` is included even though acceptance criterion 2 lists
only create: the "What to build" section names "batch `--from-json`
creates/updates", the update batch loop is structurally identical, and omitting
it would be a surprising asymmetry. This is an intentional superset of the
minimum.

## Threading the flag down

`ProgressFormat` is stored on `CommandContext` (`progress: Option<ProgressFormat>`,
`with_progress`/`progress` accessors), set in `lib.rs::build_command_context`
from `cli.progress`, exactly like `--timeout`/`--retry`. The batch handlers
already hold `ctx`, so they read `ctx.progress()` directly.

The pagination path is asymmetric: `bug search` / `query run` reach
`paging::fetch_page` through `search::execution::execute` (which has `ctx`), but
`bug list` reaches it through `list::handle(&client, args, format, w)` (no
`ctx`). Rather than reshape every read handler, `fetch_page` gains two
parameters — `progress: Option<ProgressFormat>` and `w: &mut Writers<'_>` — and
each caller passes `ctx.progress()` (threaded into `list::handle` as one extra
parameter). Emission lives in a new `output::progress` module as small free
functions that no-op when the mode is `None`, writing to `w.err`.

Rejected alternative: a process-global static for the mode. The stderr writer
is a borrowed `&mut dyn Write` and cannot live in a static; passing the mode
explicitly matches the existing `CommandContext` pattern and stays testable.

## Ordering and failure guarantees

- On success, `done` is the final progress event.
- On a pagination error (network failure, safety-cap exhaustion), no `done` is
  emitted; `main.rs` emits the final `error` event.
- On a partial batch failure (exit 11), the per-item `batch` events are emitted,
  `done` is **suppressed**, and `main.rs` emits the final `error` event. So the
  last progress line is always `done` (full success) or `error` (any failure),
  never both.
- The existing human/JSON error line still goes to stderr after the `error`
  event. A progress consumer keys on the `event` field and ignores the trailing
  non-`event` error line.

## Resolved ambiguities (hidden assumptions in the issue)

1. **`total_estimated` is undeliverable.** Bugzilla's `Bug.search` returns no
   total-match count (the reason `--paginate` exists at all, per `paging.rs`
   module docs). The issue's `"total_estimated":500` cannot be computed without
   fetching everything first, which defeats streaming. We therefore omit the
   key entirely rather than ship a perpetually-`null` phantom field.
2. **`fetched` is cumulative, not per-page.** The issue example
   (`n:2, fetched:50`) reads as per-page, but the stated motivation ("125 bugs
   so far") is a running total. Cumulative is the useful contract; the example
   numbers are illustrative.
3. **`--progress` is accepted globally, not gated per command.** Unlike
   `--dry-run` (which is rejected on unsupported commands because silently
   ignoring it risks an unintended write), ignoring `--progress` on a
   non-long-running command is harmless. Gating it would force agents to add the
   flag conditionally — anti-agent-native. Commands with no long operation
   simply emit no events (except `error` on failure).

## Acceptance criteria → coverage

- Events on stderr only; stdout unchanged → emission writes solely to `w.err`;
  wiremock test asserts `out` is the clean document and `err` carries events.
- Supported on the four (+update) operations → table above.
- Events match documented shapes; `done` always last on success → schema +
  ordering section; serde-derived event structs guarantee shape.
- Final `error` event on failure → `main.rs` emission.
- Absent `--progress`, unchanged → free functions no-op on `None`; existing
  tests pass unmodified except for the new `fetch_page` parameters.
- Wiremock test for ordering + stdout cleanliness → `paging_tests.rs`,
  `create_json_tests.rs`, `update_json_tests.rs`, `progress_tests.rs`.
- `docs/bzr-cli.md` documents the flag; `bzr-reference` notes it → docs tasks.
- `commands.yml` unchanged → no new verbs.

## Testing

- Unit (sibling `*_tests.rs`): `output/progress_tests.rs` asserts each event's
  exact JSON and the `None` no-op.
- Wiremock: paginated `fetch_page` emits ordered `page`+`done` to `err` with
  clean `out`; batch create/update emit `batch`+`done`, and partial failure
  emits no `done`.
- CLI parse: `cli` test that `--progress ndjson` parses and a bad value errors.
- Functional: extend a phase script to run `bug list --paginate --progress
  ndjson` and a batch `bug create --from-json --progress ndjson` against a real
  container, asserting stderr carries `"event":"done"` and stdout parses.

## Guardrails

`cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test`, `make check-test-layout`, `make skills-test` (flag-drift), and a
functional run.
