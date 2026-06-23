# 0002 — Per-test config-path isolation replaces the shared env lock

- Status: Accepted
- Date: 2026-06-23
- Issue: #421

## Context

Async tests that exercise config loading, server connection, and auth/API
detection point `bzr` at a throwaway config by mutating the process-global
`XDG_CONFIG_HOME` environment variable. Because that variable is process-global
and is read *lazily* inside `connect_and_configure` / `dispatch` (not at test
setup time), every such test must:

1. Serialize behind a single `tokio::sync::Mutex` (`ENV_LOCK`) so concurrent
   tests do not clobber each other's `XDG_CONFIG_HOME`, and
2. Hold the guard across **all** of its `.await` points — including the mock
   network round-trips — because the env var must still be set when the command
   reads it deep inside the call stack.

Desloppify flags these tests as "async lock guard held across await points."
The guard-across-await pattern is not itself a `tokio::sync::Mutex` misuse (that
type is designed to be held across awaits), but it does make the whole
env-touching test suite a single global critical section: tests cannot run in
parallel, and the breadth of the lock hides which state it actually protects.

The production code already supports an alternative: every config-loading and
config-persisting call site threads `CommandContext::config_path_override`
(surfaced from the `--config` global flag). `Config::path_at` resolves with the
precedence `explicit override > BZR_CONFIG > XDG_CONFIG_HOME`, so an explicit
path short-circuits before any environment variable is consulted.

## Decision

Tests select their throwaway config by passing an **explicit per-test config
path** — via `CommandContext::with_config_path_override` for `execute`-level
tests, or the `--config <path>` flag for `dispatch`-level tests — instead of
mutating `XDG_CONFIG_HOME`. Such tests acquire no shared lock and run in
parallel.

`ENV_LOCK` is retained for, and only for, tests that genuinely mutate
process-global environment state that command resolution must observe:

- tests that `set_var`/`remove_var` an **API-key environment variable** named by
  `api_key_env` / an inline `--server-api-key-env`, and
- the test that deliberately verifies `--config` overrides `XDG_CONFIG_HOME`
  precedence (it must set both).

Each retained `ENV_LOCK` acquisition keeps a comment stating which
process-global variable it protects and why the explicit-path approach does not
apply.

This is scoped to the four files named in issue #421
(`src/test_helpers.rs`, `src/lib_tests.rs`,
`src/commands/runtime/shared/mod_tests.rs`, `src/commands/field_tests.rs`). The
shared `setup_test_env` / `setup_empty_config_env` helpers stay in place,
unchanged, for the ~100 call sites in other test files that are out of scope.

## Consequences

- Migrated tests no longer serialize behind `ENV_LOCK`; they hold no guard
  across awaits, so the desloppify finding clears for them and they can run
  concurrently.
- The invariant guarded by `ENV_LOCK` narrows from "any test that wants an
  isolated config" to "a test that mutates a process-global env var." The lock's
  purpose becomes legible at every remaining call site.
- A new lightweight helper writes a config file to a temp dir and returns its
  path (no env mutation), centralizing the directory/permissions boilerplate the
  per-file writers previously duplicated.
- Test behavior is unchanged: same configs, same mocks, same assertions. Only
  the config-selection mechanism (explicit path vs. env var) changes.

## Considered & rejected

- **Narrow the lock to wrap only the `set_var` call.** Not feasible: the env var
  is read lazily inside the command under test, long after setup returns, so the
  guard must remain held until the command finishes reading config. Narrowing the
  guard to the mutation alone would reintroduce the race the lock exists to
  prevent.
- **Replace `ENV_LOCK` with a per-variable lock registry.** More machinery than
  the problem warrants; the handful of genuine env-mutating tests are cheap to
  serialize behind one mutex, and a registry would not remove the
  guard-across-await pattern for them.
- **Switch `ENV_LOCK` to a `std::sync::Mutex`.** Does not address the finding —
  the guard would still span awaits (and a std mutex held across an await is a
  genuine clippy `await_holding_lock` violation, which is worse).
- **Migrate all ~100 `setup_test_env` call sites.** Out of scope for #421 and a
  much larger, riskier change; the four flagged files capture the tests the audit
  identified.
