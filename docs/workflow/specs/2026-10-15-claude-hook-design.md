# Restore Claude commit checks (#886)

## Problem

The Bash PreToolUse hook reads an unset variable rather than JSON stdin.
Its failing Cargo status also does not block Claude Code (issue #886).

## Scope

Replace the inline command in `.claude/settings.json` with a Bash adapter
`tools/claude-pre-commit.sh`, resolved through `CLAUDE_PROJECT_DIR`.
It reads `.tool_input.command` using jq, retaining the existing literal
`git commit` substring match. It never executes the received command.
The adapter resolves the Git root from JSON `cwd` (not the initial project
root) and invokes a Makefile `check-commit` recipe there: format check, the existing all-target/test-helpers Clippy recipe,
and the existing quiet test recipe. All check output goes to stderr;
a failed check produces an actionable message and exit 2. Success or an
unrelated command returns 0. Remove the variable-based inline path.
The adapter owns protocol translation; Makefile owns check commands.
Native Git hook policy and the compiled CLI stay unchanged. No general shell
parser or bypass-prevention redesign; maintainers own those excluded areas.

### Failure model

- Actors/deployments: local developers using Claude Bash hooks, including sibling worktrees.
- Invariant: checks validate the invocation checkout; failures block with diagnostics.
- Accepted: substring false positives/negatives; broader shell parsing is excluded.
- Elsewhere: native Git hooks retain their existing maintainer-owned commit policy.
Malformed JSON/non-string command, missing jq, or unavailable cwd/Git/Make
must return 2 with stderr after adapter launch. Absent/null command returns 0.

### Threat model

- Boundary: existing Claude stdin/path metadata enters the local adapter; no widened access.
- Actors: trust operator-owned checkout and Claude cwd metadata; command text stays data.
- Controls: jq validates input; quote cwd/path arguments; never evaluate command text.
- Excluded: hostile checkout/PATH replacement and shell bypass detection; local operator owns these.

## Success

C1: stdin-only invocation recognizes matching commands without the old variable.
C2: matching commits run format, Clippy and tests sequentially.
C3: failure at each check stops later checks, exits 2 and explains failure on stderr.
C4: successful and unrelated commands exit 0 with empty stdout.
C5: a manually runnable harness exercises the configured hook; no CI policy changes.

## Validation

Mode: focused-test. `tools/claude-pre-commit-tests.sh` executes the configured
hook with JSON stdin and fake Cargo commands, observing exit, streams and
call order. Cover C1-C4, each check failure, unrelated/missing/null commands,
malformed/non-string input, missing jq/Git/Make, unavailable cwd, and distinct
project/invocation roots with spaces. Assert failures run no later checks. Red: the original hook skips the
stdin commit checks and fails expected call order. Green command:
`bash tools/claude-pre-commit-tests.sh`.
C5 is covered by the same configured-hook harness. CI wiring is excluded by
the operator. The generic `check-commit` recipe is tested through fake Cargo
call order, including its format, Clippy and quiet test arguments.
Run shell lint/format guards, repository pre-commit checks and `make test`.
Run `make functional-test` before opening the PR, as repository policy requires.
