# Uniform `--fields` / `--exclude-fields` projection across list/view verbs

**Date:** 2026-06-26
**Issue:** [#455](https://github.com/randomparity/bzr/issues/455)
**ADR:** [0010](../../adr/0010-uniform-fields-projection.md)
**Related prior art:** [`2026-05-27-json-field-trimming-design.md`](2026-05-27-json-field-trimming-design.md)
(bug-verb projection)

## Goal

Let agents request only the JSON keys they need on every list/view verb, not
just bug verbs. Today `bug list`/`view`/`search`/`my` honor `--fields` /
`--exclude-fields`; the rest of the read surface serializes the whole record.
A 200-comment thread fetched for a tag index, or an attachment list fetched for
`file_name`/`size` alone, wastes tokens and latency.

After this change these verbs accept `--fields <a,b,c>` and
`--exclude-fields <a,b,c>` with uniform semantics:

- `comment list`
- `attachment list`
- `product view`, `product list`
- `component list`, `component view`
- `user search`
- `group list-users`, `group view`
- `classification list`, `classification view`
- `field list`

This is a consistency feature, not a new design — the projection concept already
exists on bug reads. The bug machinery is **not reused verbatim**: it is built
around a hand-maintained `BugField` enum with aliases, table headers, and
column-reflow logic. The new verbs need none of that — the issue specifies
"field names match the `--json` key names already documented for each verb", so
projection is a generic top-level key filter over the serialized
`serde_json::Value`, keyed off each resource's serde field names.

## Decisions (locked)

1. **Projection target = the serde JSON keys, no aliases.** Unlike bug verbs
   (`assignee`→`assigned_to`), the new verbs accept exactly the documented
   `--json` key names. No alias table, no per-resource enum.

2. **Top-level keys only.** A non-blank `--fields` retains exactly the named
   top-level keys; `--exclude-fields` drops the named top-level keys. Nested
   structures (e.g. a product's `components` array, a user's `groups`) are kept
   or dropped whole — projection does not descend. Matches gh and the bug model.

3. **Projection applies to `--json` and `--output ndjson` only.** For an array
   payload each element object is projected; for a single-object payload the
   object is projected. `ndjson` array streaming is unchanged except each emitted
   line is the projected element.

4. **Table output is a documented no-op with a warning.** Table columns are
   fixed per verb. When `--fields`/`--exclude-fields` is given with table output,
   the verb emits one stderr warning (`warning: --fields/--exclude-fields only
   affect --json/--output ndjson; ignoring for table output`) and renders the
   normal table. Chosen over per-resource column reflow: it avoids 8 new column
   registries, keeps the human table stable, and matches the agent-centric use
   case (projection is for machine output). This resolves the issue's
   "no-op OR adjust columns — pick one" choice in favor of no-op.

5. **Unknown field name is an error (exit 7), strict and uniform.** In the JSON
   family, any token in `--fields` *or* `--exclude-fields` that is not a known
   serde key for that resource → `BzrError::InputValidation` (exit 7). A
   selection that resolves to zero keys (e.g. `--exclude-fields` covering every
   key) → exit 7. This is **stricter than bug verbs**: bug `list`/`search` warn
   on partial-unknown includes and bug `view` is exempt from the zero-field
   error. The new verbs are uniformly strict — the issue's acceptance criterion
   says "unknown field name is an error", and uniform strictness is the point of
   the consistency work. The bug verbs keep their existing (documented,
   legacy-compatible) leniency; this change does not touch them.
   - Validation runs **only in the JSON family**. In table mode the flags are a
     true no-op (per decision 4), so a misspelled field in table mode is not
     validated — it is ignored with the same warning. Documented.

6. **No new error variant, no new dependency.** Reuse `InputValidation`
   (exit 7). `serde_json` `preserve_order` is already enabled (bug work), so
   projected objects keep struct-declaration key order for free.

7. **Combined `--fields` + `--exclude-fields` resolves include first, then
   subtracts exclude** (same as bug verbs). The effective key set is: the
   include set when a non-blank `--fields` is given, else all known keys; then
   every `--exclude-fields` token is removed. An exclude token that is a valid
   known key but absent from the include set is **inert, not an error**
   (e.g. `--fields a,b --exclude-fields c` → `{a, b}`). An exclude token that is
   not a known key is still an error (decision 5). If the final set is empty
   (e.g. `--fields a --exclude-fields a`, or excluding every key) → exit 7. This
   is validated by an explicit combined-flag unit + wiremock case.

8. **Selecting a key that a given verb's records never carry is allowed and
   yields sparse objects, not an error.** Some serde keys are absent from a
   verb's actual payload (e.g. `Attachment.data` is only populated by
   `attachment download`, never by `attachment list`). `--fields data` on
   `attachment list` validates (it is a known `Attachment` key) and projects to
   `{}` per element. This is gh-consistent — asking for an absent key returns
   nothing — and is documented in the verb's `docs/bzr-cli.md` note rather than
   special-cased out of the key universe.

## Non-goals

- No change to bug verbs (`bug list`/`view`/`search`/`my`, `query run`). Their
  alias-aware projection and bug-view leniency stay as-is.
