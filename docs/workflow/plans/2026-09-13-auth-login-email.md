# Persist auth-login identity

Goal: bind a successful named-server login token to its authenticated email so
the existing old-Bugzilla identity fallback can run. The Rust command already
owns the locked config update; tests use wiremock and the existing RHBZ compare
phase. No dependencies, schema changes, or new command surface are introduced.

## Global Constraints

- Rust MSRV is 1.89.0; preserve the single-threaded Tokio runtime.
- Use `Writers` for CLI output and sibling `*_tests.rs` test modules.
- Keep token/email writes in the existing `Config::update_locked_at` closure.

Expected implementation size: 20–55 changed lines (S) — derived from one
existing assignment, focused tests, and two functional sequence extensions.

## Task 1: atomically persist authenticated identity

Files: modify `src/commands/auth.rs`; test `src/commands/auth_tests.rs`.

Interfaces: `AuthAction::Login { email, password, restrict_login }` supplies
`email: String`; `ServerConfig` exposes `token: Option<String>` and
`email: Option<String>`.

Verification:

- Contract: successful login writes matching token/email. Mode: focused-test;
  add a test with an initially missing email, observe its red failure before the
  assignment, then run `make test-one T=login_persists` expecting success.
- Contract: stale email is replaced and failed login preserves both values.
  Mode: focused-test; add separate mocked responses and run
  `make test-one T=login_` expecting success.

Steps: add `server.email = Some(email.clone())` beside the existing token write;
add the focused tests; run `cargo fmt`; run the named test command.

Acceptance: the config mutation has one lock acquisition and only follows a
successful `client.login` result.

## Task 2: prove the named-server fallback on real Bugzilla

Files: modify `tests/functional/compare/06-auth-config-tls.sh` and
`tests/functional/phases/02-server-auth.sh`.

Interfaces: `run_bzr --server r11-login auth login`, `whoami`,
`bug my --limit 1`, and `auth logout` are existing CLI commands.

Verification:

- Contract: the configured named RHBZ server can log in, identify itself, list
  its bugs, and log out. Mode: focused-test; use an explicit failure-aware
  `if`/`test_pass`/`test_fail` case because the nearby helper records PASS
  before its command group; `make functional-compare-rhbz` passes afterward.
- Contract: the same named-server path reaches the email fallback on stock 5.0
  and 5.2. Mode: focused-test; add a version-gated case to phase 02 and run
  `make functional-test-bz50` and `make functional-test-bz52` expecting pass.

Steps: add a dedicated RHBZ assertion containing login, whoami, bug-my, and
logout; add the stock version-gated phase assertion; run the three functional
commands, then `make lint` and `make test`.

Acceptance: the phase proves the real RHBZ path without changing the excluded
transport semantics.
