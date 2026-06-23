# Issue #421 — Clean async test environment locking

- Status: Draft
- Date: 2026-06-23
- ADR: [0002](../../adr/0002-test-config-isolation-over-env-lock.md)

## Problem

Desloppify flags many async tests that hold a `tokio::sync::Mutex` guard
(`ENV_LOCK`) across `.await` points. The lock exists to serialize tests that
mutate the process-global `XDG_CONFIG_HOME` env var, which `connect_and_configure`
/ `dispatch` read lazily — so the guard must span every network await. The lock
is broad (it stands in for "this test wants an isolated config"), gratuitous in
some cases, and forces all env-touching tests into one global critical section.

## Goal

Make the locking explicit and narrow across the four flagged files:

- `src/test_helpers.rs`
- `src/lib_tests.rs`
- `src/commands/runtime/shared/mod_tests.rs`
- `src/commands/field_tests.rs`

A migrated test selects its throwaway config by an **explicit path** and holds
no shared lock. `ENV_LOCK` survives only where a test mutates a process-global
env var that command resolution must read.

## Mechanism

`Config::path_at` precedence is `explicit override > BZR_CONFIG > XDG_CONFIG_HOME`.
Production already threads `CommandContext::config_path_override` (from `--config`)
through `Config::load_at` / `update_locked_at` / `read_unvalidated_at` at every
config call site (verified: `resolve_connect_target` at
`src/commands/runtime/shared/connection.rs:461,478`). Therefore:

- **`execute`-level tests** build
  `CommandContext::new(..).with_config_path_override(Some(config_path))`.
- **`dispatch`-level tests** add `--config <config_path>` to the parsed argv.

Either way `path_at` returns before reading `BZR_CONFIG` or `XDG_CONFIG_HOME`, so
the test needs no `XDG_CONFIG_HOME` and no lock.

### Concurrency safety (why dropping the lock is sound)

The retained env-mutating tests (and the ~100 out-of-scope `setup_test_env`
tests) still `set_var`/`remove_var` under `ENV_LOCK`, while migrated tests run
lock-free. This is safe for two independent reasons:

1. **No value cross-talk.** With an explicit config path, a migrated test reads
   neither `XDG_CONFIG_HOME` nor `BZR_CONFIG`. The only process-env it reads is
   `BZR_TIMEOUT` (in `build_command_context`, dispatch-level only) and any proxy
   vars reqwest consults at client-build time. **No test in the suite writes
   `BZR_TIMEOUT` or proxy vars** (verified by grepping every `set_var`/`remove_var`
   target), so the values a migrated test reads are deterministic regardless of
   what a concurrent writer sets. Writers only touch disjoint variables
   (`XDG_CONFIG_HOME`, `BZR_*` api-key/keyring vars, `EDITOR`, `DISPLAY`, …).
2. **No data race on `environ`.** Rust's `std::env::var`/`set_var`/`remove_var`
   serialize through libstd's internal env lock, so concurrent Rust-side env
   access is not a data race even across threads (the edition-2024 `unsafe`
   marking reflects the C-FFI/signal hazard, not removal of that lock). All env
   access in this test process is pure-Rust `std::env`; the only non-Rust env
   reads would be libc `getenv` inside `getaddrinfo`, which is avoided because
   wiremock binds `127.0.0.1` (numeric host, no name resolution). This residual
   is pre-existing — any current non-locked env read already shares it — and is
   not introduced by this change.

A `tokio::sync::RwLock` read/write split (migrated tests take a read guard,
mutators take a write guard) was rejected: a read guard held across the network
awaits is still a guard held across await points, so it would not clear the
desloppify finding this issue exists to fix.

## Helper changes (`src/test_helpers.rs`)

Add, without removing the existing `setup_test_env` / `setup_empty_config_env`
(still used by ~100 out-of-scope tests):

- `write_config_to(tmp: &TempDir, contents: &str) -> PathBuf` — writes
  `contents` to `<tmp>/bzr/config.toml`, applies `0o700`/`0o600` perms on unix,
  returns the file path. **No env mutation.** Centralizes the dir/permissions
  boilerplate the per-file writers duplicate.
