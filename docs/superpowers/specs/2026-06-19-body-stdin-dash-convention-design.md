# Design: `-` (dash) stdin convention for body-input flags

- **Issue:** #295 — "test comment is not posting to bugzilla"
- **Date:** 2026-06-19
- **Status:** Accepted

## Problem

A user ran:

```sh
echo "Test comment from command line" | bzr comment add 203732 --body -
```

and expected the piped text to be posted. Instead, the literal string `-`
was posted as the comment body.

The cause is in `commands/comment.rs`: the body is resolved as

```rust
let text = match body {
    Some(t) => t.clone(),       // --body "-" lands here as the literal "-"
    None => read_comment_body()?, // stdin/editor only when --body is omitted
};
```

`--body -` makes clap set `body = Some("-")`, so the literal dash is posted.
The stdin path only runs when `--body` is omitted entirely. The reporter's
mental model — `-` as a stand-in for stdin — is the widespread Unix
convention (`cat -`, `git apply -`, `kubectl apply -f -`), which `bzr`
currently honours nowhere.

## Goals

1. `bzr comment add <id> --body -` reads the comment body from stdin.
2. The same `-`-means-stdin convention applies consistently to the other
   body-input flags: `bug create --description` and `bug update --comment`.
3. Add a `--body-file PATH` companion to `comment add`, matching the existing
   `--description-file` / `--comment-file` pattern, where `PATH` of `-` also
   means stdin.
4. The `-` convention also works on the existing file companions:
   `--description-file -` and `--comment-file -` read stdin.
5. No regression to the flag-omitted behaviours (auto-read piped stdin, or
   open `$EDITOR` at a TTY) for `comment add` and `bug create`.

## Non-goals

- Changing `attachment add --comment` (a one-line annotation, not a body
  composed from stdin). Out of scope for #295.
- Adding empty-body validation where none exists today. Each call site keeps
  its current empty-body handling (see "Empty input" below).
- Introducing a `docs/adr/` tree. This repo records decisions as dated specs;
  this document is that record.

## Scope of change

Three commands and one shared resolver:

| Command       | Inline flag     | File flag (`-` = stdin)        | Flag-omitted fallback        |
| ------------- | --------------- | ------------------------------ | ---------------------------- |
| `comment add` | `--body`        | `--body-file` (**new**)        | piped stdin, else `$EDITOR`  |
| `bug create`  | `--description` | `--description-file` (exists)  | piped stdin, else `$EDITOR`  |
| `bug update`  | `--comment`     | `--comment-file` (exists)      | none (no comment)            |

## Decision

### Shared resolver — pure classifier + thin materializer

The logic is split so the new branching (dash detection, mutual exclusion,
source selection) is a **pure function** that needs no real file descriptor,
and only a trivial wrapper touches I/O. This is what makes the change
unit-testable in the repo's in-process integration harness (see "Test plan").

Add to `commands/shared.rs`:

```rust
/// Where a body string comes from. Pure result of classifying the
/// inline + file flag pair; carries no I/O.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodySource {
    /// A literal inline value (`--body "text"`).
    Literal(String),
    /// Read all of stdin (`--body -` or `--*-file -`).
    Stdin,
    /// Read a UTF-8 file (`--*-file PATH`, PATH != "-").
    File(std::path::PathBuf),
    /// Neither source supplied; caller applies its own fallback.
    None,
}

/// Pure classifier. No file descriptors are touched. `inline_flag` /
/// `file_flag` name the originating options for the mutual-exclusion error.
/// The two sources are mutually exclusive; clap `conflicts_with` makes the
/// both-present arm unreachable in normal use, but the classifier guards
/// regardless so correctness does not depend on the CLI layer.
pub(crate) fn classify_body_source(
    inline: Option<&str>,
    file: Option<&std::path::Path>,
    inline_flag: &str,
    file_flag: &str,
) -> Result<BodySource>;

/// Read all of stdin to a String. Shared by the materializer and the
/// flag-omitted auto-stdin paths.
pub(crate) fn read_stdin_to_string() -> Result<String>;

/// Thin materializer: turn a classified source into the actual body, or
/// `Ok(None)` for `BodySource::None` so the caller applies its fallback.
/// The only place that performs stdin/file I/O for explicit flags.
pub(crate) fn materialize_body_source(
    source: BodySource,
    file_flag: &str,
) -> Result<Option<String>>;
```

`classify_body_source` semantics (pure — fully unit-tested):

| `inline`        | `file`          | Result                                   |
| --------------- | --------------- | ---------------------------------------- |
| `Some("-")`     | `None`          | `Ok(BodySource::Stdin)`                  |
| `Some(s)`       | `None`          | `Ok(BodySource::Literal(s))`             |
| `None`          | `Some("-")`     | `Ok(BodySource::Stdin)`                  |
| `None`          | `Some(path)`    | `Ok(BodySource::File(path))`             |
| `Some(_)`       | `Some(_)`       | `Err(InputValidation, "X and Y are mutually exclusive")` |
| `None`          | `None`          | `Ok(BodySource::None)`                   |

`materialize_body_source` maps `Literal(s) → Some(s)`, `Stdin →
Some(read_stdin_to_string())`, `File(p) → Some(read_file_with_context(p, file_flag))`,
`None → None`. Call sites compose the two:
`materialize_body_source(classify_body_source(...)?, file_flag)?`.