- No change to bare output (no field selection) — full object/array, unchanged.
- No nested/dotted field paths (`components.name`). Top-level only.
- No new verbs; `commands.yml` is unchanged.
- No projection on mutation/result output (create/update/tag/etc.) — only the
  read verbs listed above.

## Architecture

### Shared validator + projector (`src/validation/fields.rs`)

The single place field-list parsing lives, per the issue's "shared validation
helper under `validation/`" criterion and the repo convention that cross-command
value-shape validators live under `validation/`.

```rust
/// A validated field selection ready to apply to serialized JSON.
/// `None`-equivalent (no include, no exclude) is the identity projection.
pub struct FieldProjection { /* include: Option<Vec<String>>, exclude: Vec<String> */ }

impl FieldProjection {
    /// Identity projection (passes every key through).
    pub fn none() -> Self;

    /// Parse + validate raw `--fields` / `--exclude-fields` against the
    /// resource's `known` serde keys. Tokens are trimmed; blanks skipped;
    /// duplicates collapsed. Returns `InputValidation` (exit 7) when:
    ///   - any include or exclude token is not in `known`, or
    ///   - the resulting key set is empty.
    /// A blank/all-empty `--fields` is treated as "no include" (full set).
    pub fn resolve(include: Option<&str>, exclude: Option<&str>, known: &[&str])
        -> crate::error::Result<Self>;

    /// Whether either flag was given (drives the table no-op warning).
    pub fn is_requested(&self) -> bool;

    /// Project a serialized value in place: an object keeps/drops top-level
    /// keys; an array projects each element object; other values are untouched.
    pub fn apply(&self, value: &mut serde_json::Value);
}
```

The error message names the resource and the offending token, e.g.
`unknown field 'fil_name' for comment; known fields: id, bug_id, text, ...`.

### Per-resource known-key registries

Each resource type declares its serde keys next to the type:

| Resource type | Module | Keys |
|---|---|---|
| `Comment` | `types/comment.rs` | id, bug_id, text, creator, creation_time, count, is_private, attachment_id |
| `Attachment` | `types/attachment.rs` | id, bug_id, file_name, summary, content_type, creator, creation_time, last_change_time, size, is_obsolete, is_private, is_patch, flags, data |
| `Product` | `types/product.rs` | id, name, description, is_active, components, versions, milestones |
| `Component` | `types/component.rs` | id, name, description, is_active, default_assignee |
| `BugzillaUser` | `types/user.rs` | id, name, real_name, email, groups, can_login |
| `GroupInfo` | `types/group.rs` | id, name, description, is_active, membership |
| `Classification` | `types/classification.rs` | id, name, description, sort_key, products |
| `FieldValue` | `types/field.rs` | name, sort_key, is_active, can_change_to |

Declared as `pub const <TYPE>_FIELDS: &[&str]` in the type's module. A drift-guard
unit test (below) keeps each list in lockstep with the struct's serde output.

`user search` and `group list-users` both serialize `BugzillaUser` (regardless of
the `--details` flag, which only changes the table writer), so both use
`BUGZILLA_USER_FIELDS`.

### Shared clap args (`src/cli/fields.rs`)

```rust
#[derive(clap::Args, Debug, Clone, Default)]
pub(crate) struct ProjectionArgs {
    /// Comma-separated JSON keys to keep (only affects --json/--output ndjson).
    #[arg(long)]
    pub fields: Option<String>,
    /// Comma-separated JSON keys to drop (only affects --json/--output ndjson).
    #[arg(long)]
    pub exclude_fields: Option<String>,
}
```

`#[command(flatten)] projection: ProjectionArgs` is added to each target
`*Action` struct-variant / args struct. clap renders `--fields` /
`--exclude-fields` (underscore→hyphen) — same spelling as bug verbs. This is a
new, separate struct from bug's `FieldArgs` (which carries bug-specific
table-column help); the two do not share code because their help text and
semantics differ.

### Output seam (`src/output/formatting.rs`)

A projection-aware sibling of the existing `write_formatted` /
`write_table_or_empty`, so JSON stays a single path per writer (no dead arms):

```rust
pub(crate) fn write_formatted_projected<T, W>(
    value: &T, format: OutputFormat, projection: &FieldProjection, out: &mut W,
    table_fn: impl FnOnce(&T, &mut W),
) // Json/Ndjson: to_value -> projection.apply -> write_json / write_ndjson
  // Table:       table_fn(value, out)   (projection ignored)

pub(crate) fn write_table_or_empty_projected<T, W>(
    items: &[T], format: OutputFormat, projection: &FieldProjection,
    out: &mut W, table: TableSpec<'_>, to_record: impl Fn(&T) -> Vec<String>,
)
```

`write_json` keeps wrapping the JSON-family pretty output in the
`schema_version` envelope; `write_ndjson` stays bare. Projection happens on the
`Value` before either, so the envelope wraps the trimmed payload.

### Command-layer wiring (per verb)