- `setup_isolated_env() -> (MockServer, TempDir, PathBuf)` — starts a mock,
  writes the default `[servers.test]` config (api_key + cached
  `auth_method="header"`, `api_mode="rest"`, matching `setup_config`), returns
  the config path. No lock, no env var. The isolated analogue of
  `setup_test_env`.

## Per-test disposition

### `field_tests.rs`

- `field_aliases_succeeds_without_server`: **drop `ENV_LOCK` entirely.**
  `FieldAction::Aliases` returns a static list and never loads config or reads
  env (`src/commands/field.rs:15`).
- `field_list_*` (4 tests): migrate `setup_test_env()` → `setup_isolated_env()`,
  pass the path via `with_config_path_override`.

### `lib_tests.rs`

- All `setup_test_env()` / `write_public_config()` `dispatch` tests: migrate to
  an isolated config path + `--config <path>` in argv. `write_public_config`
  becomes path-returning, no env mutation.
- Inline `--server-url` tests (`dispatch_rejects_inline_write_without_api_key_env_before_network`,
  the self-signed TLS group via `assert_self_signed_inline_server_info_succeeds`):
  inline connects never load config; pass `--config <isolated path>` and assert
  the file is not created. Drop the lock.
- **Keep `ENV_LOCK`** in `dispatch_applies_config_flag_without_global_override`:
  it must set `XDG_CONFIG_HOME` to prove `--config` overrides it. Keep the
  comment naming the variable.

### `commands/runtime/shared/mod_tests.rs`

- Convert local `write_config` / `write_credentialless_config` to path-returning,
  no-env helpers (built on `write_config_to`). Thread the path into
  `connect_context` (add a config-path field) and `load_config(path)`.
- Migrate every `connect_and_configure` / `detect_*` / `persist_detected_settings`
  test to the explicit path; drop their locks.
- **Keep `ENV_LOCK`** in the three tests that mutate an API-key env var:
  - `connect_client_resolves_env_backed_api_key` (sets `BZR_TEST_API_KEY`),
  - `inline_server_connects_without_config_and_persists_nothing` (sets
    `BZR_INLINE_TEST_KEY`),
  - `inline_server_missing_env_var_is_clean_error` (removes
    `BZR_INLINE_ABSENT_KEY`).
  Each keeps a comment naming the variable it protects.

## Out of scope

- `setup_test_env` / `setup_empty_config_env` and their ~100 other call sites.
- Any production code change. `connect_and_configure` and `Config` are untouched.

## Acceptance criteria

- Flagged async tests no longer hold a guard across awaits except the documented
  env-mutating tests; each retained lock has a comment naming the variable.
- Migrated tests pass the config by explicit path.
- **Structural invariant (checkable, not statistical):** after the change, the
  only `ENV_LOCK.lock()` acquisitions in the four files are the four documented
  env-mutating tests (`connect_client_resolves_env_backed_api_key`,
  `inline_server_connects_without_config_and_persists_nothing`,
  `inline_server_missing_env_var_is_clean_error`,
  `dispatch_applies_config_flag_without_global_override`); every other test in
  the four files acquires no lock. Verify by grepping `ENV_LOCK` in the four
  files and confirming the count and the names.
- `cargo test` passes. To surface any ordering assumption, the **full** suite
  (not just the migrated subset — the relevant interaction is migrated-reader vs.
  writer in another file) runs green three times at `--test-threads=16`:
  `for i in 1 2 3; do cargo test -- --test-threads=16 || break; done`.
- `cargo clippy --all-targets --all-features -- -D warnings` clean;
  `make check-test-layout` and `make check-no-spawn` pass.

## Risks

- A migrated test that *did* implicitly rely on a cached `auth_method`/`api_mode`
  from `setup_config` must reproduce the same config contents via the isolated
  helper. Mitigation: `setup_isolated_env` mirrors `setup_config` byte-for-byte.
- `connect_context` currently hard-codes `config_path_override: None`; tests that
  drive `detect_and_build_client` / `persist_detected_settings` and then reload
  must thread the explicit path through both the write and the reload, or the
  reload reads the wrong file. Mitigation: single `load_config(path)` signature.
