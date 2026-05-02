# code-comment-reviewer — Project Memory

## Project: bzr (Bugzilla CLI, Rust)

This file is loaded into the agent's system prompt every session. Keep it
under 200 lines. Offload detail to topic files and link from here.

## Topic files

(none yet — populate as audits surface recurring patterns)

## Stable references

- Style guide and rendering rules: `docs/plans/2026-05-02-cli-doc-expansion.md`
- Optional standalone style guide (created in phase 1 of the plan): `docs/dev/cli-doc-style.md`
- Canonical CLI reference: `docs/bzr-cli.md`
- Manpage generator: `xtask/src/main.rs` (uses `clap_mangen` 0.3)
- Exit codes: `src/error.rs` — `BzrError::exit_code()` is the source of truth.

## Recurring drift sources

(empty — populate as audits expose patterns)

## Rendering quirks (clap_mangen 0.3)

(empty — populate after the first phase-1 render confirms or refutes the
`verbatim_doc_comment` question for indented example blocks)

## False positives

(empty)
