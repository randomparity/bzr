# bugzillarc import design

## Problem

Add `bzr config import-bugzillarc [--path FILE]`, a local-only migration command for
python-bugzilla configuration. It reads the standard three files in python-bugzilla
precedence when no path is supplied; an explicit path imports only that file.

## Scope

The parser accepts INI sections and the `url`, `api_key`, `user`, `password`, and
`cert` keys. `[DEFAULT]` supplies a URL; other sections apply when their name is a
substring of that URL, matching python-bugzilla. Later source files replace earlier
values. Each resolved URL becomes a server named from its host, or updates a matching
existing URL. Only `api_key` is persisted. `user`, `password`, and `cert` are reported
as unsupported and are never persisted.

The command performs one locked config update and does no connection, authentication,
or TLS probing. It leaves an existing default unchanged; otherwise the first imported
server becomes default.

## Failure model

- Actors and deployments: local terminal users migrating local python-bugzilla files.
- Invariants and assets at stake: config updates remain one locked atomic write; secrets never
  appear in output; login credentials never become `Bugzilla_token` values.
- Accepted failure classes: missing default files are ignored; malformed or explicitly unreadable
  files fail before mutation.
- Covered elsewhere: password login is excluded by the user decision; client certificates are #677.

## Success

Unreadable explicit paths and malformed INI input fail before config mutation. Missing
default source files are ignored. Human output reports unsupported password credentials and
certificates without printing secrets. JSON returns counts and the config-file path.

## Validation

Unit tests cover precedence, DEFAULT and substring resolution, API-key mapping, password
non-persistence, malformed input, and matching-URL updates. The comparison fixture changes
the import row from an expected gap to API-key import coverage; a functional phase imports a
fixture into a fresh config and inspects the persisted credential source and unsupported report.