```rust
let projection = if ctx.format().is_json_family() {
    FieldProjection::resolve(p.fields.as_deref(), p.exclude_fields.as_deref(),
                             COMMENT_FIELDS)?            // exit 7 on unknown/empty
} else {
    if p.fields.is_some() || p.exclude_fields.is_some() {
        let _ = writeln!(w.err,
            "warning: --fields/--exclude-fields only affect --json/--output \
             ndjson; ignoring for table output");
    }
    FieldProjection::none()
};
// ... fetch ...
write_comments(&comments, ctx.format(), &projection, w.out);
```

Each affected writer gains a `projection: &FieldProjection` parameter and swaps
`write_formatted` → `write_formatted_projected` (or the table-or-empty variant).
The table closures are unchanged. Validation is pre-fetch where the handler
parses args before connecting, so a typo fails before any network call.

## Data flow

```
CLI args ─> ProjectionArgs ─> command handler
                                · json family: FieldProjection::resolve
                                    (exit 7 on unknown token / empty result)
                                · table: warn no-op, FieldProjection::none()
                                      │
                                  network fetch
                                      │
                            write_*(value, format, &projection)
                                · json/ndjson: to_value -> apply -> envelope/bare
                                · table: fixed columns, projection ignored
                                      │
                                   stdout
```

## Testing (TDD)

**Unit — `src/validation/fields_tests.rs`:**
- include keeps exactly named keys; exclude drops named keys; neither = identity.
- unknown include token → `Err` (exit 7); unknown exclude token → `Err`.
- exclude covering every known key → `Err`; include of all-blank (`""`,`,,`) =
  identity (full set).
- combined flags: `--fields a,b --exclude-fields a` → `{b}`; `--fields a,b
  --exclude-fields c` (c known, not in include) → `{a, b}` (inert exclude);
  `--fields a --exclude-fields a` → `Err` (exit 7) (decision 7).
- `apply` over an array projects every element; over a single object projects it;
  over a scalar is a no-op; selecting a key absent from the value yields an
  object without it (sparse, not an error — decision 8).
- projected object key order = serialization order (preserve_order lock).

**Unit — per type drift guard (`types/<t>_tests.rs`):** serialize a
**fully-serializing** instance and assert its serde key set is exactly equal to
`<TYPE>_FIELDS` (set equality, both directions), and that the list has no
duplicates. "Fully-serializing" is load-bearing: several target types use
`skip_serializing_if = "Option::is_none"` / `"Vec::is_empty"` (`Attachment`,
`Product`, `Component`, `GroupInfo`, `FieldValue`), so the fixture MUST set every
`Option` field to `Some` and every skip-on-empty `Vec` to non-empty — otherwise a
skipped key could be silently missing from both the serialization and the const,
the equality would pass, and `--fields <that_key>` would wrongly exit 7 for a
real field. The set-equality-both-directions assertion is what catches an
incomplete const, but only if the fixture forces the full key universe to
serialize. Each type's test states the populated fixture explicitly.

**Command/wiremock — one per verb (`<cmd>_tests.rs`):**
- `--json --fields <k>` returns objects with only `<k>` (envelope `data` trimmed).
- `--output ndjson --fields <k>` emits trimmed lines.
- `--json --fields <typo>` → exit 7, nothing on stdout.
- table + `--fields <k>` → normal table on stdout + warning on stderr, exit 0.

**Functional (`tests/functional/phases/`):** extend the existing per-resource
phase scripts (`03-products`, `04-components`, `05-fields-classifications`,
`06-users`, `07-groups`, `15-comments`, `16-attachments`) to assert
`--json --fields <k> | jq` returns only the projected key against a real
container, and that an unknown field exits 7. Cover the credentialless path where
the verb supports it.

**Guardrails:** `make lint` (fmt + clippy + check-test-layout + check-no-spawn),
full `cargo test`, `cargo deny check`, flag-drift-check (docs tree updated),
`make skills-test` (json-recipes recipe added). The `ProjectionArgs` help text
and the table no-op warning must not reintroduce any phrase forbidden by
`cli_and_docs_avoid_misleading_trim_phrasing` (tests/integration.rs) — the
proposed strings ("Comma-separated JSON keys to keep/drop", the no-op warning)
already comply; that test scans `docs/bzr-cli.md` and `src/cli/*.rs` (top level,
so `cli/fields.rs` is covered).

## Docs & artifacts

- `docs/bzr-cli.md`: add `--fields`/`--exclude-fields` to each verb's section
  with the no-op-on-table note and the exit-7 contract.
- `CHANGELOG.md` `[Unreleased] > Added`: one bullet, `(#455)`.
- `agent-skills/skills/bzr-reference/reference/json-recipes.md`: a
  "project to cut tokens" recipe.
- `commands.yml`: unchanged (no new verbs).

## Blast radius

- Additive flags on read verbs. Bare invocations are byte-for-byte unchanged.
- New `--json` + selection invocations return trimmed objects (the intended
  effect). `jq` on an absent key returns `null`, so well-behaved consumers are
  unaffected.
- No mutation, auth, config, or wire-request behavior changes — projection is
  client-side after the fetch. (For attachment/comment the full record is still
  fetched; only the emitted JSON is trimmed.)
```

