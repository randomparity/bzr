# Issue #363: bug update URL and Target Milestone Design

## Context

Issue #363 asks for parity between `bug create` and `bug update` for two fields:
`--url` and `--target-milestone`. `bug create` can already send both values in
`CreateBugParams`, but `UpdateArgs` and `UpdateBugParams` do not expose either
field today.

This is a narrow command-surface change. It unblocks structured `bug update`
input work in #365 because the accepted update field set should include the full
public flag surface.

## Current Evidence

- `src/cli/bug.rs` has `CreateFields::url` and
  `CreateFields::target_milestone`, but `UpdateArgs` has neither field.
- `src/types/bug.rs` serializes `CreateBugParams.url` and
  `CreateBugParams.target_milestone`, but `UpdateBugParams` has neither field.
- `src/commands/bug/update.rs::build_update_params` cannot copy these values
  into a request body because they are absent from the CLI and payload type.
- `docs/bzr-cli.md` and `CHANGELOG.md` document create parity, but not update
  support for these two flags.

## Design

Add two optional scalar fields to `bug update`:

- `--url <URL>` sets Bugzilla's URL field for the target bug.
- `--target-milestone <MILESTONE>` sets Bugzilla's target milestone field.

Both flags are optional and serialize only when present, matching the current
`UpdateBugParams` pattern for scalar fields such as `alias`, `deadline`,
`summary`, and `whiteboard`.

No URL validation is added in this issue. `bug create --url` forwards the value
as provided, and the server remains the source of truth for whether a value is
accepted.

No special multi-ID validation is added. Unlike alias, Bugzilla does not have a
known single-bug-only constraint for the URL field or target milestone, so a
single update request shape can be applied to one or many IDs.

## Files

- `src/cli/bug.rs`: add `url` and `target_milestone` to `UpdateArgs`.
- `src/commands/bug/update.rs`: copy the new fields into `UpdateBugParams`.
- `src/types/bug.rs`: add optional serialized fields to `UpdateBugParams`.
- `src/cli/mod_tests.rs`: prove clap parses both flags.
- `src/commands/bug/update_tests.rs`: prove the command handler sends both
  fields in the PUT body.
- `src/types/bug_tests.rs`: prove update serialization includes both fields
  when set and omits them in the existing default scalar-field omission test.
- `docs/bzr-cli.md`: document both `bug update` flags.
- `CHANGELOG.md`: note the user-visible behavior under the unreleased section.

## Testing

Focused tests are enough because the change follows existing scalar update
patterns:

- `cargo test parse_bug_update_url_and_target_milestone`
- `cargo test update_bug_params_serializes_scalar_parity_fields`
- `cargo test update_bug_params_default_omits_scalar_parity_fields`
- `cargo test bug_update_sends_url_and_target_milestone_body`
- `cargo test bug_update --lib`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Out of Scope

- `bug update --from-json`; tracked by #365.
- Input schemas for structured update payloads; tracked by #366.
- Custom fields or arbitrary `cf_*` writes.
- Client-side URL format validation.

## Self-Review

- No placeholder requirements remain.
- The design does not add new behavior beyond issue #363.
- The testing plan covers clap parsing, payload serialization, and command
  handler request body as required by the issue.
- The docs/changelog updates are included because this is a user-visible
  command change.
