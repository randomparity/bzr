# Issue #368: Query Run Count Design

## Context

Issue #368 asks for `bzr query run <NAME> --count` to match the count-only
behavior already available on `bug list`, `bug search`, and `bug my`.

`query run` already converts saved queries into `SearchParams`, applies runtime
overrides, resolves URL-sourced offsets, applies deterministic ordering, and then
uses the same Bugzilla search client as the bug commands. The missing piece is
the CLI flag and the count branch.

## Design

Add `--count` to `query run`.

The handler should:

- reject `--count` with `--offset` or `--paginate` using the existing
  `ensure_no_paging_with_count` helper;
- apply saved-query and runtime overrides exactly as today, including date/filter
  validation;
- when counting, rewrite the resolved `SearchParams` with the existing
  `count_search_params` helper so only ids are fetched and `limit=0` lifts the
  client-side row limit;
- clear saved URL offsets in the count helper so URL-sourced saved queries count
  the full match set instead of a saved result window;
- skip field-selection validation, output column selection, paging fetch, and
  truncation notes in the count branch;
- print through the existing `write_count` helper, preserving bare table integer
  and `{"count": N}` JSON/NDJSON output.

Saved/per-run `--fields` and `--limit` are accepted but ignored for the count
result, matching the established bug command count semantics. Saved/per-run sort
settings may still be carried into the search params, as they are for existing
count paths, but they do not affect the count output.

## Files

- `src/cli/query.rs`: add the `count` flag to `RunArgs` and help text.
- `src/commands/query.rs`: reject paging/count conflicts and add the count
  branch.
- `src/cli/mod_tests.rs`: prove the flag parses and conflicts with paging.
- `src/commands/query_tests.rs`: prove JSON and table count output, id-only
  search params, lifted limit, saved URL offset clearing, and conflict
  validation.
- `docs/bzr-cli.md`: document `query run --count`.
- `CHANGELOG.md`: add an Unreleased entry.

## Testing

- `cargo test parse_query_run_count`
- `cargo test query_run_count --lib`
- `cargo test commands::query::tests --lib`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `git diff --check`

## Out of Scope

- Adding server-side total-count APIs; Bugzilla search has no separate total
  endpoint.
- Changing saved-query storage shape.
- Changing `bug list`, `bug search`, or `bug my` count behavior.

## Self-Review

- The design reuses the existing count helpers and output shape.
- Count happens after saved-query resolution and override validation, so invalid
  dates and malformed field override syntax keep existing behavior where relevant.
- The count branch avoids table/JSON field projection because the result is not a
  bug list.
- The shared count helper clears offset so saved URL windows do not leak into
  count-only requests.
