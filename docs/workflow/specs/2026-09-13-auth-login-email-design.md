# Auth login identity persistence design

## Problem

`auth login` persists a returned token but not the authenticated email. Named
servers on Bugzilla 5.0/5.2 then cannot use the existing email-backed `whoami`
fallback, so `bug my` also fails after a successful login.

## Scope

The existing successful-login `Config::update_locked_at` closure will write the
returned token and the supplied email together. The write remains after the
network login succeeds, so a failed login leaves the previously stored pair
unchanged. No new configuration shape, command, transport behavior, or inline
server persistence is introduced. `auth.rs` remains the owner of this update;
the existing client fallback and `bug my` consumers need no migration.

### Failure model

- Actors and deployments: terminal operators using named servers against REST,
  XML-RPC, or hybrid Bugzilla 5.0/5.2 deployments.
- Invariants and assets: a saved token and identity describe the same successful
  login; locked config updates do not overwrite concurrent changes accidentally.
- Accepted failure classes: a login request that fails leaves storage untouched;
  server-side identity lookup errors remain owned by the existing client path.
- Covered elsewhere: login transport/token semantics are owned by ADR 0076;
  inline-server persistence is owned by the named-server auth contract.

## Success

- Successful named-server login replaces missing or stale `email` with the login
  email in the same locked update that saves the returned token.
- A failed login keeps the existing token/email pair unchanged.
- Existing fallback behavior lets a post-login `whoami` and `bug my --limit 1`
  run on the supported 5.0/5.2 functional path.
- Focused Rust tests, a failure-aware RHBZ comparison assertion, and stock 5.0
  and 5.2 functional cases prove the behavior.

## Validation

- `src/commands/auth_tests.rs`: focused mocked-login tests prove missing and
  stale identity replacement, plus preservation after a login failure; run
  `make test-one T=login_`.
- `tests/functional/compare/06-auth-config-tls.sh`: a dedicated failure-aware
  named-server login sequence runs `whoami`, `bug my --limit 1`, and logout;
  run `make functional-compare-rhbz`.
- `tests/functional/phases/02-server-auth.sh`: a 5.0/5.2-only named-server
  login sequence runs `whoami`, `bug my --limit 1`, and logout; run
  `make functional-test-bz50` and `make functional-test-bz52`.
- `make lint` and `make test` validate project-wide Rust and shell contracts.