### `-` reads stdin unconditionally

When the value is `-`, stdin is read directly (`read_stdin_to_string`) with
**no** `is_terminal` check. This is the deliberate distinction from omitting
the flag:

- **Flag omitted** → "read stdin *if piped*, otherwise open `$EDITOR`."
- **`--body -`** → "read stdin, full stop." At a TTY with nothing piped this
  blocks for keyboard input until EOF (Ctrl-D), exactly like `cat -`.

This is the standard, expected behaviour of an explicit `-` and keeps the two
idioms semantically distinct.

### Call-site wiring

Each call site composes `materialize_body_source(classify_body_source(...)?, file_flag)?`:

- **`comment add`** — add `--body-file: Option<PathBuf>` with
  `conflicts_with = "body"`. On `Some(text)`, use it; on `None`, fall back to
  the existing `read_comment_body()` (piped stdin / `$EDITOR`). The existing
  `text.trim().is_empty()` check is unchanged.
- **`bug create`** — `resolve_description` composes the resolver for
  `--description` / `--description-file`; on `None` it keeps the existing
  piped-stdin fallback and `Ok(None)` (→ `$EDITOR`) tail.
- **`bug update`** — `resolve_comment` replaces its `match (comment, comment_file)`
  with the composed resolver, preserving the existing `comment_private`
  and empty-body checks afterwards. `resolve_comment` is called once at
  params-build time (before the per-bug loop), so the single stdin read is
  correct for batch updates.

The three existing ad-hoc stdin reads (`comment.rs::read_comment_body`,
`bug/create.rs::resolve_description`, and the new dash path) are consolidated
onto `read_stdin_to_string`.

## Consequences

- A body that is literally the single character `-` can no longer be supplied
  via `--body -` (it now means stdin). Mitigation: pipe it
  (`printf - | bzr comment add <id>`) or use `--body-file` with a file whose
  contents are `-`. A single-dash body has no real use case; this is the same
  trade-off every `-`-as-stdin tool accepts.
- `comment add` gains a `--body-file` flag → the man page, `docs/bzr-cli.md`,
  the command-surface drift check, and `CHANGELOG.md` must be updated in the
  same PR.
- The behaviour is now uniform across the three body commands, removing the
  prior inconsistency where stdin was reachable only by omitting the flag.

## Empty input

Behaviour per path is preserved, not newly unified:

- `comment add` and `bug update` already reject an empty/whitespace body
  (`text.trim().is_empty()` → exit 7) after resolution, so `--body -` /
  `--comment -` over empty stdin errors exactly like an empty literal.
- `bug create` keeps its existing empty-piped-stdin error on the *flag-omitted*
  path. An explicit `--description -` (or `--description-file`) over empty input
  yields an empty description, identical to today's `--description ""`. This
  asymmetry is **intentional**: explicit sources pass through unvalidated (the
  caller asked for exactly this content), while the flag-omitted convenience
  path guards against the "piped from an empty producer by mistake" case. The
  server still rejects a genuinely empty required description with exit 4, so no
  data-loss path is opened.

## Considered & rejected

- **`--body-file` only (gh-style), keep `--body` literal.** Rejected: it does
  not fix the reporter's exact `--body -` invocation, which would still post a
  literal `-`. The chosen design does both.
- **`-` only on `comment add`.** Rejected (operator decision): leaves
  `bug create --description -` / `bug update --comment -` inconsistent.
- **Reject `--body -` with a "did you mean to omit --body?" error.** Rejected:
  the user's invocation is a reasonable convention; honour it rather than
  teach-by-error.
- **Honour `-` only when stdin is non-TTY, else error.** Rejected: an explicit
  `-` blocking for TTY input is the standard, least-surprising behaviour and
  keeps the dash path semantically distinct from the flag-omitted path.

## Test plan

The classifier carries all the new branching, so the behaviour that #295 is
about is covered by pure unit tests with **no** file descriptor or process
spawn required.

Unit tests (sibling `shared_tests.rs`) on `classify_body_source` — pure, no I/O:

- `Some("-"), None` → `Stdin`
- `Some("text"), None` → `Literal("text")`
- `None, Some("-")` → `Stdin`
- `None, Some("path")` → `File("path")`
- `Some(_), Some(_)` → `Err` whose message names both flags (one case per
  command's flag pair, to lock the messages).
- `None, None` → `None`

Unit tests on `materialize_body_source`:

- `File(missing_path)` → `InputValidation` (exit 7), message names the file flag.
- `Literal`/`None` map straight through (no I/O).
- (`Stdin` is the one arm that reads fd 0; see below.)

Integration (`tests/integration.rs`, in-process against wiremock, the
established pattern): assert that `comment add --body "literal"`,
`--body-file <tmpfile>`, and the mutual-exclusion error each behave correctly
end-to-end through `dispatch`. These need no stdin.

The single `Stdin` materialization (`read_stdin_to_string` reading real fd 0)
is the only arm not reachable in-process. It is the thinnest possible function
(read fd 0 to a String); one focused test redirects fd 0 to a temp file/pipe
to confirm it returns the bytes. No binary spawn and no new harness is
introduced.
