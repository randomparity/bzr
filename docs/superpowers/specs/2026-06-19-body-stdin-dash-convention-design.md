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

### Shared resolver

Add to `commands/shared.rs`:

```rust
/// Read all of stdin to a String. Shared by the `-` (dash) convention and
/// the flag-omitted auto-stdin paths.
pub(crate) fn read_stdin_to_string() -> Result<String>;

/// Resolve a body from an inline flag value and/or a `--*-file` path,
/// honouring the `-` = stdin convention for both. Returns `Ok(None)` when
/// neither source was supplied, so the caller applies its own fallback
/// (piped stdin, `$EDITOR`, or "no body"). The two sources are mutually
/// exclusive.
pub(crate) fn resolve_body_source(
    inline: Option<&str>,
    file: Option<&std::path::Path>,
    inline_flag: &str,
    file_flag: &str,
) -> Result<Option<String>>;
```

`resolve_body_source` semantics:

| `inline`        | `file`          | Result                                   |
| --------------- | --------------- | ---------------------------------------- |
| `Some("-")`     | `None`          | `Ok(Some(stdin))`                        |
| `Some(s)`       | `None`          | `Ok(Some(s))`                            |
| `None`          | `Some("-")`     | `Ok(Some(stdin))`                        |
| `None`          | `Some(path)`    | `Ok(Some(read_file_with_context(path)))` |
| `Some(_)`       | `Some(_)`       | `Err(InputValidation, "X and Y are mutually exclusive")` |
| `None`          | `None`          | `Ok(None)`                               |

The `Some(_), Some(_)` arm is a defence-in-depth guard; clap `conflicts_with`
makes it unreachable in normal use, but the resolver must not depend on the
CLI layer for correctness.

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

- **`comment add`** — add `--body-file: Option<PathBuf>` with
  `conflicts_with = "body"`. Resolve via
  `resolve_body_source(body, body_file, "--body", "--body-file")?`; on `None`,
  fall back to the existing `read_comment_body()` (piped stdin / `$EDITOR`).
  The existing `text.trim().is_empty()` check is unchanged.
- **`bug create`** — `resolve_description` first calls
  `resolve_body_source(description, description_file, "--description", "--description-file")?`;
  on `None` it keeps the existing piped-stdin fallback and `Ok(None)` (→
  `$EDITOR`) tail.
- **`bug update`** — `resolve_comment` replaces its `match (comment, comment_file)`
  with `resolve_body_source(...)`, preserving the existing `comment_private`
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
  path. An explicit `--description -` over empty stdin yields an empty
  description, identical to today's `--description ""` — no new validation is
  added.

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

Unit tests (sibling `*_tests.rs`) drive the resolver directly with injected
values — no real stdin needed for the literal/file/mutual-exclusion arms:

- `resolve_body_source`: inline literal; inline `-` (stdin behaviour exercised
  via integration, see below); file path; file `-`; mutual-exclusion error;
  both-None → `None`.
- `read_file_with_context` for `--body-file` missing/unreadable path → exit 7.
- Mutual-exclusion error messages name the correct flags for each command.

stdin-dependent arms (`--body -`, `--body-file -`) are exercised through the
binary in `tests/integration.rs` (or a functional test) by piping to a
child process, since `read_stdin_to_string` reads the real fd 0.
