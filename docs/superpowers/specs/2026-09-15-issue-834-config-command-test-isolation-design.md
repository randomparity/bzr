# Issue #834 — `commands/config` test isolation design

- Status: Implemented
- Date: 2026-09-15
- Issue: #834 (epic #818)
- ADR: [0002](../../adr/0002-test-config-isolation-over-env-lock.md)

## Scope

Migrate the eight `src/commands/config/*_tests.rs` siblings off
`ENV_LOCK`-guarded `XDG_CONFIG_HOME` mutation onto explicit per-test config
paths, using the lock-free helper companions #819 landed in
`src/test_helpers.rs`.

Measured surface at branch point (`origin/main`, re-counted directly, not
taken from the issue body):

| Quantity | Count |
| --- | --- |
| `setup_empty_config_env().await` call sites | 62 |
| Ambient-helper calls (`load_config`, `seed_inline_server`, …) | 129 |
| `CommandContext::new` constructions | 56 |

The issue body says 68 call sites; the direct count is **62**, matching the
campaign's corrected figure. `62 + 129 + 56 = 247`, against the dispatched
denominator of ~250.

Out of scope, reported only: `src/config/store.rs` (epic non-goal), production
paths in `commands/config/migrate.rs`, deletion of the superseded
`setup_test_env` / `setup_config` / `setup_empty_config_env` helpers (#835),
the env-race decision (#831).

## Mechanical transform

Per test:

```rust
let (_lock, _tmp) = setup_empty_config_env().await;   // before
let (_tmp, config_path) = setup_empty_isolated_env(); // after
```

Ambient helpers are replaced by their `_at` companions, each taking
`&config_path`. `config_path()` (which resolves `Config::path_at(None)`)
becomes the local `config_path` binding.

`CommandContext::new(None, fmt, None)` gains
`.with_config_path_override(Some(config_path.clone()))`. Because that appears
56 times, each file gets one private `ctx_at(&Path, OutputFormat)` helper,
mirroring the `ctx_at` already used in
`src/commands/runtime/shared/connection/mod_tests.rs:16`.

## Decision 1 — `ENV_LOCK` is retained for five tests, and narrowed

ADR-0002 retains `ENV_LOCK` for tests that mutate a process-global env var the
command must observe. Five tests in scope mutate one inline and cannot route
through `seed_keyring_secret_at` (which takes the lock itself):

| Test | Variable | Why the seeder does not fit |
| --- | --- | --- |
| `keyring_tests::set_keyring_table_output_reports_human_summary` | `BZR_KEYRING_TEST_SECRET` | asserts **table** output; the seeder runs `--json` |
| `keyring_tests::set_keyring_is_blocked_by_other_structurally_invalid_server` | `BZR_KEYRING_TEST_SECRET` | asserts the `Err`; the seeder panics on error |
| `keyring_tests::unset_keyring_deletes_the_entry_named_by_the_server` | `BZR_KEYRING_TEST_SECRET` | needs an explicit `account`, which the seeder does not take |
| `remove_tests::remove_server_deletes_the_entry_named_by_the_removed_server` | `BZR_KEYRING_TEST_SECRET` | needs an explicit `account`, as above |
| `migrate_tests::migrate_to_keyring_from_env_preserves_config` | `BZR_MIGRATE_TEST_KEY` | seeds an `api_key_env` server, not a keyring one |

These five take `ENV_LOCK` directly, with a comment naming the variable, and
**still** select config by explicit path — the pattern already set by
`connection/mod_tests.rs:279`. None of them calls `seed_keyring_secret_at`, so
the non-reentrant lock cannot deadlock.

Net: `ENV_LOCK` acquisitions in this scope fall 62 → 5, and none of the five
mutates `XDG_CONFIG_HOME`.

## Decision 2 — unique keyring `service` per test

`keyring::install_test_store` memoizes **one** process-global store keyed on
`(service, account)`. `ENV_LOCK` was serializing whole test bodies; once it is
gone, account collisions become races. Four collisions exist today:

- `("bzr", "prod")` — two tests in `keyring_tests.rs`
- `("bzr", "target")` — two tests in `keyring_tests.rs`
- `("bzr", "keepme")` — `remove_tests.rs` **and** `rename_tests.rs`
- `("bzr", "dropme")` — two tests in `remove_tests.rs`

Fix: each keyring-touching test picks a `service` unique to itself, carried by
`ConfigAction::SetKeyring`'s existing `service` field to both the store and the
persisted `KeyringRef`. No production change. `retrieve`/`delete`/`store`
assertions move to the same service.

**Two tests deliberately keep the default `"bzr"` service**:
`unset_keyring_deletes_the_entry_named_by_the_server` and
`remove_server_deletes_the_entry_named_by_the_removed_server`. Both plant a
decoy at the *default* coordinates to prove the command reads the entry's
explicit service/account instead of falling back — changing the service would
delete the property under test. Their accounts (`"named"`, `"dropme"`) stay
collision-free because the sibling that also used `("bzr", "dropme")`
(`remove_server_keeps_the_secret_when_the_config_write_fails`) moves to a
unique service.

## Config-path write hazard

`Config::path_at(None)` resolves `dirs::config_dir()` on macOS —
`~/Library/Application Support/bzr/config.toml`, the operator's real config.
Every ambient helper in scope reaches it. The migration removes all of them
from these files; the fault-injection check (below) substitutes an absolute
nonexistent path, never `None`, because `path_at` returns an explicit override
immediately (`src/config/store.rs:18-22`) and never consults the environment.

## Verification

1. `make test-one T=commands::config` — all migrated tests green.
2. Fault injection: rewrite `ctx_at` to override with an absolute nonexistent
   path instead of `config_path`. Tests must go **red**, proving the override is
   load-bearing rather than incidentally passing off an ambient root. Revert.
3. `stat` the real macOS config path before and after the run and compare
   mtime — not an `~/.config/bzr` existence check, which returns clean on a
   machine where the damage happened.
4. `make lint`, then `make test`.

Behavior is unchanged throughout: same configs, same assertions, same
commands. Only config selection and keychain coordinates change.
