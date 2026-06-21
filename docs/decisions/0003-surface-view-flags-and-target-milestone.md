# 0003 — Surface `flags` and `target_milestone` in `bug view` / `attachment view`

- Status: Accepted
- Date: 2026-06-20
- Issue: [#351](https://github.com/randomparity/bzr/issues/351)

## Context

The Bugzilla REST API returns a `flags` array on bugs and attachments, and a
`target_milestone` string on bugs, but bzr's view types never deserialized them:

- The view `Bug` struct (`src/types/bug.rs`) lists explicit built-in fields and
  collects everything else via `#[serde(flatten)] extra`, but `From<BugWire>`
  keeps only `cf_*` keys (`is_custom_field_name`) — so `flags` and
  `target_milestone` land in `extra` and are then dropped.
- The view `Attachment` struct (`src/types/attachment.rs`) has no `flags` field;
  the `flags: Vec<FlagUpdate>` that exists there is on the *upload/update request*
  payloads, not the view response.

So a `--flag` / `--target-milestone` value written via `bug create` / `update`
is persisted server-side but invisible through `bug view` / `attachment view`,
and `bug view <id> --fields flags` returns `{}`. This also blocks functional
coverage of `--flag` / `--target-milestone` round-trips (see #350).

`FlagUpdate` (`src/types/common.rs`) is a *write* type: `{ name, status:
FlagStatus, requestee }`, where `FlagStatus` is a strict enum that only accepts
`+ - ? X` and **errors** on anything else. It is not suitable for the read path.

## Decision

1. **Add a read-side `Flag` type** in `src/types/common.rs`, distinct from the
   write-side `FlagUpdate`:

   ```rust
   pub struct Flag {
       pub name: String,
       pub status: String,           // raw server token: "+", "-", "?"
       pub setter: Option<String>,
       pub requestee: Option<String>,
   }
   ```

   - `status` is a **plain `String`**, not the `FlagStatus` enum. The view path
     must be tolerant: a status value the enum does not model must not make
     `bug view` fail to deserialize. The raw token is also exactly what we
     display.
   - Fields beyond these (`id`, `type_id`, `creation_date`,
     `modification_date`) are not surfaced — they are not useful in the view and
     would be speculative. Every field uses `#[serde(default)]` so a server that
     omits one still deserializes. Derives `Serialize, Deserialize`.

2. **Add `flags: Vec<Flag>` and `target_milestone: Option<String>` to the view
   `Bug`** (struct, `BugWire`, `From`, and the manual `Serialize` impl). Bump
   `BUG_BUILT_IN_FIELD_COUNT` 23 → 25. `flags` serializes as an array (empty →
   `[]`, matching `keywords`/`cc`); `target_milestone` as a nullable string.

3. **Add `flags: Vec<Flag>` to the view `Attachment`** (`#[serde(default)]`,
   serialized always, empty → `[]`).

4. **Register `flags` and `target_milestone` in the bug column registry**
   (`COLUMNS` in `src/output/resources/bug.rs`) with canonical keys `flags` and
   `target_milestone` that match the serialized JSON keys. This makes
   `--fields flags` / `--fields target_milestone` work for both table columns
   and JSON field-selection (the registry drives `bug_to_json` key retention and
   the server `include_fields` payload).

5. **Add `flags` and `target_milestone` to `BUG_DEFAULT_FIELDS`**
   (`src/client/bug.rs`). This is load-bearing: a default `bug view` (no
   `--fields`) sends `include_fields = BUG_DEFAULT_FIELDS`, so without these two
   names the server never returns the data and the new struct fields stay empty.
   When `--fields flags` is given, the canonical token `flags` is sent verbatim
   as `include_fields` and Bugzilla's `Bug.get` returns the array; the same for
   `target_milestone`. (XML-RPC ignores field lists and returns the full record,
   so both fields surface there once the struct deserializes them.)

6. **Render in detail/table output.** A flag renders as its `name` followed by
   the status token, with the requestee in parentheses when present —
   symmetric with the `name?(user@example.com)` input syntax:
   - `review+`, `review-`, `review?`, `review?(bob@example.com)`
   The bug detail view gets `Target Milestone` and `Flags` rows; the attachment
   detail view gets a `Flags` row.

   **Unset sentinels are suppressed in the table detail, not in JSON.** Bugzilla
   returns `target_milestone` as the literal `---` (its default "no milestone"
   value), not null, so a naive row would print `Target Milestone: ---` on
   nearly every bug. The detail view therefore omits the Target Milestone row
   when the value is absent, empty, or `---`; likewise the Flags row is omitted
   when there are no flags. JSON stays faithful — it carries the raw
   `target_milestone` string (including `---`) and the full `Flag` objects
   (including `setter`) — so machine consumers see exactly what the server
   returned.

## Consequences

- `bug view` / `attachment view` now show flags and (for bugs) the target
  milestone, in table and JSON. `bug view <id> --fields flags` returns the
  flags. The deferred functional coverage for `--flag` /
  `--target-milestone` (#350) is unblocked.
- Two new selectable bug fields (`flags`, `target_milestone`) appear in
  `--fields` / table columns. `docs/bzr-cli.md` and `CHANGELOG.md` are updated.
- Additive only: no existing field changes shape. New fields default to empty /
  null, so servers that omit them (or transports that don't return them) keep
  working.

## Considered & rejected

- **Reuse `FlagUpdate` / `FlagStatus` for the view.** Rejected: `FlagStatus`
  deserialization errors on any token outside `+ - ? X`, so an unexpected server
  value would break the whole `bug view`. The read path must be lenient, and a
  write-shaped type (with `requestee` but no `setter`) is the wrong shape for
  display.
- **Drop `flags` into `custom_fields` / `extra` generically.** Rejected: those
  are typed for `cf_*` custom fields and render as opaque JSON blobs; `flags`
  deserves a real typed field and a readable rendering.
- **Surface every flag sub-field (`id`, `type_id`, dates).** Rejected as
  speculative; `name`/`status`/`setter`/`requestee` are what a user reads.
