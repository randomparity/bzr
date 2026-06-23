# Issue #419 — CLI type API boundary

Status: Draft
Issue: #419 "Decide and apply CLI type API boundary"
Related ADR: [docs/adr/0001-cli-types-are-crate-internal.md](../adr/0001-cli-types-are-crate-internal.md)

## Problem

The `bzr` library crate re-exports ~45 clap-derived action/argument types from
`src/cli/` as `pub` (`BugAction`, `ListArgs`, `QueryAction`, `AttachmentAction`,
…). Desloppify flags every one as a future-proofing risk: they are part of the
crate's public API yet are not marked `#[non_exhaustive]`, so adding a field or
variant is a breaking change for any external consumer.

The crate must take a single, consistent stance on these types and apply it
everywhere, without changing how the CLI parses arguments or dispatches
commands.

## Decision (see ADR-0001)

The clap-derived CLI action and argument types are **crate-internal
implementation details**, not a public API. They drop from `pub` to
`pub(crate)`. The only public seam of the library remains:

- `cli::Cli` (the parsed top-level command) — `pub`, because `src/main.rs`,
  `xtask`, and integration tests parse it.
- `dispatch(&Cli, OutputFormat, &mut Writers)` — `pub`, the single behavioral
  entry point that mirrors `main.rs`.
- `Cli`'s flag fields read by `main.rs` (`json`, `output`, `quiet`, `verbose`,
  `no_color`, `timeout`, …) stay `pub`.

Everything else under `src/cli/` becomes `pub(crate)`.

Rationale and rejected alternatives live in ADR-0001. In short: `bzr` is a CLI
*application*; its real public contract is argv → behavior → output, not the
Rust types used to model parsed arguments. Reducing visibility removes the
future-proofing surface at the source rather than papering over it with
`#[non_exhaustive]`, and avoids committing to a stable library API the project
does not intend to version.

## Scope of change

### Production code (`src/`)

1. **Action enums and argument structs** (43 types across `src/cli/attachment.rs`,
   `bug/**`, `classification.rs`, `comment.rs`, `component.rs`, `config.rs`,
   `field.rs`, `group.rs`, `product.rs`, `query.rs`, `server.rs`, `template.rs`,
   `user.rs`): `pub` → `pub(crate)`.

2. **`cli::Commands`** (`src/cli/mod.rs`): `pub` → `pub(crate)`. Required by
   E0446 — a `pub` enum cannot expose `pub(crate)` action types in its variant
   fields.

3. **`cli::Cli.command` field**: `pub` → `pub(crate)`. Required by E0446 — a
   `pub` struct cannot expose the now-`pub(crate)` `Commands` type. The `Cli`
   struct itself and its flag fields stay `pub`.

4. **Re-exports in `src/cli/mod.rs`**: the `pub use bug::{…}` /
   `pub use attachment::{…}` etc. blocks become `pub(crate) use`. A `pub use`
   of a `pub(crate)` item does not compile (E0365); internal modules import
   these types via the re-export path (`crate::cli::BugAction`), so the
   re-exports must remain, just at crate visibility.

### Binary crate (`src/main.rs`, `src/main_tests.rs`)

`main.rs` already only uses `Cli::parse()` + `dispatch(&cli, …)` for production.
Two test-only touch points break and must move to parse-based construction:

5. Remove `#[cfg(test)] use bzr::cli::Commands;` from `main.rs`.

6. `main_tests.rs::base_cli`/`dummy_command` hand-construct
   `Cli { …, command: Commands::Whoami }`. Replace with a helper that parses a
   real `Cli` (`Cli::try_parse_from(["bzr", "whoami"])`) and then mutates the
   public flag fields the test needs. This removes the dependency on the
   `pub(crate)` `command` field and `Commands` type while testing the same
   `resolve_format` / `tracing_filter_directive` logic.

### Integration tests (`tests/integration.rs`, external crate)

The file already contains `dispatch_cli`, `dispatch_cli_with_output`, and
`dispatch_cli_with_io` helpers that parse argv and call `bzr::dispatch`,
exercising "the same path as `main.rs::run()`" against the wiremock server set
up by `setup_test_env` (config has `default_server = "test"` with auth
pre-configured, so no `--server` flag or auth round-trip is needed).

