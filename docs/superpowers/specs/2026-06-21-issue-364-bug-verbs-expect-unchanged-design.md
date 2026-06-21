# Issue #364: Convenience Verb Concurrency Guard Design

## Context

Issue #364 asks for the optimistic-concurrency guard from `bug update` to be
available on the state-transition convenience verbs:

- `bug resolve`
- `bug close`
- `bug reopen`
- `bug dup`

`bug update --expect-unchanged-since` already re-reads every target bug before
writing and exits 14 if any `last_change_time` differs. The convenience verbs
currently build `UpdateBugParams` and call `update::apply` directly, bypassing
that guard.

## Design

Add `--expect-unchanged-since <TIMESTAMP>` to the four convenience verb argument
structs. The flag keeps the same timestamp contract and error wording as
`bug update`.

Move the guard invocation into a shared update-application helper that accepts a
small request struct:

- target IDs
- `UpdateBugParams`
- optional expected timestamp

`bug update` and all four convenience verbs call that helper. The helper runs
the guard before `update::apply`, skips the guard under `--dry-run`, and preserves
the existing batch confirmation/write behavior.

`bug close` and `bug reopen` still validate the target status before the guard.
That preserves their existing local validation order: malformed comment/private
usage fails before network I/O, then status validity is checked, then the
concurrency guard runs before any PUT.

## Files

- `src/cli/bug.rs`: add the flag to `ResolveArgs`, `CloseArgs`, `ReopenArgs`,
  and `DupArgs`; update verb help text.
- `src/commands/bug/update.rs`: add a guarded apply helper and reuse it from
  `handle`.
- `src/commands/bug/verbs.rs`: pass each verb's expected timestamp to the shared
  guarded helper.
- `src/cli/mod_tests.rs`: prove each verb parses the flag.
- `src/commands/bug/verbs_tests.rs`: prove each verb aborts on a mismatch and no
  PUT fires; prove a matching timestamp still writes the normal verb payload.
- `docs/bzr-cli.md`: document the flag for all four convenience verbs.
- `CHANGELOG.md`: add an Unreleased entry.

## Testing

- `cargo test parse_bug_verbs_expect_unchanged_since`
- `cargo test bug_verbs_expect_unchanged_since --lib`
- `cargo test commands::bug::verbs::tests --lib`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Out of Scope

- Changing the client-side compare-and-set semantics.
- Adding server-side idempotency or atomic update behavior.
- Changing dry-run behavior; dry-run still previews without a guard re-read.

## Self-Review

- The design uses the existing guard implementation rather than creating a new
  comparison path.
- The test plan covers all four verbs, the no-write invariant, and the guarded
  success path.
- The status validation order for `close`/`reopen` remains explicit.
