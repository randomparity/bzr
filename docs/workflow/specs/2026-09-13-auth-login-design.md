# Auth login and logout design

## Problem

Named servers can hold a REST login token, but bzr cannot acquire or revoke one.
The new network `auth` family must not be folded into local-only `config`.

## Scope

`auth login` accepts an email and password, plus `--restrict-login`; it selects
REST or XML-RPC from the resolved API mode, receives a token, and atomically
stores it on the selected named server. `auth logout` invokes the corresponding
remote invalidation endpoint and removes the local token only after that call
succeeds. Inline servers are rejected because they cannot safely persist a
token. Existing API-key sources are rejected rather than silently replaced.

### Failure model

- Missing named server, a non-token credential source, or inline configuration fails before a network write.
- Failed login leaves the existing configuration untouched.
- Failed remote logout retains the local token for a retry.
- A concurrent replacement token is retained; logout removes only the token it revoked.
- REST-only token use remains enforced by the existing connection path after persistence.

### Threat model

The email/password CLI values and server-issued token are secrets. Passwords are sent only
to the selected server over its configured TLS policy and are never written, logged, or
returned. A token crosses into the existing atomic config writer, which creates owner-only
files; logout removes it only after the server accepts invalidation. Server name and URL come
from validated named configuration, so the command does not accept a destination URL.

## Success

The commands use `GET /login` and `GET /logout` for REST and `User.login` and
`User.logout` for XML-RPC, persist or remove only the token field under the
config lock, expose a stable result, document the command, and prove both
transport request shapes plus real-server login/logout behavior.

## Validation

| Contract | Focused test | Task test not applicable |
| --- | --- | --- |
| REST request, token persistence, and logout removal | command/auth unit tests with wiremock | — |
| XML-RPC login/logout request shape | XML-RPC user-resource tests | — |
| configured command parsing and output | CLI/command unit tests | — |
| live server login, cached token, logout | comparison and functional auth phase | — |
