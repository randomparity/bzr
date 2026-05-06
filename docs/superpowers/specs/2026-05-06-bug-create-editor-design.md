# `bzr bug create`: `$EDITOR` flow + `--description-file`

**Date:** 2026-05-06
**Issues:** #159 (`$EDITOR` flow), #160 (`--description-file`)
**Source review:** `docs/superpowers/specs/2026-05-06-bzl-parity-review-design.md` (Issues D, E)
**Branch:** `docs/bug-create-editor-spec` (spec only) → implementation branch TBD

## 1. Summary

`bzr bug create` today requires `--summary` and a description from
`--description`. `bzl-create` (the bzl reference)
opens `$EDITOR` with a templated buffer when no description is given,
and accepts `--description-file FILE`. Both gaps are tester-ergonomic
regressions surfaced by the bzl → bzr workflow-parity review.

This spec covers a single PR that adds:

- `--description-file <PATH>` (closes #160)
- `$EDITOR` fallback for the description (closes #159)
- Stdin support for the description (mirrors `bzr comment add`)
- A precedence chain across all four sources

It also extracts the existing `comment add` editor helpers
(`TempFile`, `launch_editor`) into a shared
`src/commands/editor.rs` so both callers reuse the same primitives.

## 2. Goals / non-goals

**Goals.**

- `bzr bug create` works with `--summary X --description Y` exactly as
  it does today.
- `bzr bug create` with no `--description` and a TTY opens `$EDITOR`
  with a pre-filled summary (if supplied), a template-supplied
  description body (if any), and a `git commit -v`-style sentinel
  divider with informational field reminders below.
- `bzr bug create --description-file PATH` reads the description from
  a UTF-8 file. Mutually exclusive with `--description`.
- Piped stdin (`echo "body" | bzr bug create ...`) is read as the
  description, mirroring `bzr comment add`.
- Precedence is deterministic and validated end-to-end by a functional
  test.

**Non-goals.**

- Adding a new "editor template" config mechanism. The existing
  `--template` flag (saved templates from `bzr template`) already
  carries a `description` field; the editor pre-fills the buffer with
  it when set. Org-specific buffer skeletons live in saved templates,
  not in a new config surface.
- Changing the `bzr comment add` editor flow's user-visible behavior.
  The shared-helpers refactor is internal; HTML-comment stripping
  stays in `comment.rs`.
- New error variants in `BzrError`. `InputValidation` (exit 7) covers
  every new failure mode.

## 3. Architecture

Single PR, four phases as cohesive commits on one branch.

### 3.1 Phase 1 — Refactor: extract `src/commands/editor.rs`

New module containing the generic editor-launch primitives:

```rust
// src/commands/editor.rs (sketch)
pub(super) struct TempFile { /* path + Drop impl */ }

pub(super) fn launch(initial: &str, prefix: &str) -> Result<String> {
    // 1. Write `initial` to a fresh tempfile named with `prefix`.
    // 2. Spawn $EDITOR (or `vi` fallback) on the path.
    // 3. On non-zero exit: InputValidation("$EDITOR exited with error").
    // 4. Read the file back as UTF-8 and return its contents.
    // 5. Tempfile is removed via TempFile::drop.
}
```

`commands/comment.rs` is updated to call `editor::launch(template,
"bzr-comment")` and apply `filter_comment_body` to the result. The
HTML-comment-stripping rule is comment-add-specific and stays there.

### 3.2 Phase 2 — `--description-file <PATH>` flag (closes #160)

- Clap surface (`src/cli/bug.rs::BugAction::Create`):
  - Add `description_file: Option<PathBuf>` with
    `#[arg(long, conflicts_with = "description")]`.
- Handler (`src/commands/bug/create.rs`):
  - When set, read file as UTF-8.
  - Missing path → `InputValidation` (exit 7).
  - Non-UTF-8 contents → `InputValidation` (exit 7).
  - Otherwise treat the contents as the resolved description; no
    editor involvement.

### 3.3 Phase 3 — Editor flow + optional summary (closes #159)

- Clap surface:
  - `summary: String` → `summary: Option<String>`. Doc-comment
    updated to "required unless the editor flow is active."
- Handler precedence chain (highest wins):

  1. `--description "text"` (literal)
  2. `--description-file PATH` (file contents)
  3. piped stdin (when `stdin.is_terminal() == false`)
  4. `$EDITOR` (when stdin is a TTY and none of the above)

  `--description` and `--description-file` are clap-mutually-exclusive
  (exit 2 from clap if both supplied).

- When the editor flow is active, the buffer is built as:

  ```
  <pre-filled summary or empty line>

  <template description body or empty>

  # ------------------------ >8 ------------------------
  # Do not modify or remove the line above.
  # Everything below it will be ignored.
  #
  # Product:    <resolved>
  # Component:  <resolved>
  # Version:    <resolved>
  # Priority:   <resolved or unset>
  # Severity:   <resolved or unset>
  # Assignee:   <resolved or unset>
  # OpSys:      <resolved or unset>
  # Platform:   <resolved or unset>
  ```

- Parsing rules after the editor exits:
  - Truncate at the sentinel line (`# ------------------------ >8 ------------------------`); discard the line and everything after.
  - **Summary**: the first non-empty line of the truncated buffer
    (after stripping any leading blank lines).
  - **Description**: everything after that line, with leading blank
    lines stripped and trailing whitespace trimmed. Internal blank
    lines are preserved verbatim. May be empty (an empty description
    is permitted by the API).
  - Empty truncated buffer or no non-empty line found →
    `InputValidation` (exit 7), message `"empty buffer, aborting"`.
  - Editor non-zero exit → `InputValidation` (exit 7),
    message `"$EDITOR exited with error"`.
  - User-supplied `--summary` is the pre-fill only. If the user
    edits/clears it in the buffer, the parsed value wins (per #159
    spec).

- Validation order in handler:
  1. Resolve `(summary_cli, description_source)` from precedence chain.
  2. If editor flow chosen, run editor + parse → `(summary_parsed,
     description)`. Otherwise `(summary_cli, description)`.
  3. If summary still `None`/empty after both paths →
     `InputValidation`.
  4. Build `CreateBugParams` and call API as today.

### 3.4 Phase 4 — Docs + changelog

- `docs/bzr-cli.md` — `bug create` section documents new flags,
  precedence chain, editor flow, and exit codes.
- `CHANGELOG.md` — two bullets under the unreleased section, one per
  issue.
- Clap `doc_comment`s updated with examples and exit-code references
  to match existing per-resource style.

## 4. Components

| File                                 | Change                                                                                      |
|--------------------------------------|---------------------------------------------------------------------------------------------|
| `src/commands/editor.rs`             | New. `TempFile` + `launch(initial, prefix) -> Result<String>`. No domain knowledge.          |
| `src/commands/editor_tests.rs`       | New. Sibling tests: tempfile cleanup, `launch` happy/error paths, fake `$EDITOR`.            |
| `src/commands/mod.rs`                | Add `pub(super) mod editor;`.                                                                |
| `src/commands/comment.rs`            | Remove `TempFile`, `create_comment_tempfile`, `compose_comment_in_editor`; use `editor::launch`. `filter_comment_body` stays. |
| `src/cli/bug.rs`                     | `Create.summary: String` → `Option<String>`; add `description_file: Option<PathBuf>` with `conflicts_with = "description"`; doc-comment overhaul. |
| `src/commands/bug/create.rs`         | Resolution chain, editor template builder, summary/description parser, validation. ~150 lines added. |
| `src/commands/bug/create_tests.rs`   | Sibling tests: buffer builder, parser edge cases, resolution dispatch (with mocked client + fake editor). |
| `tests/functional/run-tests.sh`      | New scenario: precedence ordering validated end-to-end against a real Bugzilla container.   |
| `docs/bzr-cli.md`                    | Updated `bug create` reference.                                                              |
| `CHANGELOG.md`                       | Two bullets under unreleased.                                                                |

No new `BzrError` variants. No new dependencies.

## 5. Data flow (Phase 3)

```
                    ┌──────────────────────────┐
                    │ bug create handler       │
                    └────────────┬─────────────┘
                                 │
            resolve description source by precedence:
                                 │
   ┌────────────┬────────────────┼─────────────────┬──────────────┐
   ▼            ▼                ▼                 ▼              ▼
--description  --description-   piped stdin     $EDITOR        none + non-TTY
  literal      file (read)      (read_to_string) (template+launch)  → error
                                                       │
                                       ┌───────────────┴──────────────┐
                                       │ parse: truncate at sentinel, │
                                       │ first non-empty line=summary │
                                       │ rest=description             │
                                       └───────────────┬──────────────┘
                                                       │
                       ┌───────────────────────────────┴──────┐
                       │ summary resolved? (CLI flag OR parsed)│
                       └─────────────┬────────────────────────┘
                                     ▼
                          build CreateBugParams → API → output
```

If the editor flow is not active, `--summary` is required (current
behavior). If it is active, summary may come from CLI pre-fill or the
parsed first block.

## 6. Error handling

| Condition                                                      | Variant            | Exit |
|----------------------------------------------------------------|--------------------|------|
| `--description` + `--description-file` both set                | clap-built-in      | 2    |
| `--description-file` path missing                              | `InputValidation`  | 7    |
| `--description-file` non-UTF-8                                 | `InputValidation`  | 7    |
| `$EDITOR` exits non-zero                                       | `InputValidation`  | 7    |
| Editor produces empty buffer (no non-empty first-block line)   | `InputValidation`  | 7    |
| `--summary` absent and editor flow not active                  | `InputValidation`  | 7    |
| Bugzilla API rejects                                           | `Api` (existing)   | 4    |

## 7. Testing

### 7.1 Unit (Phase 1, sibling tests)

- `editor::TempFile` removes its file when dropped; missing file at
  drop time is non-fatal (debug-logged).
- `editor::launch` writes the initial content, invokes a fake `$EDITOR`
  script that overwrites the file, and returns the new contents.
- `editor::launch` propagates a non-zero `$EDITOR` exit as
  `InputValidation`.

### 7.2 Unit (Phase 3, sibling tests in `bug/create_tests.rs`)

- Buffer builder:
  - Pre-fills the supplied `--summary` on the first line.
  - Includes the template-description body (when `--template` carries
    one) above the sentinel.
  - Renders resolved fields below the sentinel; uses `<unset>` for
    `None` values for visibility.
- Parser:
  - Single-line summary + paragraph description → both preserved.
  - Multi-line summary text (no blank line before description) →
    first non-empty line is summary; remaining lines become the
    leading lines of the description.
  - Buffer with leading blank lines → blank lines skipped before
    summary extraction.
  - Sentinel correctly truncates trailing informational block.
  - Buffer with only the sentinel and informational lines below →
    `InputValidation`.
  - Buffer with content but no non-empty line above the sentinel →
    `InputValidation`.
- Resolution dispatch (mocked client + temp fake editor):
  - `--description "X"` wins; editor never invoked.
  - `--description-file PATH` wins over stdin and editor.
  - Piped stdin (non-TTY) wins over editor.
  - Editor flow only when none of the above set AND stdin is TTY.

### 7.3 Functional (`tests/functional/run-tests.sh`)

A new precedence-ordering scenario uses a deterministic fake-editor
script (per #159's test plan) and asserts the resulting bug's
description via `bzr bug view --json`:

```sh
cat > "$TMPDIR/fake-editor.sh" <<'SH'
#!/bin/sh
printf 'Editor summary\n\nEditor description\n' > "$1"
SH
chmod +x "$TMPDIR/fake-editor.sh"
```

Sub-cases:

1. `--description "flag"` only → assert description is `"flag"`.
2. `--description-file /tmp/f` (containing `"file"`) + piped stdin
   `"stdin"` → assert description is `"file"`.
3. Piped stdin `"stdin"` + `EDITOR=fake-editor.sh` → assert
   description is `"stdin"`.
4. No source, TTY, `EDITOR=fake-editor.sh`, no `--summary` → assert
   summary `"Editor summary"`, description `"Editor description"`.
5. Negative: empty fake editor (writes empty file) → exit 7.
6. Negative: `--description-file /nonexistent` → exit 7.
7. Negative: `--description "X"` with no `--summary` and no editor
   flow active → exit 7.

The existing functional-test framework handles per-test bug cleanup;
new bugs are created against a real Bugzilla container.

### 7.4 Integration

`tests/integration.rs` is unchanged unless an existing test asserts
`Create.summary` is a required-positional clap field by type — in
which case the test is updated to match the new optional signature.

## 8. CLI surface diff

```
bzr bug create
    [--template <NAME>]
    [--product <PRODUCT>]
    [--component <COMPONENT>]
    [--summary <SUMMARY>]            # was required; now optional when editor flow active
    [--version <VERSION>]
    [--description <TEXT>]
    [--description-file <PATH>]      # new; conflicts_with --description
    [--priority <PRIORITY>]
    [--severity <SEVERITY>]
    [--assignee <ASSIGNEE>]
    [--op-sys <OP_SYS>]
    [--rep-platform <REP_PLATFORM>]
    [--blocks <ID>,...]
    [--depends-on <ID>,...]
```

## 9. Risks / open questions

- **Stdin TTY detection.** `IsTerminal` on stdin is reliable on Linux
  and macOS. The edge case is a CI invocation with no controlling TTY
  and no pipe — stdin is non-TTY but empty. In that case stdin "wins"
  the precedence chain with empty content and the empty-description
  check fires (exit 7). Same surface as `bzr comment add` today; not
  a regression.
- **Editor template `<unset>` vs. omitting the line.** Spec keeps
  `<unset>` for visibility — helps testers spot missing required
  server-side fields (`op_sys`, `rep_platform` on installations that
  require them) before submission.
- **Locale.** Editor input is read as UTF-8. If `$EDITOR` writes
  Latin-1, we error. Same as `bzr comment add`; not a regression.
- **`--template` interaction with editor.** A saved template's
  `description` field becomes the editor body pre-fill. If
  `--template foo --description X` is supplied, the literal
  `--description` wins per the precedence chain — the template body
  is unused (consistent with current per-field merge: CLI > template).

## 10. Out of scope (intentionally not addressed here)

- Issues F (`bzr bug update --comment`/`--comment-file`/`--private-comment`),
  G (`--dupe-of`), H (list-mutation flags), I (extra field flags) from
  the parity review. Each gets its own spec/PR.
- bzl-style `@@PRIVATE@@` comment syntax.
- Multi-bug clone with shared editor session.
