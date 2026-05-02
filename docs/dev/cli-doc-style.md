# CLI doc-comment style guide

Source-of-truth conventions for the `///` doc comments under `src/cli/`.
These comments drive both `bzr <cmd> --help` long-form output and the
`man/man1/*.1` pages produced by `clap_mangen`. Every comment ships to
end users on every release.

For background, scope, and phasing, see
[`docs/plans/2026-05-02-cli-doc-expansion.md`](../plans/2026-05-02-cli-doc-expansion.md).

## How clap-derive turns `///` into help text

Clap-derive splits a doc comment at the first blank `///` line:

- First paragraph → `about` / `help` (used in `bzr --help` summaries
  and the man page NAME line).
- Full text → `long_about` / `long_help` (used in `<cmd> --help` long
  form and the man page DESCRIPTION).

A trailing period on the *short* paragraph's last sentence is stripped
by clap. Inside each paragraph, newlines collapse to spaces unless
`#[command(verbatim_doc_comment)]` (or `#[arg(verbatim_doc_comment)]`)
is set on the item.

## Required attribute on items with examples

If a doc comment contains an indented example block, set
`#[command(verbatim_doc_comment)]` on the item. Without it,
`clap_mangen` collapses the example lines into a single space-separated
paragraph that is unreadable as commands.

```rust
/// Create a new bug under a product and component.
///
/// ...prose...
///
/// Examples:
///
///   bzr bug create --product P --component C --summary "Crash"
#[command(verbatim_doc_comment)]
Create { ... }
```

## Indent example blocks at 2 spaces, not 4

Indent each example line **2 spaces** from the surrounding prose. Do
**not** use 4 or more spaces.

CommonMark treats 4+ leading spaces as a code block, and rustdoc
compiles unfenced code blocks as Rust by default — your shell command
becomes a doctest that fails to compile. With 2 spaces, rustdoc
treats the lines as a paragraph (so doctests pass), and `clap_mangen`
with `verbatim_doc_comment` still preserves the layout in the man
page output.

Continuation of a single example with `\` line-continuation indents
**2 more spaces** (so 4 total from the surrounding prose):

```rust
/// Examples:
///
///   bzr config set-server prod --url https://bz.example.com \
///     --api-key-env BZR_API_KEY
///   bzr config set-default prod
```

## ASCII only — no em dashes or smart quotes

Em dashes (`—`, U+2014), en dashes (`–`, U+2013), and curly quotes
(`“ ” ‘ ’`) round-trip through the roff pipeline as mojibake (`â`).
Use ASCII alternatives:

- Em dash → `--` (double hyphen)
- En dash → `-` (single hyphen)
- Curly quotes → straight `"` and `'`

## Suppress `clippy::doc_markdown` on items with shell examples

Clippy's `pedantic` `doc_markdown` lint flags bare URLs and CamelCase /
ALL_CAPS identifiers in doc comments. Wrapping them in `<>` or
backticks would degrade copy-paste UX in the rendered help output.

For items whose doc comment includes shell examples that mention bare
URLs or env-var names, add an `#[expect]` (the project denies bare
`#[allow]`):

```rust
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
```

Apply at the smallest scope that covers all hits — a struct, an enum,
or a single variant. Module-level suppression is too broad.

## Required shape per level

### Top-level `Cli` (in `src/cli/mod.rs`) — produces `bzr.1`

- Short paragraph: one declarative sentence (NAME line).
- Long body: introduction, config location, output behavior, exit-code
  overview, 2–4 representative examples, "See bzr-foo(1), …"
  cross-reference list of every per-resource page.
- Drop any redundant `#[command(about = "...")]` once the doc comment
  is present.

### Subcommand groups (`Commands` variants in `src/cli/mod.rs`)

- Short paragraph: one-sentence orientation, leading with the verbs
  the group covers (e.g. "List, view, search, create, clone, update,
  history bugs.").
- Long body covers, in order:
  1. Auth / permission expectations (e.g. "read paths free, write
     paths require credentials").
  2. Cross-cutting flag semantics that recur across actions in the
     group (e.g. filter-flag repeatability and `!`-prefix negation).
  3. 2–4 representative example invocations.
  4. Cross-references to every per-action manpage.

### Action variants (e.g. `BugAction::Create`)

Every action's long body must hit each applicable item from this
checklist:

| Section | When to include |
|---|---|
| What it does | always (second paragraph) |
| Required vs. conditional vs. optional inputs | always when more than one flag |
| Output shape | when output is more than "the resource" |
| Side effects beyond the obvious | when present |
| Examples | always; minimum 1, ideally 2 |
| Exit codes | when behavior differs from generic 0/1 |
| Env-var and stdin behavior | when the command reads them |
| Cross-references | when a related command exists |

### Individual args / options

A short single-line doc is fine for most flags. Expand into a
multi-paragraph doc when **any** of these applies:

- The flag conflicts with another flag (document which, and the
  recommended alternative).
- The flag has a non-obvious unit or default.
- The flag accepts a structured value (URL, flag-syntax, ID list).
- The flag gates behavior elsewhere (e.g. `--all`, `--template`).
- The flag has env-var fallback or stdin behavior.

For repeatable filter flags (`Vec<String>` typed), always document
repeatability and the `!` prefix for negation.

For enum-typed flags (`--type`, `--auth-method`, `--api`, `--output`),
mention the default; don't enumerate values clap will already
enumerate via `value_parser`.

## Cross-cutting conventions

- **Source line length** ≈ 78–88 chars (renders well in both `--help`
  and the man page). Hard cap is 100 per `CLAUDE.md`.
- **Backticks** around literal flag names, env vars, file paths, and
  code identifiers. Never use them for emphasis. Avoid them inside
  shell-example blocks (the user copy-pastes those literally).
- **No phantom features.** Every flag, env var, exit code, and
  `bzr-foo-bar(1)` reference mentioned in a doc must exist in the
  current code.
- **No "previously this used to..."**. Document current behavior only
  (`CLAUDE.md` "replace, don't deprecate").
- **Exit codes:** prefer named codes ("exit code 4 on Bugzilla API
  error") over bare numbers. Defer the full table to
  `docs/bzr-cli.md`.
- **No puff.** Avoid "robust", "comprehensive", "elegant", "powerful",
  "seamless".
- **No bullet lists in roff-rendered prose** — `-` items render as a
  literal hyphen with no special formatting. Use prose ("Mutually
  exclusive with `--foo`, `--bar`, and `--baz`") instead.

## Validating before commit

```bash
make man                                # regenerate man/man1/*.1
git diff --stat man/man1/               # confirm only intended pages changed
groff -man -Tutf8 man/man1/<page>.1     # eyeball each affected page
cargo run -- <cmd> --help               # confirm long-form help reads cleanly
cargo test --workspace --locked         # parser tests + doctests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

If you're adding examples and a doctest fails on `cli::Foo::Bar (line
N)`, you almost certainly used 4+ space indentation. Reduce to 2.

If a man page shows mojibake (`â`, `Ã©`), grep the source for non-ASCII
characters — em dashes and curly quotes are the usual culprits.
