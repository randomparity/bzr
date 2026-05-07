# Issue #157: `bug list` date-range filters

**Date:** 2026-05-06
**Issue:** [#157](https://github.com/randomparity/bzr/issues/157)
**Surfaced by:** `docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md` (Issue B)

## 1. Summary

Add `--created-since` and `--changed-since` ISO-8601 date filters to
`bzr bug list`, and the matching pair to `bzr query save` and
`bzr query run` (override). Filters are validated client-side and
rejected with exit code 7 on malformed input. While building the
shared validator, retrofit the existing `bzr bug history --since`
and `bzr comment list --since` flags to use it.

## 2. Motivation

Common tester workflow: "find bugs filed/modified since DATE."
`bzl-search` exposes both filters
(`reference/bzl/bzl-search:77-93`); `bzr bug list` has neither.
Without them, testers fall back to ad-hoc `--from-url` parsing or
client-side filtering of the full result set.

## 3. Scope

In scope:

- Two new CLI flags on `bzr bug list`, `bzr query save`, and
  `bzr query run` (the last as overrides).
- A shared client-side validator for ISO-8601 dates.
- Retrofit of `bzr bug history --since` and
  `bzr comment list --since` to use the same validator.
- REST and XML-RPC encoding for both new `SearchParams` fields.
- Unit, integration, and one functional test.
- `docs/bzr-cli.md` and `CHANGELOG.md` updates.

Out of scope:

- Mapping `chfieldfrom`/`chfieldto` from URL imports (`--from-url`)
  to the new structured fields. URL imports continue to pass these
  through `SavedQuery.raw_params` verbatim, as today.
- The other field filters surfaced under bzl-parity Issue C
  (`--whiteboard`, `--target-milestone`, etc.).
- Any new date-handling crate dependency. The validator is
  hand-rolled (~30 lines) and accepts only the two forms below.

## 4. CLI surface

### 4.1 New flags

```
--created-since <DATE>   Filter to bugs created at or after DATE
--changed-since <DATE>   Filter to bugs last modified at or after DATE
```

Both are single-value `Option<String>`. Both apply to:

- `bzr bug list` — primary surface.
- `bzr query save` — stores them on the saved query.
- `bzr query run` — overrides whatever the saved query carries.
  Override semantics match the existing `--limit` / `--fields` /
  `--exclude-fields` / `--server` overrides: `Some(_)` from the CLI
  replaces the saved value; `None` keeps the saved value. There is
  no "clear" sentinel.

The two filters AND with each other and with all existing filters.
Either, both, or neither may be set.

### 4.2 Accepted DATE formats

Validated client-side, before any network call:

1. `YYYY-MM-DD` — bare date. Canonicalized to
   `YYYY-MM-DDT00:00:00Z` before being sent.
2. `YYYY-MM-DDTHH:MM:SS` — no zone. Sent verbatim; the server
   treats it as UTC.
3. `YYYY-MM-DDTHH:MM:SSZ` — sent verbatim.
4. `YYYY-MM-DDTHH:MM:SS+HH:MM` (or `-HH:MM`) — sent verbatim.

The `T` separator is required when a time component is present
(no space). Fractional seconds, week dates (`2026-W18-3`), and
ordinal dates (`2026-128`) are rejected — they are valid ISO-8601
but the server isn't guaranteed to accept them, and we'd rather
fail fast at the CLI than ship a malformed payload.

### 4.3 Error message shape

Validator failures produce `BzrError::InputValidation` (exit
code 7) with this shape:

```
--created-since: 'tomorrow' is not a valid ISO-8601 date or datetime.
Expected: YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, YYYY-MM-DDTHH:MM:SSZ, or YYYY-MM-DDTHH:MM:SS±HH:MM
```

The flag name is included so the message is self-locating when the
caller passes multiple date flags.

## 5. Implementation

### 5.1 Validator module

New module `src/validation/datetime.rs` with sibling
`datetime_tests.rs`:

```rust
/// Validate an ISO-8601 datetime or bare date for use as a Bugzilla
/// search filter. On success, returns the canonical form sent to the
/// server (bare dates are expanded to `YYYY-MM-DDT00:00:00Z`).
///
/// `flag` is the CLI flag name (e.g. "--created-since") and is
/// included in the error message so callers can locate the offending
/// input when multiple date flags are in play.
pub fn parse_iso8601_or_date(s: &str, flag: &str) -> Result<String>;
```

The implementation uses byte-level matching against the four accepted
shapes — no regex crate, no date crate. Roughly:

1. Length-check against the four valid lengths (10, 19, 20, 25).
2. Pattern-match the digit/separator positions.
3. Range-check month (1–12), day (1–31; no per-month or leap-year
   logic — the server validates exact day validity).
4. For datetime forms, range-check hour (0–23), minute (0–59),
   second (0–60 to allow a leap second).
5. For zone-offset forms, range-check the offset hours/minutes.

A new top-level module `src/validation/mod.rs` re-exports
`parse_iso8601_or_date` (no logic in `mod.rs`, so no sibling
test file is needed there). The module is named `validation`
rather than `datetime` because it is the obvious home for any
future input-validators that aren't tied to a single command
(e.g. email shape, alias shape).

### 5.2 Type changes

**`src/types/bug.rs::SearchParams`** gains two fields:

```rust
pub creation_time: Option<String>,     // server-canonical form
pub last_change_time: Option<String>,  // server-canonical form
```

Field names match the Bugzilla REST API (and the existing `Bug`
struct's `creation_time`/`last_change_time`). The CLI flags
(`--created-since` / `--changed-since`) and struct fields are
deliberately different: flags follow the user-facing "since" idiom;
struct fields follow the API.

Both `has_filters()` and `has_structured_filters()` gain checks for
the two new fields.

**`src/types/bug.rs::SavedQuery`** gains two fields with the same
names and `#[serde(default, skip_serializing_if = "Option::is_none")]`
so existing TOML configs without these keys deserialize unchanged:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub creation_time: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub last_change_time: Option<String>,
```

`SavedQuery::into_search_params()` and `to_search_params()` forward
both fields.

`SavedQuery::has_filters()` also gains checks for both new fields.
Without this update, `bzr query save my-recent --created-since
2026-04-01` (date filter as the *only* filter) would be rejected
with the existing "query must have at least one filter set"
error, even though the date filter alone is a meaningful query.

**`SearchParams::apply_overrides`** signature grows by two
parameters:

```rust
pub fn apply_overrides(
    &mut self,
    limit: Option<u32>,
    fields: Option<&str>,
    exclude_fields: Option<&str>,
    creation_time: Option<&str>,
    last_change_time: Option<&str>,
);
```

The total reaches the project's 5-positional-param limit (the
`&mut self` does not count). At one more, the function would need
to take an `Overrides` struct; for now the direct shape is
preserved.

### 5.3 REST encoding

`src/client/bug.rs::append_option_params` gains two entries in its
`option_fields` table:

```rust
("creation_time", &params.creation_time),
("last_change_time", &params.last_change_time),
```

Result: `&creation_time=2026-04-01T00:00:00Z&last_change_time=...`
on the request URL when set.

### 5.4 XML-RPC encoding

`src/xmlrpc/client.rs::search_bugs` gains two entries in its
matching `option_fields` table. Both serialize as `Value::String`
in the RPC params map.

### 5.5 Command wiring

**`src/commands/bug/list.rs`** validates both flags before
constructing `SearchParams`. Validator failure bails with exit
code 7 before any network call.

**`src/commands/bug/history.rs`** and
**`src/commands/comment.rs`** call the validator on `--since` and
forward the canonical form to the existing `new_since` server
parameter.

**`src/commands/query.rs::handle_save`** validates both flags
before storing on `SavedQuery`. **`handle_run`** validates the
override flags and forwards them via the extended
`apply_overrides`.

### 5.6 Output module

`bzr query show` (and the JSON form) lists the two new fields when
present. `bzr query list` summary is unchanged unless the brief
view already names every set field — confirm during implementation
and add the fields to the summary if so.

### 5.7 Documentation

`docs/bzr-cli.md` gains entries for the new flags under `bug list`,
`query save`, and `query run`. The canonicalization rule
(`YYYY-MM-DD` → `T00:00:00Z`) is documented once, in the
`bug list` section, and referenced from the others.

`CHANGELOG.md` gains an entry under the current `## [0.4.0-dev]`
section:

```
### Added
- `bzr bug list` and `bzr query save`/`run` accept `--created-since`
  and `--changed-since` ISO-8601 date filters. Closes #157.

### Changed
- `bzr bug history --since` and `bzr comment list --since` now
  validate their input client-side (exit code 7 on malformed
  dates), matching the new `--created-since` / `--changed-since`
  behavior.
```

## 6. Testing

### 6.1 Unit tests (sibling `_tests.rs` files)

- `src/validation/datetime_tests.rs` — table-driven coverage of
  every accepted form, every rejected form, and the bare-date →
  `T00:00:00Z` canonicalization. Verifies the error message
  contains the flag name and the offending input.
- `src/types/bug_tests.rs` — `SearchParams::has_filters()` and
  `has_structured_filters()` return `true` when only `creation_time`
  is set. `SavedQuery::has_filters()` returns `true` when only
  `creation_time` (or only `last_change_time`) is set, so a
  date-only `bzr query save` is accepted.
  `SavedQuery::into_search_params()` forwards both new fields.
  Round-trip TOML preserves both fields with default-shape configs.
- `src/client/bug_tests.rs` — wiremock asserts the REST request
  URL contains `creation_time=...` and `last_change_time=...` when
  the corresponding `SearchParams` field is set.
- `src/xmlrpc/client_tests.rs` — RPC params map contains
  `creation_time` and `last_change_time` string entries.
- `src/commands/bug/list_tests.rs` — invalid `--created-since`
  surfaces as `BzrError::InputValidation` (exit code 7) with no
  network call (no wiremock expectation).
- `src/commands/bug/history_tests.rs` and
  `src/commands/comment_tests.rs` — same exit-code-7 check for the
  retrofitted `--since` flag.
- `src/commands/query_tests.rs` — `query save` rejects malformed
  dates; `query run` overrides replace saved values; round-trip
  TOML preserves both fields.
- `src/cli/mod_tests.rs` — clap parses the new flags with the
  expected types on every affected subcommand.

### 6.2 Integration test

`tests/integration.rs` gains one wiremock end-to-end run of
`bzr bug list --product P --changed-since 2026-04-01` asserting
the outgoing query string carries
`last_change_time=2026-04-01T00:00:00Z`.

### 6.3 Functional test

`tests/functional/run-tests.sh` (Phase 8 — Bugs) gains the test
plan from the issue verbatim:

```sh
# Setup: create two bugs, modify one, capture timestamp between
#        the two events.
# Action: bzr bug list --product <P> --changed-since <ts> --json
# Assert: only the modified bug appears (jq verifies bug id
#         present/absent).
```

Slots into the existing fixture pattern; ~25 added lines.

## 7. Migration & compatibility

- `SavedQuery` gains two `#[serde(default)]` fields. Existing
  configs deserialize unchanged. No migration step required.
- `SearchParams` is `#[non_exhaustive]`; new fields do not break
  external callers.
- `apply_overrides`'s expanded signature is a breaking API change
  for any external caller — there are none in tree.
- `bug history --since` and `comment list --since` change from
  "always accepted, server validates" to "client-validates first."
  Strings the server previously accepted but our validator now
  rejects (e.g. fractional seconds, week dates) become exit-code-7
  failures. The CHANGELOG entry calls this out.

## 8. Out of scope (revisitable)

- URL imports (`--from-url`) currently route `chfieldfrom` /
  `chfieldto` through `raw_params`. Mapping them to the new
  structured fields would lose the `chfieldto` upper-bound (we
  expose only `since`, not a range), so a partial mapping risks
  silently dropping query semantics. Revisit if a tester files a
  workflow gap on URL-imported date queries specifically.
- A future date-range filter (`--created-between A B`) is a
  different shape and isn't pre-empted by this design.