7. **~56 execute-based tests** that hand-build an action
   (`BugAction::List(ListArgs { … })`) and call
   `bzr::commands::<resource>::execute(&action, &ctx, w)` migrate to the
   `dispatch_cli*` helpers with the equivalent argv. The wiremock mocks and
   response assertions are unchanged; only the invocation changes.

   **The migration is not context-equivalent — verify each test individually.**
   The old tests pass a *bare* context: `CommandContext::new(Some("test"), Json,
   None)`, where `credential_requirement`, `retry_max`, `request_timeout`,
   `dry_run`, and `assume_yes` all hold their defaults (in particular
   `credential_requirement = None`, `context.rs:36`). `dispatch` instead routes
   through `build_command_context` (`src/lib.rs:127`), which sets
   `.with_credential_requirement(command_requires_credentials(&cli.command))`
   plus retry/timeout/dry-run/assume-yes from the parsed flags.
   `connect_and_configure` consults that field —
   `if let Some(name) = command.credential_requirement() {
   require_credentials_for_connection(…)? }` (`connection.rs:540`) — so a
   migrated *write*-command test now traverses a credential-enforcement branch
   the original skipped. It passes because `setup_config` writes
   `api_key = "test-key"`, but this must be confirmed, not assumed.

   Migration obligation, per test:
   - Confirm the assertion outcome is unchanged: same `Ok`/`Err`, same
     `wiremock` `expect(n)` match, same captured stdout/JSON.
   - For credential-requiring commands, confirm the configured test creds
     satisfy the now-active credential gate (they do under the default
     `setup_test_env` config; a test that deliberately ran credential-free must
     keep that intent — see R5).
   - If an action carries a value the CLI layer rejects or normalizes at
     parse/validation time (so the value reaching `execute` via `dispatch`
     would differ from the hand-built action), do **not** force it through
     `dispatch`; move that assertion into a `src/cli/*_tests.rs` parser unit
     test instead.

8. **Parser-structure assertions that name inner types**
   (e.g. `if let BugAction::List(ListArgs { .. })`,
   `matches!(parsed.command, Commands::Whoami)`) lose access to those types.
   These assert on *parser output*, which is already covered by the
   `src/cli/*_tests.rs` unit tests. Convert each to an equivalent behavioral
   assertion through `dispatch_cli`, or drop the redundant structural check
   while keeping the behavioral one (e.g. bare `whoami` parses + `whoami show`
   is rejected — both provable without naming `Commands`).

### Out of scope / unchanged

- `Cli` stays `pub`; `xtask` manpage generation
  (`<bzr::cli::Cli as CommandFactory>::command()`) is unaffected.
- No clap derive attributes change; `#[non_exhaustive]` is **not** added.
- No CLI behavior, flag, help text, or dispatch routing changes.
- Domain types in `types/`, `error.rs`, `config/model.rs`, `client/` keep their
  existing `pub` + `#[non_exhaustive]` convention (genuinely model external
  data; out of scope for this issue).

## Invariants / success criteria

- `cargo build`, `cargo clippy --all-targets --features test-helpers
  -- -D warnings`, and `cargo test --features test-helpers` all pass with zero
  warnings.
- `make check-test-layout` and `make check-no-spawn` pass (no inline `mod tests`;
  sibling `_tests.rs` layout preserved).
- `rg 'bzr::cli::(BugAction|ListArgs|…|Commands)' tests/ ` returns nothing — no
  external code names a now-internal type. (`bzr::cli::Cli` references are
  allowed.)
- `git grep -n '^pub (struct|enum)' src/cli` shows only `Cli` remaining `pub`.
- Generated manpages are byte-identical (CLI surface unchanged): the Manpages CI
  job stays green.
- No change to any user-visible output, exit code, or parse behavior. The full
  test suite passing is the behavior-preservation proof.
- No migrated test changes its pass/fail outcome: each test asserts the same
  `Ok`/`Err`, the same `wiremock` `expect(n)` match, and the same captured
  output before and after migration. A test whose outcome would flip under
  `dispatch` is a defect in the migration, not an accepted change.

## Risks and mitigations

- **R1 — argv→dispatch context differs from direct execute.** `execute()` and
  `dispatch` share the same `connect_and_configure` path, but the *context* they
  pass differs: `dispatch` enriches it via `build_command_context`
  (`credential_requirement`, retry, timeout, dry-run, assume-yes) where the old
  tests passed a bare `CommandContext::new`. The concrete behavioral hinge is the
  credential gate at `connection.rs:540`. Mitigation: the default
  `setup_test_env` config supplies header auth + `api_key`, satisfying the gate;
  the per-test migration obligation in item 7 requires confirming outcome
  equality rather than assuming it. The existing `dispatch_cli`-based tests in
  the same file already exercise this enriched path against the same config.
- **R5 — a test deliberately ran a credential-requiring command without
  credentials.** Such a test would now fail at the `connection.rs:540` gate.
  Mitigation: audit for tests that assert a *non-auth* error from a
  credential-requiring command under the default config; if any exist, keep
  their original intent (e.g. by asserting the credential error explicitly, or
  by leaving that specific assertion as a parser/unit test) rather than
  silently absorbing the gate.
- **R2 — a test asserted on something only reachable via the constructed
  action.** Mitigation: audit each migrated test; if a behavioral assertion is
  not expressible through dispatch, move that specific check into a
  `src/cli/*_tests.rs` parser unit test rather than weakening it.
- **R3 — E0446 cascade reaches further than `Commands`/`Cli.command`.**
  Mitigation: the compiler enumerates every site; fix iteratively under
  `-D warnings` until clean. `Cli` flag fields stay `pub` so `main.rs` is
  unaffected.
- **R4 — behavior change hidden by test migration.** Mitigation: migrate in
  small commits, keeping the suite green at each step; never delete a test's
  assertion intent, only its construction mechanism.

## Rollback

Pure visibility + test-shape change on a feature branch. Rollback is reverting
the branch; no migrations, persisted state, or external effects are involved.
