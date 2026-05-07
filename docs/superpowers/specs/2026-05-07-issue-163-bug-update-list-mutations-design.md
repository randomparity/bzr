# Issue #163 — `bug update`: list-mutation flags for keywords / cc / groups / see-also

**Date:** 2026-05-07
**Branch:** `feat/issue-163-bug-update-list-mutations`
**Tracking:** [#163](https://github.com/randomparity/bzr/issues/163)
**Spec source:** [bzl-parity review, Issue H](2026-05-06-bzl-parity-review-design.md)

## 1. Problem

`bzl-update` supports `+` / `-` / bare-set syntax for `keywords`, `cc`,
`groups`, and `see_also`
(`reference/bzl/bzl-update:74-78,279-290`). `bzr bug update` only supports
add / remove for two list-typed fields: `blocks` and `depends_on`. To
mutate the other four list fields a tester has to fall back to the
Bugzilla web UI.

## 2. Goal

Add four `*-add` / `*-remove` flag pairs to `bzr bug update`, mirroring
the existing `--blocks-add` / `--blocks-remove` shape:

- `--keywords-add` / `--keywords-remove`
- `--cc-add` / `--cc-remove`
- `--groups-add` / `--groups-remove`
- `--see-also-add` / `--see-also-remove`

Closes [#163](https://github.com/randomparity/bzr/issues/163).

## 3. Non-goals

- **Bare-set / replace syntax** (`--keywords foo,bar` to overwrite).
  bzl supports it; this issue scopes to incremental edits only.
- **Field-specific validation** (email regex for CC, URL parsing for
  see-also). Light trim-and-reject-empty validation only.
- **Pre-flight permission checks for groups.** The existing `bug update`
  pattern trusts the server to enforce permissions and surfaces the
  resulting error. Groups behave identically.
- **The Issue I scalar flags** (`--alias`, `--deadline`,
  `--estimated-time`, etc.) are tracked separately.

## 4. Design

### 4.1 Type changes (`src/types/bug.rs`)

Add a sibling type alongside `IdListUpdate`:

```rust
/// Incremental update to a string-typed list field
/// (keywords, cc, groups, see_also). Bugzilla accepts
/// `{ "add": [...], "remove": [...] }` for these fields.
#[derive(Debug, Default, Serialize)]
#[non_exhaustive]
pub struct StringListUpdate {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

impl StringListUpdate {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}
```

Extend `UpdateBugParams` with four new fields, each guarded by
`skip_serializing_if`:

```rust
#[serde(skip_serializing_if = "StringListUpdate::is_empty")]
pub keywords: StringListUpdate,
#[serde(skip_serializing_if = "StringListUpdate::is_empty")]
pub cc: StringListUpdate,
#[serde(skip_serializing_if = "StringListUpdate::is_empty")]
pub groups: StringListUpdate,
#[serde(skip_serializing_if = "StringListUpdate::is_empty")]
pub see_also: StringListUpdate,
```

Re-export `StringListUpdate` from `src/types/mod.rs` next to
`IdListUpdate`.

**Why a sibling type, not a generic `ListUpdate<T>`:** the codebase
prefers explicit types over generics; the duplication is small and the
two concrete shapes are short.

### 4.2 CLI surface (`src/cli/bug.rs`)

Eight new `#[arg(long)]` fields on `BugAction::Update`. Three pairs use
`value_delimiter = ','` matching the existing convention; `see_also`
does not (URLs may legitimately contain commas):

```rust
/// Add keywords (comma-separated). Combine with
/// --keywords-remove for incremental edits.
#[arg(long, value_delimiter = ',')]
keywords_add: Vec<String>,
/// Remove keywords (comma-separated).
#[arg(long, value_delimiter = ',')]
keywords_remove: Vec<String>,

/// Add CC entries (comma-separated). Accepts usernames or
/// email addresses; format is server-defined.
#[arg(long, value_delimiter = ',')]
cc_add: Vec<String>,
/// Remove CC entries (comma-separated).
#[arg(long, value_delimiter = ',')]
cc_remove: Vec<String>,

/// Add groups (comma-separated). Group operations require
/// permission; failures surface from the server.
#[arg(long, value_delimiter = ',')]
groups_add: Vec<String>,
/// Remove groups (comma-separated).
#[arg(long, value_delimiter = ',')]
groups_remove: Vec<String>,

/// Add a see-also URL. Repeat the flag to add multiple
/// (URLs may contain commas, so no comma-list parsing).
#[arg(long)]
see_also_add: Vec<String>,
/// Remove a see-also URL. Repeat the flag to remove multiple.
#[arg(long)]
see_also_remove: Vec<String>,
```

Update the `Update` variant doc comment (`src/cli/bug.rs:524-555`) to
mention the four new list-mutation fields and add an example showing
combined use:

```text
bzr bug update 100 --keywords-add fix-needed,regression \
  --cc-add alice@example.com --see-also-add https://...
```

### 4.3 Command plumbing (`src/commands/bug/update.rs`)

Extend `build_update_params` to destructure the eight new args. A
single small helper performs trim-and-reject-empty validation per field:

```rust
fn clean_string_list(values: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(BzrError::InputValidation(
                "list value cannot be empty or whitespace-only".to_string(),
            ));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}
```

The validator runs after clap's comma-splitting, so
`--keywords-add ",,foo"` → `["", "", "foo"]` → first empty value
rejected. URLs in `--see-also-add` aren't comma-split, so trimming a
single entry is the only concern there.

The `UpdateBugParams` build site grows linearly; no further extraction
needed:

```rust
let params = UpdateBugParams {
    // ... existing fields ...
    blocks: IdListUpdate { add: blocks_add.clone(), remove: blocks_remove.clone() },
    depends_on: IdListUpdate { add: depends_on_add.clone(), remove: depends_on_remove.clone() },
    keywords: StringListUpdate {
        add: clean_string_list(keywords_add)?,
        remove: clean_string_list(keywords_remove)?,
    },
    cc: StringListUpdate {
        add: clean_string_list(cc_add)?,
        remove: clean_string_list(cc_remove)?,
    },
    groups: StringListUpdate {
        add: clean_string_list(groups_add)?,
        remove: clean_string_list(groups_remove)?,
    },
    see_also: StringListUpdate {
        add: clean_string_list(see_also_add)?,
        remove: clean_string_list(see_also_remove)?,
    },
};
```

### 4.4 Client layer

`src/client/bug.rs::update_bug` requires no changes. It serializes
`UpdateBugParams` whole, and the `skip_serializing_if` guards keep the
request body minimal when fields are unset. Empty-input case (no new
flags supplied) sends exactly the same JSON it does today.

### 4.5 Error handling

Empty / whitespace-only values raise `BzrError::InputValidation`
(exit code 7) before any network call. Server-side rejections (unknown
keyword, no permission to set group, malformed see-also URL) propagate
as the existing `BzrError::HttpStatus` / `BzrError::Api` would for any
other update field — no special-casing.

## 5. Testing

### 5.1 Unit tests

`src/types/bug_tests.rs`:

- `string_list_update_serializes_with_add_and_remove`
- `string_list_update_skips_empty_add`
- `string_list_update_skips_empty_remove`
- `update_bug_params_omits_empty_string_lists`
- `update_bug_params_serializes_string_lists` — populates all four
  fields, asserts JSON shape
  `{"keywords":{"add":[…]},"cc":{"remove":[…]},…}`.

`src/commands/bug/update_tests.rs`:

- `build_update_params_populates_string_lists`
- `build_update_params_rejects_empty_keyword`
- `build_update_params_rejects_whitespace_only_cc`
- `build_update_params_trims_see_also_url`

`src/cli/mod_tests.rs`:

- `bug_update_parses_keywords_add_comma_list`
- `bug_update_parses_see_also_add_repeated_flag` — confirms repeated
  `--see-also-add` accumulates without comma-splitting.

### 5.2 Integration tests

One wiremock test in `tests/integration.rs` that posts an update with
all four list mutations and asserts the JSON request body shape matches
`{"keywords":{"add":[…],"remove":[…]},…}`.

### 5.3 Functional tests

One scenario in `tests/functional/run-tests.sh`: create a bug, run
`bzr bug update <id> --keywords-add fix-needed --cc-add functional@example.com`,
then `bzr bug view <id> --json` and assert both fields appear in the
output. Skip groups (server-permission dependent) and see-also (URL
whitelist may be enforced) — focus on the unprivileged happy path.

## 6. Documentation

- `docs/bzr-cli.md` — update the `bug update` section to document the
  eight new flags. Mirror the `--blocks-add` / `--depends-on-add`
  style; call out that `--see-also-add` does not split on commas
  (repeat the flag for multiple URLs).
- `CHANGELOG.md` — add an entry under the `## [Unreleased]`
  section: *"`bug update`: list-mutation flags for `keywords`, `cc`,
  `groups`, `see_also` (closes #163)."*

## 7. Rollout

Single PR on `feat/issue-163-bug-update-list-mutations`. Spec lands in
the same PR as the implementation.

## 8. Open questions

None at design time. Decisions captured during brainstorming:

| Question | Decision |
|---|---|
| Generic vs. sibling type | Sibling `StringListUpdate` |
| `value_delimiter` for see-also | Drop it for `--see-also-{add,remove}` only |
| Client-side validation | Light: trim and reject empty/whitespace-only |
| Group-permission pre-check | None — let server enforce |
