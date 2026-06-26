# Spec: add `auth_mode` and `server_name` to `whoami --json`

- Issue: #470 (follow-up from #461 / PR #469)
- ADR: [0009](../../adr/0009-whoami-connection-metadata.md)
- Status: Draft

## Problem

`bzr whoami --json` serializes only the identity fields the Bugzilla server
returns (`id`, `name`, `real_name`, `login`). An agent that wants to confirm
"which server am I talking to, and how am I authenticated?" cannot get that from
one call — the connection facts (the resolved server name and whether the
connection used an API key) are known to `bzr` at request time but never
surfaced.

## Goal

Make `bzr whoami --json` answer config + auth + identity in one call by adding
two **connection-metadata** fields to the `data` payload:

- `server_name` (string) — the configured/inline server name the identity
  resolved against (e.g. `default`, `auto`, or the inline-server sentinel).
- `auth_mode` (enum string) — `"api_key"` when the connection carried
  credentials, `"anonymous"` otherwise. Reuses the vocabulary already published
  by `server capabilities` (`auth_modes`).

The change is **additive**: the existing identity fields keep their names,
types, and positions. No identity field is reshaped.

## Non-goals

- No change to `--output ndjson` shape beyond the two added fields (records stay
  bare, no envelope).
- No change to the `whoami` Bugzilla request, the 5.0 user-lookup fallback, or
  the credential-required gate (anonymous `whoami` still fails before the
  network call, so in practice `auth_mode` is `"api_key"` on success today; the
  enum is emitted honestly from client state and stays correct if that gate ever
  changes).
- No new CLI flags.

## Design

### Where the data lives (see ADR 0009)

- `auth_mode` and `server_name` are **connection** facts, not identity facts.
  They are owned by `BugzillaClient` (which already holds the resolved API mode,
  base URL, and credential) rather than by `WhoamiResponse` (the
  `Deserialize` target for the server's `/rest/whoami` body).
- `WhoamiResponse` is unchanged. A new `WhoamiOutput` wrapper composes the
  identity (`#[serde(flatten)]`) with the two connection fields. Flattening
  yields one flat JSON object: `{id, name, real_name, login, server_name,
  auth_mode}`.

### Type changes

- New `AuthMode` enum in `src/types/transport.rs` (next to `AuthMethod` /
  `ApiMode`): variants `ApiKey`, `Anonymous`, `#[serde(rename_all =
  "snake_case")]` → `"api_key"` / `"anonymous"`, with `Display`.
- New `WhoamiOutput` in `src/types/user.rs`:

  ```rust
  pub struct WhoamiOutput {
      #[serde(flatten)]
      pub identity: WhoamiResponse,
      pub server_name: String,
      pub auth_mode: AuthMode,
  }
  ```

- `BugzillaClient` gains a `server_name: String` field and two accessors:
  `server_name(&self) -> &str` and `auth_mode(&self) -> AuthMode` (derived from
  `self.api_key.is_some()`). `BugzillaClientConfig` gains `server_name: &str`.
  All construction sites (`target.rs::build_client`, `client/test_helpers.rs`)
  pass it.

### Command + output

- `commands/whoami.rs` composes `WhoamiOutput` from `client.whoami()` plus
  `client.server_name()` / `client.auth_mode()`.
- `write_whoami` takes `&WhoamiOutput`; the human-readable form gains `Server:`
  and `Auth:` lines after the identity lines. Table/JSON/ndjson go through the
  existing `write_formatted` path.

### Schema

- New `schemas/whoami.json` (draft 2020-12), closed (`additionalProperties:
  false`). `required`: `id`, `name`, `real_name`, `login`, `server_name`,
  `auth_mode`. `auth_mode` is an `enum` of `["api_key", "anonymous"]`. Added to
  the `SCHEMAS` registry (`whoami`, sorted last) and exercised by a maximally
  populated `whoami_conforms` drift test.

## Acceptance criteria

1. `bzr whoami --json` `data` contains `server_name` (string) and `auth_mode`
   (`"api_key"`/`"anonymous"`) alongside the unchanged identity fields.
2. `bzr schema whoami` prints the new schema; `bzr schema --json` lists
   `whoami`.
3. The schema drift test fails if any `WhoamiOutput` field is added/removed/
   renamed without updating `schemas/whoami.json`.
4. A functional phase asserts `.server_name` and `.auth_mode` on a real
   credentialed `whoami` (named and inline).
5. CHANGELOG, `docs/bzr-cli.md` whoami section, and the `bzr-setup` Health-check
   section mention the two new fields.
6. `--output ndjson` whoami carries the two fields with no envelope.

## Failure modes / edges

- Anonymous/credentialless `whoami` still exits before the network call
  (`requires credentials`); the new fields never appear on that error path.
- Bugzilla 5.0 user-lookup fallback path still produces a `WhoamiResponse`; the
  wrapper composes identically.
- `name`/`real_name`/`login` remain nullable and always serialize (as `null`
  when absent), so the closed-schema bijection holds.
</content>
</invoke>
